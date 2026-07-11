#!/usr/bin/env node
import {
  isReleaseLikeMessage,
  isVersionMetadataChangePath,
  messageSubject,
  parseReleaseGuardArgs,
  selectedChanges,
} from "./release-guard-common.mjs";

const CHANGELOG_PATH = "CHANGELOG.md";

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/release-changelog-coupling-guard.mjs [selector]",
      "",
      "Fails version metadata changes that do not update CHANGELOG.md in the same commit.",
      "",
      "Selectors:",
      "  --range <rev-range>       inspect each commit in a git range",
      "  --base <rev> --head <rev> inspect each commit in base..head",
      "  --commit <rev>            inspect one commit",
      "  --staged                  inspect staged files",
      "  --worktree                inspect unstaged files",
      "  --message <subject>       use this message for staged/worktree checks",
      "  --message-file <path>     read the message from a file",
      "  --json                    print machine-readable result",
      "",
      "Default selector: HEAD.",
    ].join("\n") + "\n",
  );
}

function issueForChange(change) {
  const versionMetadataFiles = change.files.filter((filePath) =>
    isVersionMetadataChangePath(change, filePath),
  );
  if (versionMetadataFiles.length === 0 || change.files.includes(CHANGELOG_PATH)) {
    return null;
  }

  const subject = messageSubject(change.message);
  return {
    label: change.label,
    subject: subject || "(no message provided)",
    releaseLike: isReleaseLikeMessage(change.message),
    versionMetadataFiles,
    message:
      "version metadata changed without CHANGELOG.md; run release tooling and commit generated release metadata together",
  };
}

function printHuman(selector, issues) {
  if (issues.length === 0) {
    process.stdout.write(`release changelog coupling guard: ok (${selector})\n`);
    return;
  }

  process.stderr.write(
    `release changelog coupling guard: ${issues.length} split release metadata change(s)\n`,
  );
  for (const issue of issues) {
    process.stderr.write(`  - ${issue.label}: ${issue.subject}\n`);
    process.stderr.write(`    ${issue.message}\n`);
    process.stderr.write("    version metadata files:\n");
    for (const filePath of issue.versionMetadataFiles) {
      process.stderr.write(`      - ${filePath}\n`);
    }
  }
}

async function main() {
  const args = parseReleaseGuardArgs(process.argv, {
    allow: {
      staged: true,
      worktree: true,
      message: true,
      messageFile: true,
    },
  });
  if (args.help) {
    printHelp();
    return;
  }

  const { selector, changes } = await selectedChanges(args, { includeChangedLines: true });
  const issues = changes.map(issueForChange).filter(Boolean);

  if (args.json) {
    process.stdout.write(`${JSON.stringify({ selector, issues }, null, 2)}\n`);
  } else {
    printHuman(selector, issues);
  }

  if (issues.length > 0) {
    process.exitCode = 1;
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`release-changelog-coupling-guard: ${message}\n`);
  process.exitCode = 1;
}
