#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileMatchesAnyPattern, git, normalizeGitPath } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const VERSION_METADATA_PATTERNS = Object.freeze([
  "Cargo.toml",
  "Cargo.lock",
  "crates/*/Cargo.toml",
  "npm/prodex/package.json",
  "npm/platforms/*/package.json",
  "package-lock.json",
  "npm/package-lock.json",
  "npm/prodex/package-lock.json",
  "npm/platforms/*/package-lock.json",
  "README.md",
  "QUICKSTART.md",
  "CHANGELOG.md",
]);

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

function isVersionMetadataPath(filePath) {
  return fileMatchesAnyPattern(filePath, VERSION_METADATA_PATTERNS);
}

function isReleaseLikeMessage(message) {
  const subject = message.split(/\r?\n/, 1)[0]?.trim() ?? "";
  return RELEASE_MESSAGE_PATTERNS.some((pattern) => pattern.test(subject));
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

async function messageOverride(args) {
  if (args.messageFile) {
    return fs.readFile(path.resolve(repoRoot, args.messageFile), "utf8");
  }
  return args.message;
}

async function changedFilesForSynthetic(args) {
  const diffArgs = args.staged ? ["diff", "--cached", "--name-only"] : ["diff", "--name-only"];
  const { stdout } = await git(diffArgs, { cwd: repoRoot });
  const files = stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map(normalizeGitPath);

  if (args.includeUntracked) {
    const untracked = await git(["ls-files", "--others", "--exclude-standard"], { cwd: repoRoot });
    files.push(
      ...untracked.stdout
        .split(/\r?\n/)
        .filter(Boolean)
        .map(normalizeGitPath),
    );
  }

  return [...new Set(files)].sort();
}

async function selectedChanges(args) {
  assertSingleSelector(args);
  const override = await messageOverride(args);

  if (args.range || (args.base && args.head)) {
    const range = args.range ?? `${args.base}..${args.head}`;
    const commits = await rangeCommits(range);
    const changes = [];
    for (const rev of commits) {
      changes.push({
        label: rev,
        message: override ?? (await commitMessage(rev)),
        files: await commitFiles(rev),
      });
    }
    return { selector: range, changes };
  }

  if (args.staged || args.worktree) {
    return {
      selector: args.staged ? "staged" : "worktree",
      changes: [
        {
          label: args.staged ? "staged" : "worktree",
          message: override ?? "",
          files: await changedFilesForSynthetic(args),
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
        message: override ?? (await commitMessage(rev)),
        files: await commitFiles(rev),
      },
    ],
  };
}

function evaluateChange(change) {
  const metadataFiles = change.files.filter(isVersionMetadataPath);
  const nonMetadataFiles = change.files.filter((filePath) => !isVersionMetadataPath(filePath));
  const releaseLike = isReleaseLikeMessage(change.message);
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
  process.stderr.write(`version-metadata-release-guard: ${message}\n`);
  process.exitCode = 1;
}
