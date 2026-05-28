#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";
import {
  commitSummary,
  hasChangelogHeading,
  normalizeVersionTag,
  rangeCommits,
  requiredValue,
  versionTagsAtRev,
} from "./release-guard-common.mjs";
import { git } from "./guard-common.mjs";

function parseArgs(argv) {
  const args = {
    base: null,
    changelog: "CHANGELOG.md",
    head: null,
    json: false,
    range: null,
    rev: "HEAD",
    revProvided: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--rev") {
      index += 1;
      args.rev = requiredValue(argv[index], value);
      args.revProvided = true;
      continue;
    }
    if (value === "--range") {
      index += 1;
      args.range = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--base") {
      index += 1;
      args.base = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--head") {
      index += 1;
      args.head = requiredValue(argv[index], value);
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

  validateSelectorArgs(args);
  return args;
}

function validateSelectorArgs(args) {
  const hasRange = args.range !== null;
  const hasBaseHead = args.base !== null || args.head !== null;
  const selectorCount = Number(args.revProvided) + Number(hasRange) + Number(hasBaseHead);

  if (selectorCount > 1) {
    throw new Error("use only one selector: --rev, --range, or --base with --head");
  }
  if (hasBaseHead && (!args.base || !args.head)) {
    throw new Error("--base and --head must be used together");
  }
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/release-tag-changelog-guard.mjs [options]",
      "",
      "Fails when a version tag points at a revision but CHANGELOG.md lacks the matching release section",
      "or no matching release marker exists at the tagged commit or since the previous version tag.",
      "",
      "Options:",
      "  --rev <rev>             revision to inspect; default HEAD",
      "  --range <rev-range>     revision range to inspect, e.g. HEAD~5..HEAD",
      "  --base <rev>            base revision for --base <rev> --head <rev>",
      "  --head <rev>            head revision for --base <rev> --head <rev>",
      "  --changelog <path>      changelog path; default CHANGELOG.md",
      "  --json                  print machine-readable result",
      "  --help, -h              show help",
    ].join("\n") + "\n",
  );
}

function selectorFromArgs(args) {
  if (args.range) {
    return {
      kind: "range",
      label: args.range,
      revs: [args.range],
    };
  }
  if (args.base && args.head) {
    return {
      kind: "base-head",
      label: `${args.base}..${args.head}`,
      revs: [`${args.base}..${args.head}`],
      base: args.base,
      head: args.head,
    };
  }
  return {
    kind: "rev",
    label: args.rev,
    revs: [args.rev],
  };
}

async function selectedCommits(selector) {
  if (selector.kind === "rev") {
    return [selector.revs[0]];
  }

  return rangeCommits(selector.revs[0]);
}

async function versionTagsAtCommit(commit) {
  const tags = await versionTagsAtRev(commit);
  if (tags.length === 0) {
    return [];
  }

  const summary = await commitSummary(commit);
  return tags.map(({ tag, version }) => ({
    commit,
    tag,
    version,
    subject: summary.subject,
    shortHash: summary.shortHash,
  }));
}

async function versionTagsAtSelectedCommits(commits) {
  const tags = [];
  for (const commit of commits) {
    tags.push(...(await versionTagsAtCommit(commit)));
  }
  return tags;
}

function acceptedReleaseMarkerSubjects(version) {
  return [
    `chore(release): release ${version}`,
    `chore(release): prepare ${version}`,
  ];
}

function isAcceptedReleaseMarkerSubject(subject, version) {
  return acceptedReleaseMarkerSubjects(version).includes(subject);
}

async function mergedVersionTags(commit) {
  const { stdout } = await git(["tag", "--merged", commit, "--sort=version:refname"], { cwd: repoRoot });
  return stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map((tag) => ({ tag, version: normalizeVersionTag(tag) }))
    .filter(({ version }) => version);
}

async function previousMergedVersionTag(commit, tag) {
  const tags = await mergedVersionTags(commit);
  const index = tags.findIndex((entry) => entry.tag === tag);
  if (index <= 0) {
    return null;
  }
  return tags[index - 1];
}

async function releaseMarkerRange(commit, tag) {
  const previous = await previousMergedVersionTag(commit, tag);
  return previous ? `${previous.tag}..${commit}` : commit;
}

async function matchingReleaseMarker(commit, tag, version, taggedSubject) {
  if (isAcceptedReleaseMarkerSubject(taggedSubject, version)) {
    const summary = await commitSummary(commit);
    return {
      ...summary,
      direct: true,
    };
  }

  const range = await releaseMarkerRange(commit, tag);
  const commits = await rangeCommits(range);
  for (const rev of commits) {
    const summary = await commitSummary(rev);
    if (isAcceptedReleaseMarkerSubject(summary.subject, version)) {
      return {
        ...summary,
        direct: false,
      };
    }
  }
  return null;
}

async function check(args) {
  const selector = selectorFromArgs(args);
  const commits = await selectedCommits(selector);
  const tags = await versionTagsAtSelectedCommits(commits);
  const changelogPath = path.resolve(repoRoot, args.changelog);
  const changelog = tags.length > 0 ? await fs.readFile(changelogPath, "utf8") : "";
  const checked = [];
  for (const { commit, tag, version, subject, shortHash } of tags) {
    const marker = await matchingReleaseMarker(commit, tag, version, subject);
    checked.push({
      commit,
      shortHash,
      tag,
      version,
      expectedSubject: `${acceptedReleaseMarkerSubjects(version).join(" or ")} at the tag or since the previous version tag`,
      subject,
      subjectOk: Boolean(marker),
      releaseMarker: marker ? {
        shortHash: marker.shortHash,
        subject: marker.subject,
        direct: marker.direct,
      } : null,
      found: hasChangelogHeading(changelog, version),
    });
  }
  const missing = checked.filter(({ found }) => !found);
  const invalidSubjects = checked.filter(({ subjectOk }) => !subjectOk);

  return {
    ok: missing.length === 0 && invalidSubjects.length === 0,
    selector: selector.kind,
    label: selector.label,
    rev: selector.kind === "rev" ? args.rev : undefined,
    range: selector.kind !== "rev" ? selector.label : undefined,
    base: selector.base,
    head: selector.head,
    commits: commits.length,
    changelog: path.relative(repoRoot, changelogPath) || ".",
    tags: checked,
    missing,
    invalidSubjects,
  };
}

function printHuman(result) {
  if (result.tags.length === 0) {
    const preposition = result.selector === "rev" ? "at" : "in";
    process.stdout.write(`release-tag-changelog-guard: ok, no version tags ${preposition} ${result.label}\n`);
    return;
  }
  if (result.ok) {
    const versions = result.tags.map(({ version }) => version).join(", ");
    process.stdout.write(`release-tag-changelog-guard: ok, changelog has ${versions}\n`);
    return;
  }

  if (result.missing.length > 0) {
    const missing = result.missing.map(({ version }) => version).join(", ");
    process.stderr.write(`release-tag-changelog-guard: missing changelog section for ${missing}\n`);
  }
  if (result.invalidSubjects.length > 0) {
    const invalid = result.invalidSubjects
      .map(({ tag, expectedSubject, subject }) => `${tag}: expected "${expectedSubject}", got "${subject}"`)
      .join("; ");
    process.stderr.write(`release-tag-changelog-guard: invalid tagged commit subject: ${invalid}\n`);
  }
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
