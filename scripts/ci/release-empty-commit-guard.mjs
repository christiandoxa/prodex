#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { git, normalizeGitPath } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const RELEASE_MESSAGE_PATTERNS = Object.freeze([
  /^release(?:\([^)]*\))?!?:/i,
  /^release\s+v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/i,
  /^chore(?:\([^)]*\))?!?:\s*release\b/i,
  /^chore\(release\)!?:/i,
  /^bump(?:\([^)]*\))?!?:\s*(?:prodex\s+)?v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/i,
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
    if (value === "--staged") {
      args.staged = true;
      continue;
    }
    if (value === "--worktree") {
      args.worktree = true;
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
  const selectors = [
    Boolean(args.range),
    Boolean(args.base || args.head),
    Boolean(args.commit),
    Boolean(args.staged),
    Boolean(args.worktree),
  ].filter(Boolean).length;
  if (selectors > 1) {
    throw new Error(
      "choose only one selector: --range, --base/--head, --commit, --staged, or --worktree",
    );
  }
  if ((args.base || args.head) && !(args.base && args.head)) {
    throw new Error("--base and --head must be used together");
  }
  if (args.message && args.messageFile) {
    throw new Error("choose only one message source: --message or --message-file");
  }
}

function isReleaseLikeMessage(message) {
  const subject = message.split(/\r?\n/, 1)[0]?.trim() ?? "";
  return RELEASE_MESSAGE_PATTERNS.some((pattern) => pattern.test(subject));
}

async function messageFromArgs(args, fallback) {
  if (args.messageFile) {
    return fs.readFile(path.resolve(repoRoot, args.messageFile), "utf8");
  }
  return args.message ?? fallback;
}

async function commitMessage(rev) {
  const { stdout } = await git(["log", "-1", "--format=%B", rev], { cwd: repoRoot });
  return stdout.trimEnd();
}

async function commitFiles(rev) {
  const { stdout } = await git(["diff-tree", "--root", "--no-commit-id", "--name-only", "-r", rev], {
    cwd: repoRoot,
  });
  return stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map(normalizeGitPath);
}

async function rangeCommits(range) {
  const { stdout } = await git(["rev-list", "--reverse", range], { cwd: repoRoot });
  return stdout.split(/\r?\n/).filter(Boolean);
}

async function changedFilesForSynthetic(staged) {
  const diffArgs = staged ? ["diff", "--cached", "--name-only"] : ["diff", "--name-only"];
  const { stdout } = await git(diffArgs, { cwd: repoRoot });
  const files = stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map(normalizeGitPath);

  if (!staged) {
    const { stdout: untrackedStdout } = await git(["ls-files", "--others", "--exclude-standard"], {
      cwd: repoRoot,
    });
    files.push(
      ...untrackedStdout
        .split(/\r?\n/)
        .filter(Boolean)
        .map(normalizeGitPath),
    );
  }

  return [...new Set(files)].sort();
}

async function selectedChanges(args) {
  assertValidArgs(args);

  if (args.range || (args.base && args.head)) {
    const range = args.range ?? `${args.base}..${args.head}`;
    const commits = await rangeCommits(range);
    const changes = [];
    for (const rev of commits) {
      changes.push({
        label: rev,
        message: await messageFromArgs(args, await commitMessage(rev)),
        files: await commitFiles(rev),
      });
    }
    return { selector: range, changes };
  }

  if (args.staged || args.worktree) {
    const staged = Boolean(args.staged);
    return {
      selector: staged ? "staged" : "worktree",
      changes: [
        {
          label: staged ? "staged" : "worktree",
          message: await messageFromArgs(args, ""),
          files: await changedFilesForSynthetic(staged),
        },
      ],
    };
  }

  const rev = args.commit ?? "HEAD";
  return {
    selector: rev,
    changes: [
      {
        label: rev,
        message: await messageFromArgs(args, await commitMessage(rev)),
        files: await commitFiles(rev),
      },
    ],
  };
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

  const { selector, changes } = await selectedChanges(args);
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
