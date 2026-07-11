#!/usr/bin/env node
import {
  isReleaseMetadataChangePath,
  isReleaseLikeMessage,
  selectedChanges,
} from "./release-guard-common.mjs";

function parseArgs(argv) {
  const args = {
    assumeRelease: false,
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
    if (value === "--staged") {
      args.staged = true;
      continue;
    }
    if (value === "--worktree") {
      args.worktree = true;
      continue;
    }
    if (value === "--include-untracked") {
      args.includeUntracked = true;
      continue;
    }
    if (value === "--message") {
      index += 1;
      args.message = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--message-file") {
      index += 1;
      args.messageFile = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--assume-release") {
      args.assumeRelease = true;
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
  const selectors = [
    Boolean(args.range),
    Boolean(args.base || args.head),
    Boolean(args.commit),
    Boolean(args.staged),
    Boolean(args.worktree),
  ].filter(Boolean).length;
  if (selectors > 1) {
    throw new Error("choose only one selector: --range, --base/--head, --commit, --staged, or --worktree");
  }
  if ((args.base || args.head) && !(args.base && args.head)) {
    throw new Error("--base and --head must be used together");
  }
  if (args.includeUntracked && !args.worktree) {
    throw new Error("--include-untracked requires --worktree");
  }
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
