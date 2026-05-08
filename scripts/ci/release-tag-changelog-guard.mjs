#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { git } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const VERSION_TAG_PATTERN = /^v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)$/;

function parseArgs(argv) {
  const args = {
    changelog: "CHANGELOG.md",
    json: false,
    rev: "HEAD",
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--rev") {
      index += 1;
      args.rev = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--changelog") {
      index += 1;
      args.changelog = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  return args;
}

function requiredValue(value, name) {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/release-tag-changelog-guard.mjs [options]",
      "",
      "Fails when a version tag points at a revision but CHANGELOG.md lacks the matching release section.",
      "",
      "Options:",
      "  --rev <rev>             revision to inspect; default HEAD",
      "  --changelog <path>      changelog path; default CHANGELOG.md",
      "  --json                  print machine-readable result",
      "  --help, -h              show help",
    ].join("\n") + "\n",
  );
}

function normalizeVersionTag(tag) {
  const match = VERSION_TAG_PATTERN.exec(tag);
  return match?.[1] ?? null;
}

function escapeRegExp(value) {
  return value.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&");
}

function hasChangelogHeading(changelog, version) {
  const pattern = new RegExp(`^##\\s+${escapeRegExp(version)}\\s+-\\s+`, "m");
  return pattern.test(changelog);
}

async function versionTagsAtRev(rev) {
  const { stdout } = await git(["tag", "--points-at", rev], { cwd: repoRoot });
  return stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map((tag) => ({ tag, version: normalizeVersionTag(tag) }))
    .filter(({ version }) => version);
}

async function check(args) {
  const tags = await versionTagsAtRev(args.rev);
  const changelogPath = path.resolve(repoRoot, args.changelog);
  const changelog = tags.length > 0 ? await fs.readFile(changelogPath, "utf8") : "";
  const checked = tags.map(({ tag, version }) => ({
    tag,
    version,
    found: hasChangelogHeading(changelog, version),
  }));
  const missing = checked.filter(({ found }) => !found);

  return {
    ok: missing.length === 0,
    rev: args.rev,
    changelog: path.relative(repoRoot, changelogPath) || ".",
    tags: checked,
    missing,
  };
}

function printHuman(result) {
  if (result.tags.length === 0) {
    process.stdout.write(`release-tag-changelog-guard: ok, no version tags at ${result.rev}\n`);
    return;
  }
  if (result.ok) {
    const versions = result.tags.map(({ version }) => version).join(", ");
    process.stdout.write(`release-tag-changelog-guard: ok, changelog has ${versions}\n`);
    return;
  }

  const missing = result.missing.map(({ version }) => version).join(", ");
  process.stderr.write(`release-tag-changelog-guard: missing changelog section for ${missing}\n`);
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const result = await check(args);
  if (args.json) {
    const stream = result.ok ? process.stdout : process.stderr;
    stream.write(`${JSON.stringify(result, null, 2)}\n`);
  } else {
    printHuman(result);
  }
  if (!result.ok) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`release-tag-changelog-guard: ${error.message}\n`);
  process.exitCode = 1;
});
