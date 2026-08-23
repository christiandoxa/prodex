#!/usr/bin/env node
import {
  assertSingleChangeSelector,
  isReleaseMetadataChangePath,
  isReleaseLikeMessage,
  parseReleaseGuardArgs,
  selectedChanges,
} from "./release-guard-common.mjs";

function parseArgs(argv) {
  const assumeRelease = argv.includes("--assume-release");
  const commonArgv = argv.filter((value, index) => index < 2 || value !== "--assume-release");
  return {
    ...parseReleaseGuardArgs(commonArgv, {
      allow: {
        staged: true,
        worktree: true,
        includeUntracked: true,
        message: true,
        messageFile: true,
      },
    }),
    assumeRelease,
  };
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/release-metadata-only-guard.mjs [selector] [--assume-release]",
      "",
      "Flags release/chore release commits that mix version metadata files with non-metadata files.",
      "",
      "Selectors:",
      "  --range <rev-range>       inspect each commit in a git range",
      "  --base <rev> --head <rev> inspect each commit in base..head",
      "  --commit <rev>            inspect one commit",
      "  --staged                  inspect staged files as one synthetic change",
      "  --worktree                inspect unstaged files as one synthetic change",
      "",
      "Options:",
      "  --message <text>          message for staged/worktree synthetic change",
      "  --message-file <path>     read synthetic message from file",
      "  --assume-release          treat synthetic change as release-like",
      "  --include-untracked       include untracked files with --worktree",
      "  --json                    print machine-readable result",
      "",
      "Default selector: --commit HEAD",
    ].join("\n") + "\n",
  );
}

function assertSingleSelector(args) {
  assertSingleChangeSelector(args);
}

function evaluateChange(change, args) {
  const metadataFiles = change.files.filter((filePath) => isReleaseMetadataChangePath(change, filePath));
  const nonMetadataFiles = change.files.filter((filePath) => !isReleaseMetadataChangePath(change, filePath));
  const releaseLike = args.assumeRelease || isReleaseLikeMessage(change.message);
  return {
    label: change.label,
    subject: change.message.split(/\r?\n/, 1)[0]?.trim() ?? "",
    releaseLike,
    metadataFiles,
    nonMetadataFiles,
    violation: releaseLike && metadataFiles.length > 0 && nonMetadataFiles.length > 0,
  };
}

async function selectedMetadataOnlyChanges(args) {
  if (args.range || (args.base && args.head)) {
    return selectedChanges(
      {
        ...args,
        message: undefined,
        messageFile: undefined,
      },
      { diffFilter: "ACMR", includeChangedLines: true },
    );
  }

  if (args.commit || (!args.staged && !args.worktree)) {
    return selectedChanges(
      {
        ...args,
        messageFile: undefined,
      },
      { diffFilter: "ACMR", includeChangedLines: true },
    );
  }

  return selectedChanges(args, { diffFilter: "ACMR", includeChangedLines: true });
}

function printHuman(selector, results) {
  const violations = results.filter((result) => result.violation);
  if (violations.length === 0) {
    const releaseLikeCount = results.filter((result) => result.releaseLike).length;
    process.stdout.write(
      `release metadata-only guard: ok (${results.length} change(s), ${releaseLikeCount} release-like)\n`,
    );
    return;
  }

  process.stderr.write(
    `release metadata-only guard: ${violations.length} violation(s) in ${selector}\n`,
  );
  for (const violation of violations) {
    process.stderr.write(`\n${violation.label}: ${violation.subject || "<no subject>"}\n`);
    process.stderr.write(`  metadata files:\n`);
    for (const filePath of violation.metadataFiles) {
      process.stderr.write(`    - ${filePath}\n`);
    }
    process.stderr.write(`  non-metadata files:\n`);
    for (const filePath of violation.nonMetadataFiles) {
      process.stderr.write(`    - ${filePath}\n`);
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
  const { selector, changes } = await selectedMetadataOnlyChanges(args);
  const results = changes.map((change) => evaluateChange(change, args));
  if (args.json) {
    process.stdout.write(`${JSON.stringify({ selector, results }, null, 2)}\n`);
  } else {
    printHuman(selector, results);
  }

  if (results.some((result) => result.violation)) {
    process.exitCode = 1;
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`release-metadata-only-guard: ${message}\n`);
  process.exitCode = 1;
}
