#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { git, normalizeGitPath, parsePositiveInteger } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const DEFAULT_PRODUCTION_LINE_LIMIT = 1100;
const DEFAULT_TEST_LINE_LIMIT = 1500;

const DEFAULT_ALLOWLIST = Object.freeze([]);

function envPositiveInteger(name, fallback) {
  const value = process.env[name];
  if (value === undefined || value === "") {
    return fallback;
  }
  return parsePositiveInteger(value, name);
}

function requiredValue(value, name) {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function parseAllow(value) {
  const separator = value.lastIndexOf(":");
  if (separator <= 0 || separator === value.length - 1) {
    throw new Error("--allow expects <path>:<maxLines>");
  }

  return {
    file: normalizeGitPath(value.slice(0, separator)),
    maxLines: parsePositiveInteger(value.slice(separator + 1), "--allow maxLines"),
    reason: "command-line allowlist entry",
  };
}

function parseArgs(argv) {
  const args = {
    allowlist: [],
    json: false,
    productionLineLimit: envPositiveInteger(
      "PRODEX_SIZE_GUARD_PRODUCTION_LINES",
      DEFAULT_PRODUCTION_LINE_LIMIT,
    ),
    testLineLimit: envPositiveInteger("PRODEX_SIZE_GUARD_TEST_LINES", DEFAULT_TEST_LINE_LIMIT),
    useDefaultAllowlist: true,
    warnOnly: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--production-lines" || value === "--prod-lines") {
      index += 1;
      args.productionLineLimit = parsePositiveInteger(requiredValue(argv[index], value), value);
      continue;
    }
    if (value === "--test-lines") {
      index += 1;
      args.testLineLimit = parsePositiveInteger(requiredValue(argv[index], value), value);
      continue;
    }
    if (value === "--allow") {
      index += 1;
      args.allowlist.push(parseAllow(requiredValue(argv[index], value)));
      continue;
    }
    if (value === "--no-default-allowlist") {
      args.useDefaultAllowlist = false;
      continue;
    }
    if (value === "--warn-only") {
      args.warnOnly = true;
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

  if (args.testLineLimit <= args.productionLineLimit) {
    throw new Error("--test-lines must be higher than --production-lines");
  }

  return args;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/size-guard.mjs [options]",
      "",
      "Fails when Rust files exceed line-count limits.",
      "Production files use a lower limit; test and benchmark files use a higher limit.",
      "Allowlist entries are ratcheted: stale entries and oversized caps fail by default.",
      "",
      "Options:",
      "  --production-lines <n>  production Rust line limit",
      "  --prod-lines <n>        alias for --production-lines",
      "  --test-lines <n>        test/benchmark Rust line limit; must exceed production limit",
      "  --allow <path>:<n>      allow one file up to an exact current cap; may be repeated",
      "  --no-default-allowlist  ignore built-in caps for current hot spots",
      "  --warn-only             print violations but exit successfully",
      "  --json                  print machine-readable results",
      "",
      "Environment:",
      "  PRODEX_SIZE_GUARD_PRODUCTION_LINES",
      "  PRODEX_SIZE_GUARD_TEST_LINES",
    ].join("\n") + "\n",
  );
}

function rustFileKind(filePath) {
  const normalized = normalizeGitPath(filePath);
  if (
    normalized.startsWith("tests/") ||
    normalized.includes("/tests/") ||
    normalized.startsWith("benches/") ||
    normalized.includes("/benches/")
  ) {
    return "test";
  }
  return "production";
}

function countLines(contents) {
  if (contents.length === 0) {
    return 0;
  }
  const lines = contents.split(/\r\n|\n|\r/).length;
  return contents.endsWith("\n") || contents.endsWith("\r") ? lines - 1 : lines;
}

async function rustFiles() {
  const result = await git(["ls-files", "--cached", "--others", "--exclude-standard", "--", "*.rs"], {
    cwd: repoRoot,
  });
  return [
    ...new Set(
      result.stdout
        .split(/\r?\n/)
        .filter(Boolean)
        .map(normalizeGitPath),
    ),
  ].sort();
}

async function readExistingRustFile(filePath) {
  try {
    return await fs.readFile(path.join(repoRoot, filePath), "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") {
      return null;
    }
    throw error;
  }
}

function allowlistMap(entries) {
  const map = new Map();
  for (const entry of entries) {
    const file = normalizeGitPath(entry.file);
    if (map.has(file)) {
      throw new Error(`duplicate size allowlist entry: ${file}`);
    }
    map.set(file, { ...entry, file });
  }
  return map;
}

async function scan(args) {
  const entries = args.useDefaultAllowlist ? [...DEFAULT_ALLOWLIST, ...args.allowlist] : [...args.allowlist];
  const allowed = allowlistMap(entries);
  const files = [];
  const violations = [];
  const allowlistHits = [];
  const staleAllowlistEntries = [];
  const seenAllowlistFiles = new Set();

  for (const filePath of await rustFiles()) {
    const contents = await readExistingRustFile(filePath);
    if (contents === null) {
      continue;
    }

    const kind = rustFileKind(filePath);
    const lineCount = countLines(contents);
    const limit = kind === "test" ? args.testLineLimit : args.productionLineLimit;
    const file = { filePath, kind, lineCount, limit };
    files.push(file);

    const allowlistEntry = allowed.get(filePath);
    if (allowlistEntry) {
      seenAllowlistFiles.add(filePath);
    }

    if (allowlistEntry && lineCount <= limit) {
      staleAllowlistEntries.push({
        ...file,
        maxLines: allowlistEntry.maxLines,
        reason: allowlistEntry.reason,
        type: "allowlist-under-limit",
      });
      continue;
    }

    if (lineCount <= limit) {
      continue;
    }

    if (allowlistEntry) {
      if (lineCount < allowlistEntry.maxLines) {
        staleAllowlistEntries.push({
          ...file,
          maxLines: allowlistEntry.maxLines,
          reason: allowlistEntry.reason,
          type: "allowlist-cap-stale",
        });
        continue;
      }

      if (lineCount <= allowlistEntry.maxLines) {
        allowlistHits.push({ ...file, maxLines: allowlistEntry.maxLines, reason: allowlistEntry.reason });
        continue;
      }
    }

    violations.push({
      ...file,
      maxLines: allowlistEntry?.maxLines ?? null,
      reason: allowlistEntry?.reason ?? null,
      type: allowlistEntry ? "allowlist-exceeded" : "threshold-exceeded",
    });
  }

  for (const allowlistEntry of allowed.values()) {
    if (seenAllowlistFiles.has(allowlistEntry.file)) {
      continue;
    }
    staleAllowlistEntries.push({
      filePath: allowlistEntry.file,
      kind: null,
      lineCount: null,
      limit: null,
      maxLines: allowlistEntry.maxLines,
      reason: allowlistEntry.reason,
      type: "allowlist-missing",
    });
  }

  return { allowlistHits, files, staleAllowlistEntries, violations };
}

function printHuman(args, result) {
  const findingCount = result.violations.length + result.staleAllowlistEntries.length;
  if (findingCount === 0) {
    process.stdout.write(
      [
        `size guard: ok (${result.files.length} Rust file(s), ${result.allowlistHits.length} allowlist hit(s))`,
        `  production limit: ${args.productionLineLimit} lines`,
        `  test/benchmark limit: ${args.testLineLimit} lines`,
      ].join("\n") + "\n",
    );
    return;
  }

  const prefix = args.warnOnly ? "warning" : "violation";
  process.stderr.write(`size guard: ${findingCount} ${prefix}(s)\n`);
  for (const violation of result.violations) {
    if (violation.type === "allowlist-exceeded") {
      process.stderr.write(
        `${violation.filePath}: ${violation.lineCount} ${violation.kind} line(s), allowlist cap ${violation.maxLines}\n`,
      );
      continue;
    }
    process.stderr.write(
      `${violation.filePath}: ${violation.lineCount} ${violation.kind} line(s), limit ${violation.limit}\n`,
    );
  }
  for (const entry of result.staleAllowlistEntries) {
    if (entry.type === "allowlist-missing") {
      process.stderr.write(
        `${entry.filePath}: no tracked Rust file found; remove stale allowlist entry with cap ${entry.maxLines}\n`,
      );
      continue;
    }
    if (entry.type === "allowlist-under-limit") {
      process.stderr.write(
        `${entry.filePath}: ${entry.lineCount} ${entry.kind} line(s), normal limit ${entry.limit}; remove stale allowlist entry with cap ${entry.maxLines}\n`,
      );
      continue;
    }
    process.stderr.write(
      `${entry.filePath}: ${entry.lineCount} ${entry.kind} line(s), allowlist cap ${entry.maxLines}; lower cap to ${entry.lineCount} or split below normal limit ${entry.limit}\n`,
    );
  }
  process.stderr.write(
    "\nSplit large files, raise the configured limit deliberately, add a narrow allowlist cap with rationale, ratchet stale caps, or remove stale allowlist entries.\n",
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const result = await scan(args);
  if (args.json) {
    process.stdout.write(
      `${JSON.stringify(
        {
          limits: {
            production: args.productionLineLimit,
            test: args.testLineLimit,
          },
          ...result,
        },
        null,
        2,
      )}\n`,
    );
  } else {
    printHuman(args, result);
  }

  if ((result.violations.length > 0 || result.staleAllowlistEntries.length > 0) && !args.warnOnly) {
    process.exitCode = 1;
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`size-guard: ${message}\n`);
  process.exitCode = 1;
}
