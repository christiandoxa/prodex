#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { fileMatchesAnyPattern, git, parseNumstat, parsePositiveInteger } from "./guard-common.mjs";
import { RELEASE_METADATA_CHURN_PATTERNS } from "./test-impact-manifest.mjs";
import { repoRoot } from "../npm/common.mjs";

const DEFAULT_THRESHOLDS = Object.freeze({
  maxFiles: 35,
  maxBehaviorFiles: 25,
  maxLines: 1200,
  maxFileLines: 500,
});
const STRUCTURAL_EXTRACTION_INCIDENTAL_LINE_MULTIPLIER = 2;
const MECHANICAL_ONLY_PATTERN =
  /(?:^|\n)\s*Mechanical-only:\s*(?:yes|true)\s*(?:\n|$)|\[mechanical-only\]/i;

const BEHAVIOR_PATTERNS = Object.freeze([
  ".github/workflows/**",
  "benches/**",
  "crates/**",
  "npm/**",
  "scripts/**",
  "src/**",
  "tests/**",
  "Cargo.toml",
  "Cargo.lock",
  "package.json",
]);

const VERSION_TAG_PATTERN = /^v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/;
const LATEST_TAG_BASELINE_ALIASES = new Set(["latest-tag", "latest-version-tag", "latest-release-tag"]);

function parseArgs(argv) {
  const args = {
    check: process.env.PRODEX_CHURN_HYGIENE_REPORT_ONLY !== "1",
    dryRun: false,
    ignoreBefore: process.env.PRODEX_CHURN_HYGIENE_IGNORE_BEFORE || null,
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
    if (value === "--ignore-before") {
      index += 1;
      args.ignoreBefore = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--check") {
      args.check = true;
      continue;
    }
    if (value === "--report-only") {
      args.check = false;
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
      "Usage: node scripts/ci/churn-hygiene.mjs [selector] [--check|--report-only] [thresholds]",
      "",
      "Reports commit/diff churn and generic commit subjects against lightweight thresholds.",
      "Default mode fails on actionable violations. Use --report-only or PRODEX_CHURN_HYGIENE_REPORT_ONLY=1 for exploratory dry runs.",
      "",
      "Selectors:",
      "  --range <rev-range>       inspect a git range",
      "  --base <rev> --head <rev> inspect base..head",
      "  --staged                  inspect staged files",
      "  --worktree                inspect unstaged files",
      "",
      "Options:",
      "  --check                   fail when thresholds are exceeded; default unless report-only env is set",
      "  --report-only             report violations without failing",
      "  --ignore-before <rev>     for explicit historical --range audits, enforce only changes after this reviewed baseline",
      "                            use latest-tag to select the newest version tag inside the range",
      "  --dry-run                 print selected diff command and thresholds only",
      "  --max-files <n>           default 35",
      "  --max-behavior-files <n>  default 25",
      "  --max-lines <n>           default 1200",
      "  --max-file-lines <n>      default 500",
      "  --message <subject/body>  staged/worktree commit message for structural extraction review",
      "  --message-file <path>     read staged/worktree commit message from a file",
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

function validateIgnoreBeforeSelector(args) {
  if (!args.ignoreBefore) {
    return;
  }
  if (args.range) {
    return;
  }
  if (args.base || args.head) {
    throw new Error(
      "--ignore-before is only supported with --range; --base/--head checks must enforce the full PR/push range",
    );
  }
  throw new Error("--ignore-before requires --range");
}

function parseSimpleRange(range) {
  if (range.includes("...")) {
    return null;
  }
  const parts = range.split("..");
  if (parts.length !== 2 || !parts[0]) {
    return null;
  }
  return {
    base: parts[0],
    head: parts[1] || "HEAD",
  };
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
  validateIgnoreBeforeSelector(args);
  let originalSelector = null;
  let effectiveSelector = null;
  let effectiveBase = null;
  let effectiveHead = null;

  if (args.range || (args.base && args.head)) {
    const range = args.range ?? `${args.base}..${args.head}`;
    const parsed = args.range ? parseSimpleRange(args.range) : { base: args.base, head: args.head };
    if (args.ignoreBefore) {
      if (!parsed) {
        throw new Error("--ignore-before requires a simple two-dot range such as --range base..head");
      }
      const resolvedIgnoreBefore = await resolveIgnoreBefore(args.ignoreBefore, parsed.base, parsed.head);
      originalSelector = range;
      effectiveBase = resolvedIgnoreBefore.value;
      effectiveHead = parsed.head;
      effectiveSelector = `${effectiveBase}..${effectiveHead}`;
      await validateIgnoreBefore(parsed.base, effectiveBase, effectiveHead);
    }
    return {
      selector: effectiveSelector ?? range,
      originalSelector,
      ignoreBefore: effectiveBase,
      ignoreBeforeInput: args.ignoreBefore,
      command: ["diff", "--numstat", "--diff-filter=ACMR", effectiveSelector ?? range],
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

async function mergeBaseIsAncestor(ancestor, descendant) {
  try {
    await git(["merge-base", "--is-ancestor", ancestor, descendant], { cwd: repoRoot });
    return true;
  } catch {
    return false;
  }
}

async function resolveIgnoreBefore(ignoreBefore, rangeBase, head) {
  if (!LATEST_TAG_BASELINE_ALIASES.has(ignoreBefore)) {
    return { value: ignoreBefore };
  }

  const { stdout } = await git(["tag", "--merged", head, "--sort=-version:refname"], { cwd: repoRoot });
  const versionTags = stdout
    .split(/\r?\n/)
    .map((tag) => tag.trim())
    .filter((tag) => VERSION_TAG_PATTERN.test(tag));

  for (const tag of versionTags) {
    if ((await mergeBaseIsAncestor(rangeBase, tag)) && (await mergeBaseIsAncestor(tag, head))) {
      return { value: tag };
    }
  }

  throw new Error(`--ignore-before ${ignoreBefore} found no version tag within ${rangeBase}..${head}`);
}

async function validateIgnoreBefore(rangeBase, ignoreBefore, head) {
  if (!(await mergeBaseIsAncestor(rangeBase, ignoreBefore))) {
    throw new Error(`--ignore-before ${ignoreBefore} is not within the selected range after ${rangeBase}`);
  }
  if (!(await mergeBaseIsAncestor(ignoreBefore, head))) {
    throw new Error(`--ignore-before ${ignoreBefore} is not an ancestor of ${head}`);
  }
}

function summarize(rows) {
  const files = rows.length;
  const behaviorRows = rows.filter((row) => fileMatchesAnyPattern(row.filePath, BEHAVIOR_PATTERNS));
  const releaseMetadataOnly =
    rows.length > 0 &&
    rows.every((row) => fileMatchesAnyPattern(row.filePath, RELEASE_METADATA_CHURN_PATTERNS));
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
    releaseMetadataOnly,
  };
}

function structuralGroup(filePath) {
  const parts = filePath.split("/");
  const fileName = parts.at(-1) ?? "";
  const stem = fileName.endsWith(".rs") ? fileName.slice(0, -".rs".length) : null;
  if (parts[0] === "crates" && parts[1]) {
    return `crates/${parts[1]}`;
  }
  // Treat Rust module extraction as one group, e.g. src/foo.rs -> src/foo/*.rs.
  if (stem && parts.length === 2 && parts[0] === "src") {
    return `src/${stem}`;
  }
  if (parts[0] === "tests" && parts[1]) {
    if ((parts[1] === "src" || parts[1] === "support") && parts[2]) {
      const root = parts.length === 3 && stem ? stem : parts[2];
      return `tests/${parts[1]}/${root}`;
    }
    const rootTest = parts[1].endsWith(".rs") ? parts[1].slice(0, -".rs".length) : parts[1];
    return `tests/${rootTest}`;
  }
  return parts.slice(0, -1).join("/") || ".";
}

function mostlyOneWayChange(insertions, deletions) {
  const smaller = Math.min(insertions, deletions);
  const larger = Math.max(insertions, deletions);
  return smaller <= 50 || smaller <= Math.floor(larger * 0.35);
}

function structuralExtractionGroups(rows, thresholds) {
  const groups = new Map();
  for (const row of rows) {
    if (row.binary || !mostlyOneWayChange(row.insertions, row.deletions)) {
      continue;
    }
    const group = structuralGroup(row.filePath);
    const aggregate = groups.get(group) ?? {
      insertionLines: 0,
      deletionLines: 0,
      largeDeletion: false,
    };
    aggregate.insertionLines += row.insertions;
    aggregate.deletionLines += row.deletions;
    aggregate.largeDeletion ||= row.deletions > thresholds.maxFileLines;
    groups.set(group, aggregate);
  }
  return [...groups.entries()]
    .filter(
      ([, aggregate]) =>
        aggregate.largeDeletion &&
        aggregate.deletionLines > thresholds.maxFileLines &&
        aggregate.insertionLines > thresholds.maxFileLines,
    )
    .map(([group]) => group);
}

function structuralExtractionApplies(rows, summary, thresholds) {
  const groups = new Set(structuralExtractionGroups(rows, thresholds));
  if (groups.size === 0) {
    return false;
  }

  const nonExtractionRows = rows.filter((row) => !groups.has(structuralGroup(row.filePath)));
  const nonExtractionSummary = summarize(nonExtractionRows);
  const nonExtractionLines = nonExtractionRows
    .reduce((sum, row) => sum + row.insertions + row.deletions, 0);
  const nonExtractionLineLimit =
    thresholds.maxLines * STRUCTURAL_EXTRACTION_INCIDENTAL_LINE_MULTIPLIER;
  const nonExtractionLargest = nonExtractionRows
    .filter((row) => !row.binary)
    .reduce((max, row) => Math.max(max, row.insertions + row.deletions), 0);
  const largest = summary.largestFiles[0];
  return (
    nonExtractionSummary.files <= thresholds.maxFiles &&
    nonExtractionSummary.behaviorFiles <= thresholds.maxBehaviorFiles &&
    nonExtractionLines <= nonExtractionLineLimit &&
    nonExtractionLargest <= thresholds.maxFileLines &&
    largest &&
    groups.has(structuralGroup(largest.filePath))
  );
}

function thresholdIssues(summary, thresholds, options = {}) {
  const issues = [];
  const structuralExtractionAccepted =
    options.structuralExtractionAccepted ?? options.structuralExtraction ?? false;
  if (summary.files > thresholds.maxFiles && !summary.releaseMetadataOnly && !structuralExtractionAccepted) {
    issues.push(`files changed ${summary.files} > ${thresholds.maxFiles}`);
  }
  if (summary.behaviorFiles > thresholds.maxBehaviorFiles && !structuralExtractionAccepted) {
    issues.push(`behavior files ${summary.behaviorFiles} > ${thresholds.maxBehaviorFiles}`);
  }
  if (summary.changedLines > thresholds.maxLines && !structuralExtractionAccepted) {
    issues.push(`changed lines ${summary.changedLines} > ${thresholds.maxLines}`);
  }
  const largest = summary.largestFiles[0];
  if (largest && largest.lines > thresholds.maxFileLines && !structuralExtractionAccepted) {
    issues.push(`largest file ${largest.filePath} changed ${largest.lines} lines > ${thresholds.maxFileLines}`);
  }
  return issues;
}

function mechanicalOnlyDeclared(message) {
  return MECHANICAL_ONLY_PATTERN.test(message ?? "");
}

function structuralExtractionNeedsDeclaration(summary, thresholds, structuralExtraction) {
  return (
    structuralExtraction &&
    thresholdIssues(summary, thresholds, { structuralExtractionAccepted: false }).length > 0
  );
}

function structuralExtractionAcceptedByDeclaration(structuralExtraction, summary, thresholds, commits) {
  if (!structuralExtraction) {
    return false;
  }
  if (!structuralExtractionNeedsDeclaration(summary, thresholds, structuralExtraction)) {
    return true;
  }
  return commits.some((commit) => {
    const parsed = parseConventionalSubject(commit.subject ?? messageSubject(commit.message ?? ""));
    return mechanicalOnlyDeclared(commit.message) || mechanicalOnlySubjectAllowed(parsed);
  });
}

function structuralExtractionDeclarationIssues(
  summary,
  thresholds,
  structuralExtraction,
  structuralExtractionAccepted,
  commits,
) {
  if (
    !structuralExtractionNeedsDeclaration(summary, thresholds, structuralExtraction) ||
    structuralExtractionAccepted
  ) {
    return [];
  }
  const target = commits.length > 0 ? "commit message" : "staged/worktree commit message";
  return [
    `large structural extraction requires a mechanical extract/split/move subject or Mechanical-only: yes in the ${target}`,
  ];
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

function messageSubject(message) {
  return message.split(/\r?\n/, 1)[0]?.trim() ?? "";
}

async function messageOverride(args) {
  if (args.messageFile) {
    return fs.readFile(path.resolve(repoRoot, args.messageFile), "utf8");
  }
  return args.message;
}

function genericCommitTitle(title) {
  const normalized = title.toLowerCase().replace(/\s+/g, " ").trim();
  return [
    /^(?:improve|optimize|reduce|trim|tighten)\s+(?:default\s+)?(?:embedded\s+)?(?:smart context\s+)?(?:context\s+)?token\s+(?:efficiency|usage|overhead|budgets|calibration|compaction)$/,
    /^(?:improve|optimize|reduce|trim|tighten)\s+(?:prodex\s+)?super\s+token\s+(?:efficiency|overhead)$/,
    /^(?:improve|optimize|reduce|trim|tighten)\s+(?:embedded|context|smart context|super)\s+token\s+(?:efficiency|overhead|usage)$/,
  ].some((pattern) => pattern.test(normalized));
}

function mechanicalOnlySubjectAllowed(parsed) {
  if (!["chore", "ci", "refactor", "test"].includes(parsed.type)) {
    return false;
  }
  return /\b(?:extract|mechanical|move|relocat|rename|reshape|shard|split)\b/i.test(parsed.title);
}

function broadCommitScope(scope) {
  return !scope || ["runtime", "super", "cli", "misc"].includes(scope);
}

function normalizedSubjectTitle(title) {
  return title
    .toLowerCase()
    .replace(/\b(?:default|embedded|smart|context|prodex|super|token)\b/g, " ")
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

function commitSubjectIssues(commits) {
  const issues = [];
  const titles = new Map();
  for (const commit of commits) {
    const parsed = parseConventionalSubject(commit.subject);
    if (genericCommitTitle(parsed.title) && broadCommitScope(parsed.scope)) {
      issues.push(
        `${commit.hash.slice(0, 7)} generic subject needs narrower scope/title: ${commit.subject}`,
      );
    }
    if (mechanicalOnlyDeclared(commit.message) && !mechanicalOnlySubjectAllowed(parsed)) {
      issues.push(
        `${commit.hash.slice(0, 7)} Mechanical-only declaration needs a mechanical refactor/test/chore subject: ${commit.subject}`,
      );
    }
    const normalized = normalizedSubjectTitle(parsed.title);
    if (normalized) {
      const bucket = titles.get(normalized) ?? [];
      bucket.push(commit);
      titles.set(normalized, bucket);
    }
  }

  for (const bucket of titles.values()) {
    if (bucket.length < 3) {
      continue;
    }
    issues.push(
      `repeated similar subjects (${bucket.length}): ${bucket
        .map((commit) => `${commit.hash.slice(0, 7)} ${commit.subject}`)
        .join("; ")}`,
    );
  }
  return issues;
}

async function commitsForSelector(selector, options = {}) {
  if (selector === "staged" || selector === "worktree") {
    const message = await messageOverride(options);
    return message
      ? [
          {
            hash: selector,
            subject: messageSubject(message),
            message,
          },
        ]
      : [];
  }
  const gitArgs =
    selector === "HEAD"
      ? ["log", "-1", "--format=%H%x00%s%x00%B%x1e", "HEAD"]
      : ["log", "--format=%H%x00%s%x00%B%x1e", selector];
  const { stdout } = await git(gitArgs, { cwd: repoRoot });
  return stdout
    .split("\x1e")
    .filter((record) => record.trim())
    .map((record) => {
      const [hash, subject, ...messageParts] = record.replace(/^\n|\n$/g, "").split("\0");
      return {
        hash,
        subject: subject ?? "",
        message: messageParts.join("\0"),
      };
    });
}

function printHuman(
  plan,
  summary,
  thresholds,
  issues,
  declarationIssues,
  subjectIssues,
  check,
  options = {},
) {
  const { selector, command, originalSelector, ignoreBefore, ignoreBeforeInput } = plan;
  process.stdout.write(`churn hygiene: ${selector}\n`);
  if (originalSelector && ignoreBefore) {
    process.stdout.write(`  original range: ${originalSelector}\n`);
    const suffix = ignoreBeforeInput && ignoreBeforeInput !== ignoreBefore ? ` (from ${ignoreBeforeInput})` : "";
    process.stdout.write(`  ignored baseline: ${ignoreBefore}${suffix}\n`);
  }
  process.stdout.write(`  command: git ${command.join(" ")}\n`);
  process.stdout.write(`  files changed: ${summary.files} (threshold ${thresholds.maxFiles})\n`);
  process.stdout.write(`  behavior files: ${summary.behaviorFiles} (threshold ${thresholds.maxBehaviorFiles})\n`);
  process.stdout.write(`  changed lines: ${summary.changedLines} (threshold ${thresholds.maxLines})\n`);
  if (summary.releaseMetadataOnly) {
    process.stdout.write("  release metadata only: yes\n");
  }
  if (options.structuralExtraction) {
    const suffix = options.structuralExtractionAccepted ? "accepted" : "needs declaration";
    process.stdout.write(`  structural extraction: yes (${suffix})\n`);
  }
  if (summary.largestFiles.length > 0) {
    process.stdout.write("  largest files:\n");
    for (const file of summary.largestFiles) {
      process.stdout.write(`    - ${file.filePath}: ${file.binary ? "binary" : `${file.lines} lines`}\n`);
    }
  }
  if (issues.length > 0) {
    process.stdout.write(`  threshold warnings: ${issues.join("; ")}\n`);
    process.stdout.write(
      "  guidance: split broad structural work into narrower commits or reduce non-extraction churn\n",
    );
  }
  if (declarationIssues.length > 0) {
    process.stdout.write("  mechanical-only warnings:\n");
    for (const issue of declarationIssues) {
      process.stdout.write(`    - ${issue}\n`);
    }
  }
  if (subjectIssues.length > 0) {
    process.stdout.write("  subject warnings:\n");
    for (const issue of subjectIssues) {
      process.stdout.write(`    - ${issue}\n`);
    }
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
    if (plan.originalSelector && plan.ignoreBefore) {
      const suffix =
        plan.ignoreBeforeInput && plan.ignoreBeforeInput !== plan.ignoreBefore
          ? ` (from ${plan.ignoreBeforeInput})`
          : "";
      process.stdout.write(
        `dry-run: original range ${plan.originalSelector}; ignored baseline ${plan.ignoreBefore}${suffix}\n`,
      );
    }
    process.stdout.write(`dry-run: thresholds ${JSON.stringify(args.thresholds)}\n`);
    return;
  }

  const { stdout } = await git(plan.command, { cwd: repoRoot });
  const rows = parseNumstat(stdout);
  const summary = summarize(rows);
  const structuralExtraction = structuralExtractionApplies(rows, summary, args.thresholds);
  const commits = await commitsForSelector(plan.selector, args);
  const structuralExtractionAccepted = structuralExtractionAcceptedByDeclaration(
    structuralExtraction,
    summary,
    args.thresholds,
    commits,
  );
  const issues = thresholdIssues(summary, args.thresholds, {
    structuralExtractionAccepted,
  });
  const declarationIssues = structuralExtractionDeclarationIssues(
    summary,
    args.thresholds,
    structuralExtraction,
    structuralExtractionAccepted,
    commits,
  );
  const subjectIssues = commitSubjectIssues(commits);
  const checkIssues = [...issues, ...declarationIssues, ...subjectIssues];

  if (args.json) {
    process.stdout.write(
      `${JSON.stringify(
        {
          selector: plan.selector,
          originalSelector: plan.originalSelector,
          ignoreBefore: plan.ignoreBefore,
          ignoreBeforeInput: plan.ignoreBeforeInput,
          command: ["git", ...plan.command],
          thresholds: args.thresholds,
          summary,
          structuralExtraction,
          structuralExtractionAccepted,
          issues,
          declarationIssues,
          commitSubjects: commits.map((commit) => ({
            hash: commit.hash,
            subject: commit.subject,
            mechanicalOnly: mechanicalOnlyDeclared(commit.message),
          })),
          subjectIssues,
          check: args.check,
        },
        null,
        2,
      )}\n`,
    );
  } else {
    printHuman(plan, summary, args.thresholds, issues, declarationIssues, subjectIssues, args.check, {
      structuralExtraction,
      structuralExtractionAccepted,
    });
  }

  if (args.check && checkIssues.length > 0) {
    process.exitCode = 1;
  }
}

export {
  DEFAULT_THRESHOLDS,
  mechanicalOnlyDeclared,
  structuralExtractionApplies,
  structuralExtractionAcceptedByDeclaration,
  structuralExtractionDeclarationIssues,
  structuralExtractionGroups,
  structuralGroup,
  summarize,
  thresholdIssues,
};

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    await main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`churn-hygiene: ${message}\n`);
    process.exitCode = 1;
  }
}
