#!/usr/bin/env node
import { git } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const SEMVER_SOURCE = String.raw`v?([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)`;
const RELEASE_SUBJECT_PATTERNS = Object.freeze([
  {
    action: "release",
    pattern: new RegExp(String.raw`^chore\(release\)!?:\s*release\s+${SEMVER_SOURCE}\s*$`, "i"),
  },
  {
    action: "prepare",
    pattern: new RegExp(String.raw`^chore\(release\)!?:\s*prepare\s+${SEMVER_SOURCE}\s*$`, "i"),
  },
  {
    action: "release",
    pattern: new RegExp(String.raw`^release(?:\([^)]*\))?!?:\s*${SEMVER_SOURCE}\s*$`, "i"),
  },
  {
    action: "release",
    pattern: new RegExp(String.raw`^release\s+${SEMVER_SOURCE}\s*$`, "i"),
  },
  {
    action: "bump",
    pattern: new RegExp(String.raw`^bump(?:\([^)]*\))?!?:\s*${SEMVER_SOURCE}\s*$`, "i"),
  },
]);

function parseArgs(argv) {
  const args = {
    json: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
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
    if (value === "--commit") {
      index += 1;
      args.commit = requiredValue(argv[index], value);
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
      "Usage: node scripts/ci/release-duplicate-version-guard.mjs [selector]",
      "",
      "Rejects duplicate release-like commit subjects for the same action and semver.",
      "",
      "Release-like subjects include:",
      "  chore(release): release 0.89.0",
      "  chore(release): prepare 0.89.0",
      "  release: 0.89.0",
      "  release 0.89.0",
      "  bump: 0.89.0",
      "",
      "Selectors:",
      "  --range <rev-range>       inspect each commit in a git range",
      "  --base <rev> --head <rev> inspect each commit in base..head",
      "  --commit <rev>            inspect one commit",
      "",
      "Options:",
      "  --json                    print machine-readable result",
      "",
      "Default selector: --commit HEAD",
    ].join("\n") + "\n",
  );
}

function assertSingleSelector(args) {
  const selectors = [
    Boolean(args.range),
    Boolean(args.base || args.head),
    Boolean(args.commit),
  ].filter(Boolean).length;
  if (selectors > 1) {
    throw new Error("choose only one selector: --range, --base/--head, or --commit");
  }
  if ((args.base || args.head) && !(args.base && args.head)) {
    throw new Error("--base and --head must be used together");
  }
}

function releaseEntryFromSubject(subject) {
  const trimmed = subject.trim();
  for (const { action, pattern } of RELEASE_SUBJECT_PATTERNS) {
    const match = trimmed.match(pattern);
    if (match) {
      return { action, version: match[1] };
    }
  }
  return null;
}

async function rangeCommits(range) {
  const { stdout } = await git(["rev-list", "--reverse", range], { cwd: repoRoot });
  return stdout.split(/\r?\n/).filter(Boolean);
}

async function selectedCommitRevs(args) {
  assertSingleSelector(args);
  if (args.range || (args.base && args.head)) {
    const range = args.range ?? `${args.base}..${args.head}`;
    return {
      selector: range,
      revs: await rangeCommits(range),
    };
  }

  const rev = args.commit ?? "HEAD";
  return {
    selector: rev,
    revs: [rev],
  };
}

async function commitSummary(rev) {
  const { stdout } = await git(["log", "-1", "--format=%H%x00%h%x00%s", rev], { cwd: repoRoot });
  const [hash, shortHash, subject] = stdout.trimEnd().split("\0");
  if (!hash || !shortHash) {
    throw new Error(`failed to read commit subject for ${rev}`);
  }
  return {
    rev,
    hash,
    shortHash,
    subject: subject ?? "",
  };
}

async function selectedCommits(args) {
  const { selector, revs } = await selectedCommitRevs(args);
  const commits = [];
  for (const rev of revs) {
    commits.push(await commitSummary(rev));
  }
  return { selector, commits };
}

function evaluateCommits(commits) {
  const releaseCommits = [];
  const subjectsByKey = new Map();

  for (const commit of commits) {
    const entry = releaseEntryFromSubject(commit.subject);
    if (!entry) {
      continue;
    }

    const releaseCommit = {
      action: entry.action,
      version: entry.version,
      hash: commit.hash,
      shortHash: commit.shortHash,
      subject: commit.subject,
    };
    releaseCommits.push(releaseCommit);

    const key = `${entry.action}:${entry.version}`;
    const existing = subjectsByKey.get(key) ?? [];
    existing.push(releaseCommit);
    subjectsByKey.set(key, existing);
  }

  const subjects = [...subjectsByKey.values()].map((subjectCommits) => ({
    action: subjectCommits[0].action,
    version: subjectCommits[0].version,
    commits: subjectCommits,
    violation: subjectCommits.length > 1,
  }));

  return {
    releaseCommits,
    subjects,
    violations: subjects.filter((subject) => subject.violation),
  };
}

function printHuman(selector, commits, evaluation) {
  if (evaluation.violations.length === 0) {
    process.stdout.write(
      `release duplicate-version guard: ok (${commits.length} commit(s), ${evaluation.releaseCommits.length} release-like)\n`,
    );
    return;
  }

  process.stderr.write(
    `release duplicate-version guard: ${evaluation.violations.length} duplicate release subject(s) in ${selector}\n`,
  );
  for (const violation of evaluation.violations) {
    process.stderr.write(`\n${violation.action} ${violation.version}:\n`);
    for (const commit of violation.commits) {
      process.stderr.write(`  - ${commit.shortHash} ${commit.subject || "<no subject>"}\n`);
    }
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const { selector, commits } = await selectedCommits(args);
  const evaluation = evaluateCommits(commits);
  const result = {
    selector,
    subjects: evaluation.subjects,
    violations: evaluation.violations,
  };

  if (args.json) {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  } else {
    printHuman(selector, commits, evaluation);
  }

  if (evaluation.violations.length > 0) {
    process.exitCode = 1;
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`release-duplicate-version-guard: ${message}\n`);
  process.exitCode = 1;
}
