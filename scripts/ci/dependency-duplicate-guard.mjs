#!/usr/bin/env node
import fs from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { run } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

export const DEFAULT_DUPLICATE_BUDGET = Object.freeze([
  {
    name: "block-buffer",
    maxVersions: 2,
    reason: "digest 0.10 and 0.11 are both present through sha1/sha2-era crypto crates.",
  },
  {
    name: "cpufeatures",
    maxVersions: 2,
    reason: "sha1 and sha2 currently resolve through different cpufeatures lines.",
  },
  {
    name: "crypto-common",
    maxVersions: 2,
    reason: "digest 0.10 and 0.11 pull different crypto-common lines.",
  },
  {
    name: "digest",
    maxVersions: 2,
    reason: "tungstenite/sha1 and export crypto use different digest major lines.",
  },
  {
    name: "getrandom",
    maxVersions: 3,
    reason: "rand/proptest/tempfile/export crypto currently span getrandom 0.2, 0.3, and 0.4.",
  },
  {
    name: "rand_core",
    maxVersions: 2,
    reason: "crypto-common and rand/proptest currently span rand_core 0.6 and 0.9.",
  },
]);

function requiredValue(value, name) {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

export function parseArgs(argv) {
  const args = {
    input: null,
    json: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--input") {
      index += 1;
      args.input = requiredValue(argv[index], value);
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

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/dependency-duplicate-guard.mjs [--input <cargo-tree-output>] [--json]",
      "",
      "Runs cargo tree -d --workspace and enforces the checked-in duplicate dependency budget.",
      "",
      "Duplicate families must be explicitly budgeted. Budget entries also fail when stale,",
      "so resolved duplicate families must ratchet this guard downward.",
      "",
      "Options:",
      "  --input <path>  Read saved cargo tree -d output instead of invoking cargo.",
      "  --json          Print machine-readable result.",
    ].join("\n") + "\n",
  );
}

function versionCompare(left, right) {
  return left.localeCompare(right, "en", { numeric: true, sensitivity: "base" });
}

export function parseCargoTreeDuplicates(output) {
  const versionsByName = new Map();
  const rootLinePattern = /^([A-Za-z0-9_.-]+)\s+v([^\s]+)(?:\s|$)/;

  for (const rawLine of output.split(/\r?\n/)) {
    const line = rawLine.trimEnd();
    if (!line) {
      continue;
    }

    const match = line.match(rootLinePattern);
    if (!match) {
      continue;
    }

    const [, name, version] = match;
    const versions = versionsByName.get(name) ?? new Set();
    versions.add(version);
    versionsByName.set(name, versions);
  }

  return [...versionsByName.entries()]
    .map(([name, versions]) => ({
      name,
      versions: [...versions].sort(versionCompare),
    }))
    .filter((entry) => entry.versions.length > 1)
    .sort((left, right) => left.name.localeCompare(right.name));
}

function validateBudgetEntries(entries) {
  const seen = new Set();
  for (const entry of entries) {
    if (!entry.name || typeof entry.name !== "string") {
      throw new Error("duplicate dependency budget entry missing name");
    }
    if (!Number.isSafeInteger(entry.maxVersions) || entry.maxVersions < 2) {
      throw new Error(`duplicate dependency budget for ${entry.name} must be at least 2`);
    }
    if (seen.has(entry.name)) {
      throw new Error(`duplicate dependency budget entry: ${entry.name}`);
    }
    seen.add(entry.name);
  }
}

export function evaluateDuplicateBudget(duplicateFamilies, budgetEntries = DEFAULT_DUPLICATE_BUDGET) {
  validateBudgetEntries(budgetEntries);

  const actualByName = new Map(duplicateFamilies.map((entry) => [entry.name, entry]));
  const budgetByName = new Map(budgetEntries.map((entry) => [entry.name, entry]));

  const unallowlistedFamilies = duplicateFamilies.filter((entry) => !budgetByName.has(entry.name));
  const overBudgetFamilies = duplicateFamilies
    .filter((entry) => {
      const budget = budgetByName.get(entry.name);
      return budget && entry.versions.length > budget.maxVersions;
    })
    .map((entry) => ({
      ...entry,
      maxVersions: budgetByName.get(entry.name).maxVersions,
      reason: budgetByName.get(entry.name).reason,
    }));
  const staleBudgetEntries = budgetEntries
    .filter((entry) => {
      const actual = actualByName.get(entry.name);
      return !actual || actual.versions.length < entry.maxVersions;
    })
    .map((entry) => ({
      ...entry,
      versions: actualByName.get(entry.name)?.versions ?? [],
    }));

  const status =
    unallowlistedFamilies.length === 0 &&
    overBudgetFamilies.length === 0 &&
    staleBudgetEntries.length === 0
      ? "ok"
      : "failed";

  return {
    status,
    duplicateFamilies,
    duplicateFamilyBudget: budgetEntries.length,
    duplicateFamilyCount: duplicateFamilies.length,
    unallowlistedFamilies,
    overBudgetFamilies,
    staleBudgetEntries,
  };
}

export function budgetFailed(summary) {
  return summary.status !== "ok";
}

function formatVersions(versions) {
  return versions.length > 0 ? versions.join(", ") : "none";
}

function formatFamily(entry, budget = null) {
  const budgetText = budget ? `; budget ${budget.maxVersions}` : "";
  return `- ${entry.name}: ${entry.versions.length} version(s)${budgetText}: ${formatVersions(entry.versions)}`;
}

function printSummary(summary, budgetEntries = DEFAULT_DUPLICATE_BUDGET) {
  const budgetByName = new Map(budgetEntries.map((entry) => [entry.name, entry]));
  process.stdout.write(`dependency duplicate budget: ${summary.status}\n`);
  process.stdout.write(
    `duplicate families: ${summary.duplicateFamilyCount}/${summary.duplicateFamilyBudget} budgeted\n`,
  );

  if (summary.status === "ok") {
    for (const entry of summary.duplicateFamilies) {
      const budget = budgetByName.get(entry.name);
      process.stdout.write(`${formatFamily(entry, budget)}\n`);
    }
    return;
  }

  if (summary.unallowlistedFamilies.length > 0) {
    process.stdout.write("unallowlisted duplicate families:\n");
    for (const entry of summary.unallowlistedFamilies) {
      process.stdout.write(`${formatFamily(entry)}\n`);
    }
  }

  if (summary.overBudgetFamilies.length > 0) {
    process.stdout.write("over-budget duplicate families:\n");
    for (const entry of summary.overBudgetFamilies) {
      process.stdout.write(`${formatFamily(entry, entry)}\n`);
    }
  }

  if (summary.staleBudgetEntries.length > 0) {
    process.stdout.write("stale duplicate budget entries:\n");
    for (const entry of summary.staleBudgetEntries) {
      process.stdout.write(
        `- ${entry.name}: expected ${entry.maxVersions} duplicate version(s), found ${entry.versions.length}: ${formatVersions(entry.versions)}\n`,
      );
    }
  }
}

async function cargoTreeDuplicatesOutput() {
  const result = await run("cargo", ["tree", "-d", "--workspace"], { cwd: repoRoot });
  return result.stdout;
}

async function readDuplicateOutput(args) {
  if (args.input) {
    return await fs.readFile(args.input, "utf8");
  }
  return await cargoTreeDuplicatesOutput();
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const output = await readDuplicateOutput(args);
  const duplicateFamilies = parseCargoTreeDuplicates(output);
  const summary = evaluateDuplicateBudget(duplicateFamilies);

  if (args.json) {
    process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
  } else {
    printSummary(summary);
  }

  if (budgetFailed(summary)) {
    process.exitCode = 1;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    process.stderr.write(`dependency-duplicate-guard: ${error.message}\n`);
    process.exitCode = 1;
  });
}
