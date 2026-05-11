#!/usr/bin/env node
import {
  isReleaseLikeMessage,
  messageSubject,
  parseReleaseGuardArgs,
  selectedChanges,
} from "./release-guard-common.mjs";

const CHANGELOG_PATH = "CHANGELOG.md";
const CHANGELOG_REFRESH_ACTION_PATTERN =
  /\b(?:add|generate|regenerate|refresh|render|sync|update|prepare|reopen|record)\b/i;

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/changelog-noise-guard.mjs [selector]",
      "",
      "Fails changelog-only refresh commits outside release commits.",
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

function parseConventionalSubject(subject) {
  const match = subject.match(/^([a-z]+)(?:\(([^)]+)\))?!?:\s*(.+)$/i);
  if (!match) {
    return {
      type: null,
      scope: null,
      title: subject.trim(),
    };
  }
  return {
    type: match[1].toLowerCase(),
    scope: (match[2] ?? "").trim().toLowerCase() || null,
    title: match[3].trim(),
  };
}

function isChangelogRefreshSubject(subject) {
  const parsed = parseConventionalSubject(subject);
  if (parsed.scope !== "changelog" || !["chore", "docs"].includes(parsed.type)) {
    return false;
  }
  return CHANGELOG_REFRESH_ACTION_PATTERN.test(parsed.title);
}

function isChangelogOnly(files) {
  return files.length === 1 && files[0] === CHANGELOG_PATH;
}

function issueForChange(change) {
  const subject = messageSubject(change.message);
  if (!subject || !isChangelogOnly(change.files)) {
    return null;
  }
  if (isReleaseLikeMessage(change.message) || !isChangelogRefreshSubject(subject)) {
    return null;
  }
  return {
    label: change.label,
    subject,
    files: change.files,
    message:
      "changelog-only refresh commits are release noise; let npm run release:run render CHANGELOG.md in the release commit",
  };
}

function printHuman(selector, issues) {
  if (issues.length === 0) {
    process.stdout.write(`changelog-noise-guard: ok (${selector})\n`);
    return;
  }

  process.stderr.write(`changelog-noise-guard: ${issues.length} changelog-only noise commit(s)\n`);
  for (const issue of issues) {
    process.stderr.write(`  - ${issue.label}: ${issue.subject}\n`);
    process.stderr.write(`    ${issue.message}\n`);
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

  const { selector, changes } = await selectedChanges(args);
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
  process.stderr.write(`changelog-noise-guard: ${message}\n`);
  process.exitCode = 1;
}
