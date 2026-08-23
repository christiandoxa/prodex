#!/usr/bin/env node
import {
  assertSingleChangeSelector,
  isVersionMetadataChangePath,
  isReleaseLikeMessage,
  parseReleaseGuardArgs,
  selectedChanges,
} from "./release-guard-common.mjs";
import { normalizeGitPath } from "./guard-common.mjs";

const CHANGELOG_PATH = "CHANGELOG.md";

function parseArgs(argv) {
  return parseReleaseGuardArgs(argv, {
    allow: {
      staged: true,
      worktree: true,
      includeUntracked: true,
      message: true,
      messageFile: true,
    },
  });
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/version-metadata-release-guard.mjs [selector]",
      "",
      "Flags version/release metadata changes outside release-like metadata-only commits.",
      "",
      "Selectors:",
      "  --range <rev-range>       inspect each commit in a git range",
      "  --base <rev> --head <rev> inspect each commit in base..head",
      "  --commit <rev>            inspect one commit",
      "  --staged                  inspect staged files as one synthetic change",
      "  --worktree                inspect unstaged files as one synthetic change",
      "",
      "Options:",
      "  --message <text>          override selected change message",
      "  --message-file <path>     read override message from file",
      "  --include-untracked       include untracked files with --worktree",
      "  --json                    print machine-readable result",
      "  --help                    print this help",
      "",
      "Default selector: --commit HEAD",
    ].join("\n") + "\n",
  );
}

function assertSingleSelector(args) {
  assertSingleChangeSelector(args);
}

function evaluateChange(change) {
  const metadataFiles = change.files.filter((filePath) => isVersionMetadataChangePath(change, filePath));
  const releaseLike = isReleaseLikeMessage(change.message);
  const nonMetadataFiles = change.files.filter((filePath) => {
    if (isVersionMetadataChangePath(change, filePath)) {
      return false;
    }
    return !(releaseLike && normalizeGitPath(filePath) === CHANGELOG_PATH);
  });
  const reasons = [];
  if (metadataFiles.length > 0 && !releaseLike) {
    reasons.push("metadata change is not release-like");
  }
  if (metadataFiles.length > 0 && nonMetadataFiles.length > 0) {
    reasons.push("metadata change is not metadata-only");
  }

  return {
    label: change.label,
    subject: change.message.split(/\r?\n/, 1)[0]?.trim() ?? "",
    releaseLike,
    metadataFiles,
    nonMetadataFiles,
    reasons,
    violation: reasons.length > 0,
  };
}

function printHuman(selector, results) {
  const violations = results.filter((result) => result.violation);
  if (violations.length === 0) {
    const metadataChangeCount = results.filter((result) => result.metadataFiles.length > 0).length;
    process.stdout.write(
      `version metadata release guard: ok (${results.length} change(s), ${metadataChangeCount} metadata change(s))\n`,
    );
    return;
  }

  process.stderr.write(`version metadata release guard: ${violations.length} violation(s) in ${selector}\n`);
  for (const violation of violations) {
    process.stderr.write(`\n${violation.label}: ${violation.subject || "<no subject>"}\n`);
    process.stderr.write(`  reason: ${violation.reasons.join("; ")}\n`);
    process.stderr.write(`  metadata files:\n`);
    for (const filePath of violation.metadataFiles) {
      process.stderr.write(`    - ${filePath}\n`);
    }
    if (violation.nonMetadataFiles.length > 0) {
      process.stderr.write(`  non-metadata files:\n`);
      for (const filePath of violation.nonMetadataFiles) {
        process.stderr.write(`    - ${filePath}\n`);
      }
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
  const { selector, changes } = await selectedChanges(args, { includeChangedLines: true });
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
  process.stderr.write(`version-metadata-release-guard: ${message}\n`);
  process.exitCode = 1;
}
