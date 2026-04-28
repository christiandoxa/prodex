#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { readCargoVersion, repoRoot } from "./common.mjs";

const execFileAsync = promisify(execFile);
const changelogPath = path.join(repoRoot, "CHANGELOG.md");
const defaultReleaseCount = 12;
const versionTagPattern = /^v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;

const categories = [
  ["runtime", "Runtime"],
  ["cli", "CLI"],
  ["claude", "Claude"],
  ["docs", "Docs"],
  ["tests", "Tests"],
  ["ci", "CI"],
  ["deps", "Deps"],
  ["misc", "Misc"],
];

function parseArgs(argv) {
  const args = { mode: "write", releases: defaultReleaseCount, help: false };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--check") {
      args.mode = "check";
      continue;
    }
    if (value === "--write") {
      args.mode = "write";
      continue;
    }
    if (value === "--print") {
      args.mode = "print";
      continue;
    }
    if (value === "--releases") {
      index += 1;
      const count = Number.parseInt(argv[index] ?? "", 10);
      if (!Number.isInteger(count) || count < 1) {
        throw new Error("--releases expects a positive integer");
      }
      args.releases = count;
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

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/npm/changelog.mjs [--write|--check|--print] [--releases N]",
      "",
      "Generates CHANGELOG.md from recent conventional commits.",
      "",
      "Groups commits into runtime, CLI, Claude, docs, tests, ci, deps, and misc.",
      "Default mode writes CHANGELOG.md. --check verifies the checked-in file.",
    ].join("\n") + "\n",
  );
}

async function git(args) {
  const { stdout } = await execFileAsync("git", args, {
    cwd: repoRoot,
    maxBuffer: 16 * 1024 * 1024,
  });
  return stdout.trimEnd();
}

async function versionTags() {
  const stdout = await git(["tag", "--merged", "HEAD", "--sort=version:refname"]);
  return stdout
    .split(/\r?\n/)
    .map((tag) => tag.trim())
    .filter((tag) => versionTagPattern.test(tag));
}

function tagVersion(tag) {
  return tag.replace(/^v/, "");
}

async function tagDate(tag) {
  const date = await git(["log", "-1", "--format=%cs", tag]);
  return date || "unknown date";
}

async function commitsForRange(range) {
  const stdout = await git(["log", "--format=%H%x09%s", range]);
  if (!stdout) {
    return [];
  }

  return stdout.split(/\r?\n/).map((line) => {
    const [hash, subject] = line.split("\t", 2);
    return parseCommit(hash, subject);
  });
}

function parseCommit(hash, subject) {
  const match = subject.match(/^([A-Za-z][A-Za-z0-9-]*)(?:\(([^)]+)\))?(!)?:\s+(.+)$/);
  if (!match) {
    return {
      hash,
      subject,
      type: null,
      scope: null,
      breaking: false,
      title: subject,
    };
  }

  const [, type, scope, bang, title] = match;
  return {
    hash,
    subject,
    type: type.toLowerCase(),
    scope: scope?.toLowerCase() ?? null,
    breaking: Boolean(bang),
    title,
  };
}

function isReleaseNoise(commit) {
  const subject = commit.subject.toLowerCase();
  if (commit.scope === "release") {
    return true;
  }
  return /\b(bump version|prepare \d+\.\d+\.\d+|refresh lockfile for \d+\.\d+\.\d+)\b/.test(
    subject,
  );
}

function includesAny(value, needles) {
  return needles.some((needle) => value.includes(needle));
}

function categorize(commit) {
  const type = commit.type ?? "";
  const scope = commit.scope ?? "";
  const haystack = `${type} ${scope} ${commit.title} ${commit.subject}`.toLowerCase();

  if (type === "test" || includesAny(scope, ["test", "tests"])) {
    return "tests";
  }
  if (
    type === "docs" ||
    includesAny(scope, ["docs", "readme", "quickstart"]) ||
    /\breadme\b/.test(haystack)
  ) {
    return "docs";
  }
  if (type === "ci" || includesAny(scope, ["ci", "compat", "workflow", "workflows", "github"])) {
    return "ci";
  }
  if (type === "deps" || includesAny(scope, ["deps", "dependency", "dependencies"])) {
    return "deps";
  }
  if (
    includesAny(scope, ["runtime", "runtime-proxy", "runtime-broker", "runtime-launch", "runtime-store"]) ||
    includesAny(haystack, ["runtime", "proxy", "websocket", "codex compatibility", "continuation"])
  ) {
    return "runtime";
  }
  if (
    includesAny(scope, ["claude", "anthropic", "mcp", "caveman"]) ||
    includesAny(haystack, ["claude", "anthropic", "mcp"])
  ) {
    return "claude";
  }
  if (
    includesAny(scope, ["cli", "profile", "profiles", "quota", "secret", "import", "launch", "super"]) ||
    includesAny(haystack, ["prodex super", "super mode", "launch dry-run", "profile", "quota"])
  ) {
    return "cli";
  }

  return "misc";
}

function displayTitle(commit) {
  const title = commit.title.trim();
  if (!title) {
    return commit.subject.trim();
  }
  return `${title[0].toUpperCase()}${title.slice(1)}`;
}

function groupedEntries(commits) {
  const groups = Object.fromEntries(categories.map(([key]) => [key, []]));

  for (const commit of commits) {
    if (isReleaseNoise(commit)) {
      continue;
    }
    groups[categorize(commit)].push(commit);
  }

  return groups;
}

function hasEntries(groups) {
  return Object.values(groups).some((entries) => entries.length > 0);
}

function renderGroups(groups) {
  const lines = [];

  for (const [key, heading] of categories) {
    const entries = groups[key];
    if (entries.length === 0) {
      continue;
    }

    lines.push(`### ${heading}`, "");
    for (const commit of entries) {
      const breaking = commit.breaking ? " **Breaking:**" : "";
      lines.push(`- ${breaking}${displayTitle(commit)} (\`${commit.hash.slice(0, 7)}\`)`);
    }
    lines.push("");
  }

  return lines;
}

async function renderChangelog({ releases }) {
  const tags = await versionTags();
  const latestTag = tags.at(-1) ?? null;
  const currentVersion = await readCargoVersion();
  const lines = [
    "# Changelog",
    "",
    "Generated from conventional commits. Run `npm run changelog` to refresh.",
    "",
  ];

  if (latestTag) {
    const unreleasedCommits = await commitsForRange(`${latestTag}..HEAD`);
    const unreleasedGroups = groupedEntries(unreleasedCommits);
    if (hasEntries(unreleasedGroups)) {
      const latestVersion = tagVersion(latestTag);
      const pendingVersion =
        currentVersion !== latestVersion && !tags.some((tag) => tagVersion(tag) === currentVersion)
          ? currentVersion
          : null;
      lines.push(pendingVersion ? `## ${pendingVersion} - Unreleased` : "## Unreleased", "");
      lines.push(`Changes after \`${latestTag}\`.`, "");
      lines.push(...renderGroups(unreleasedGroups));
    }
  }

  const selectedTags = tags.slice(Math.max(0, tags.length - releases)).reverse();
  for (const tag of selectedTags) {
    const tagIndex = tags.indexOf(tag);
    const previousTag = tagIndex > 0 ? tags[tagIndex - 1] : null;
    const range = previousTag ? `${previousTag}..${tag}` : tag;
    const groups = groupedEntries(await commitsForRange(range));
    const date = await tagDate(tag);

    lines.push(`## ${tagVersion(tag)} - ${date}`, "");
    if (hasEntries(groups)) {
      lines.push(...renderGroups(groups));
    } else {
      lines.push("- No grouped changes.", "");
    }
  }

  return `${lines.join("\n").trimEnd()}\n`;
}

function firstDiffLine(actual, expected) {
  const actualLines = actual.split(/\r?\n/);
  const expectedLines = expected.split(/\r?\n/);
  const maxLength = Math.max(actualLines.length, expectedLines.length);

  for (let index = 0; index < maxLength; index += 1) {
    if (actualLines[index] !== expectedLines[index]) {
      return {
        line: index + 1,
        actual: actualLines[index] ?? "<missing>",
        expected: expectedLines[index] ?? "<missing>",
      };
    }
  }

  return null;
}

async function checkChangelog(expected) {
  let actual;
  try {
    actual = await fs.readFile(changelogPath, "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") {
      throw new Error("CHANGELOG.md is missing; run npm run changelog");
    }
    throw error;
  }

  if (actual !== expected) {
    const diff = firstDiffLine(actual, expected);
    const detail = diff
      ? ` first mismatch at line ${diff.line}: expected ${JSON.stringify(diff.expected)}, found ${JSON.stringify(diff.actual)}`
      : "";
    throw new Error(`CHANGELOG.md is stale; run npm run changelog.${detail}`);
  }

  process.stdout.write("changelog: ok\n");
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const contents = await renderChangelog(args);
  if (args.mode === "check") {
    await checkChangelog(contents);
    return;
  }
  if (args.mode === "print") {
    process.stdout.write(contents);
    return;
  }

  await fs.writeFile(changelogPath, contents);
  process.stdout.write(`changelog: wrote ${path.relative(repoRoot, changelogPath)}\n`);
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`changelog: ${message}\n`);
  process.exitCode = 1;
}
