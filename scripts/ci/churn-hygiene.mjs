#!/usr/bin/env node
import { fileMatchesAnyPattern, git, parseNumstat, parsePositiveInteger } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const DEFAULT_THRESHOLDS = Object.freeze({
  maxFiles: 35,
  maxBehaviorFiles: 25,
  maxLines: 1200,
  maxFileLines: 500,
});

const BEHAVIOR_PATTERNS = Object.freeze([
  ".github/workflows/**",
  "benches/**",
  "npm/**",
  "scripts/**",
  "src/**",
  "tests/**",
  "Cargo.toml",
  "Cargo.lock",
  "package.json",
]);

function parseArgs(argv) {
  const args = {
    check: false,
    dryRun: false,
    json: false,
    thresholds: { ...DEFAULT_THRESHOLDS },
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
    if (value === "--staged") {
      args.staged = true;
      continue;
    }
    if (value === "--worktree") {
      args.worktree = true;
      continue;
    }
    if (value === "--check") {
      args.check = true;
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--max-files") {
      index += 1;
      args.thresholds.maxFiles = parsePositiveInteger(requiredValue(argv[index], value), value);
      continue;
    }
    if (value === "--max-behavior-files") {
      index += 1;
      args.thresholds.maxBehaviorFiles = parsePositiveInteger(requiredValue(argv[index], value), value);
      continue;
    }
    if (value === "--max-lines") {
      index += 1;
      args.thresholds.maxLines = parsePositiveInteger(requiredValue(argv[index], value), value);
      continue;
    }
    if (value === "--max-file-lines") {
      index += 1;
      args.thresholds.maxFileLines = parsePositiveInteger(requiredValue(argv[index], value), value);
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
      "Usage: node scripts/ci/churn-hygiene.mjs [selector] [--check] [thresholds]",
      "",
      "Reports commit/diff churn against lightweight thresholds. Default mode never fails.",
      "",
      "Selectors:",
      "  --range <rev-range>       inspect a git range",
      "  --base <rev> --head <rev> inspect base..head",
      "  --staged                  inspect staged files",
      "  --worktree                inspect unstaged files",
      "",
      "Options:",
      "  --check                   fail when thresholds are exceeded",
      "  --dry-run                 print selected diff command and thresholds only",
      "  --max-files <n>           default 35",
      "  --max-behavior-files <n>  default 25",
      "  --max-lines <n>           default 1200",
      "  --max-file-lines <n>      default 500",
      "  --json                    print machine-readable result",
      "",
      "Default selector: HEAD~1..HEAD when available, otherwise HEAD",
    ].join("\n") + "\n",
  );
}

function assertSingleSelector(args) {
  const selectors = [
    Boolean(args.range),
    Boolean(args.base || args.head),
    Boolean(args.staged),
    Boolean(args.worktree),
  ].filter(Boolean).length;
  if (selectors > 1) {
    throw new Error("choose only one selector: --range, --base/--head, --staged, or --worktree");
  }
  if ((args.base || args.head) && !(args.base && args.head)) {
    throw new Error("--base and --head must be used together");
  }
}

async function defaultRangeAvailable() {
  try {
    await git(["rev-parse", "--verify", "HEAD~1"], { cwd: repoRoot });
    return true;
  } catch {
    return false;
  }
}

async function diffPlan(args) {
  assertSingleSelector(args);
  if (args.range || (args.base && args.head)) {
    const range = args.range ?? `${args.base}..${args.head}`;
    return {
      selector: range,
      command: ["diff", "--numstat", "--diff-filter=ACMR", range],
    };
  }
  if (args.staged) {
    return {
      selector: "staged",
      command: ["diff", "--cached", "--numstat", "--diff-filter=ACMR"],
    };
  }
  if (args.worktree) {
    return {
      selector: "worktree",
      command: ["diff", "--numstat", "--diff-filter=ACMR"],
    };
  }
  if (await defaultRangeAvailable()) {
    return {
      selector: "HEAD~1..HEAD",
      command: ["diff", "--numstat", "--diff-filter=ACMR", "HEAD~1..HEAD"],
    };
  }
  return {
    selector: "HEAD",
    command: ["show", "--numstat", "--format=", "--diff-filter=ACMR", "HEAD"],
  };
}

function summarize(rows) {
  const files = rows.length;
  const behaviorRows = rows.filter((row) => fileMatchesAnyPattern(row.filePath, BEHAVIOR_PATTERNS));
  const insertions = rows.reduce((sum, row) => sum + row.insertions, 0);
  const deletions = rows.reduce((sum, row) => sum + row.deletions, 0);
  const changedLines = insertions + deletions;
  const largestFiles = [...rows]
    .map((row) => ({
      filePath: row.filePath,
      lines: row.insertions + row.deletions,
      binary: row.binary,
    }))
    .sort((left, right) => right.lines - left.lines || left.filePath.localeCompare(right.filePath))
    .slice(0, 5);
  return {
    files,
    behaviorFiles: behaviorRows.length,
    insertions,
    deletions,
    changedLines,
    largestFiles,
    binaryFiles: rows.filter((row) => row.binary).map((row) => row.filePath),
  };
}

function thresholdIssues(summary, thresholds) {
  const issues = [];
  if (summary.files > thresholds.maxFiles) {
    issues.push(`files changed ${summary.files} > ${thresholds.maxFiles}`);
  }
  if (summary.behaviorFiles > thresholds.maxBehaviorFiles) {
    issues.push(`behavior files ${summary.behaviorFiles} > ${thresholds.maxBehaviorFiles}`);
  }
  if (summary.changedLines > thresholds.maxLines) {
    issues.push(`changed lines ${summary.changedLines} > ${thresholds.maxLines}`);
  }
  const largest = summary.largestFiles[0];
  if (largest && largest.lines > thresholds.maxFileLines) {
    issues.push(`largest file ${largest.filePath} changed ${largest.lines} lines > ${thresholds.maxFileLines}`);
  }
  return issues;
}

function printHuman(selector, command, summary, thresholds, issues, check) {
  process.stdout.write(`churn hygiene: ${selector}\n`);
  process.stdout.write(`  command: git ${command.join(" ")}\n`);
  process.stdout.write(`  files changed: ${summary.files} (threshold ${thresholds.maxFiles})\n`);
  process.stdout.write(`  behavior files: ${summary.behaviorFiles} (threshold ${thresholds.maxBehaviorFiles})\n`);
  process.stdout.write(`  changed lines: ${summary.changedLines} (threshold ${thresholds.maxLines})\n`);
  if (summary.largestFiles.length > 0) {
    process.stdout.write("  largest files:\n");
    for (const file of summary.largestFiles) {
      process.stdout.write(`    - ${file.filePath}: ${file.binary ? "binary" : `${file.lines} lines`}\n`);
    }
  }
  if (issues.length > 0) {
    process.stdout.write(`  threshold warnings: ${issues.join("; ")}\n`);
  }
  process.stdout.write(`  mode: ${check ? "check" : "report-only"}\n`);
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const plan = await diffPlan(args);
  if (args.dryRun) {
    process.stdout.write(`dry-run: churn hygiene would run git ${plan.command.join(" ")}\n`);
    process.stdout.write(`dry-run: thresholds ${JSON.stringify(args.thresholds)}\n`);
    return;
  }

  const { stdout } = await git(plan.command, { cwd: repoRoot });
  const rows = parseNumstat(stdout);
  const summary = summarize(rows);
  const issues = thresholdIssues(summary, args.thresholds);

  if (args.json) {
    process.stdout.write(
      `${JSON.stringify(
        {
          selector: plan.selector,
          command: ["git", ...plan.command],
          thresholds: args.thresholds,
          summary,
          issues,
          check: args.check,
        },
        null,
        2,
      )}\n`,
    );
  } else {
    printHuman(plan.selector, plan.command, summary, args.thresholds, issues, args.check);
  }

  if (args.check && issues.length > 0) {
    process.exitCode = 1;
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`churn-hygiene: ${message}\n`);
  process.exitCode = 1;
}
