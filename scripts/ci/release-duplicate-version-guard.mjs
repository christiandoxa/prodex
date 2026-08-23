#!/usr/bin/env node
import {
  assertSingleCommitSelector,
  parseReleaseGuardArgs,
  releaseEntryFromSubject,
  selectedCommitSummaries as selectedCommits,
} from "./release-guard-common.mjs";

function parseArgs(argv) {
  return parseReleaseGuardArgs(argv);
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
  assertSingleCommitSelector(args);
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

  assertSingleSelector(args);
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
