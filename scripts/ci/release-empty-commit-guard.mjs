#!/usr/bin/env node
import {
  assertSingleChangeSelector,
  isReleaseLikeMessage,
  parseReleaseGuardArgs,
  selectedChanges,
} from "./release-guard-common.mjs";

function parseArgs(argv) {
  return parseReleaseGuardArgs(argv, {
    allow: { staged: true, worktree: true, message: true, messageFile: true },
  });
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/release-empty-commit-guard.mjs [selector]",
      "",
      "Flags release-like commits or synthetic changes with no changed files.",
      "",
      "Selectors:",
      "  --range <rev-range>       inspect each commit in a git range",
      "  --base <rev> --head <rev> inspect each commit in base..head",
      "  --commit <rev>            inspect one commit",
      "  --staged                  inspect staged files as one synthetic change",
      "  --worktree                inspect unstaged files as one synthetic change",
      "",
      "Options:",
      "  --message <text>          override commit message or set synthetic message",
      "  --message-file <path>     read message from file",
      "  --json                    print machine-readable result",
      "",
      "Default selector: --commit HEAD",
    ].join("\n") + "\n",
  );
}

function assertValidArgs(args) {
  assertSingleChangeSelector(args);
  if (args.message && args.messageFile) {
    throw new Error("choose only one message source: --message or --message-file");
  }
}

function evaluateChange(change) {
  const subject = change.message.split(/\r?\n/, 1)[0]?.trim() ?? "";
  const releaseLike = isReleaseLikeMessage(change.message);
  return {
    label: change.label,
    subject,
    releaseLike,
    changedFileCount: change.files.length,
    files: change.files,
    violation: releaseLike && change.files.length === 0,
  };
}

function printHuman(selector, results) {
  const violations = results.filter((result) => result.violation);
  if (violations.length === 0) {
    const releaseLikeCount = results.filter((result) => result.releaseLike).length;
    process.stdout.write(
      `release empty-commit guard: ok (${results.length} change(s), ${releaseLikeCount} release-like)\n`,
    );
    return;
  }

  process.stderr.write(`release empty-commit guard: ${violations.length} violation(s) in ${selector}\n`);
  for (const violation of violations) {
    process.stderr.write(`\n${violation.label}: ${violation.subject || "<no subject>"}\n`);
    process.stderr.write("  release-like change has no changed files\n");
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  assertValidArgs(args);
  const selectionArgs = args.worktree ? { ...args, includeUntracked: true } : args;
  const { selector, changes } = await selectedChanges(selectionArgs);
  const results = changes.map(evaluateChange);
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
  process.stderr.write(`release-empty-commit-guard: ${message}\n`);
  process.exitCode = 1;
}
