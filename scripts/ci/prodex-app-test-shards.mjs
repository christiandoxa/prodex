#!/usr/bin/env node
import { fileURLToPath } from "node:url";

const FILTER_PREFIX = "main_internal_tests::runtime_proxy_selection_and_pressure::";

export const PRODEX_APP_LIB_FILTERS = Object.freeze([
  `${FILTER_PREFIX}selection::`,
  `${FILTER_PREFIX}pressure::`,
  `${FILTER_PREFIX}incidents::`,
  `${FILTER_PREFIX}admission::`,
  `${FILTER_PREFIX}state::`,
  `${FILTER_PREFIX}rotation::`,
  `${FILTER_PREFIX}health::`,
]);

export const PRODEX_APP_LIB_SHARDS = Object.freeze([
  { suite: "selection", label: "prodex-app selection", filter: PRODEX_APP_LIB_FILTERS[0] },
  { suite: "pressure", label: "prodex-app pressure", filter: PRODEX_APP_LIB_FILTERS[1] },
  { suite: "incidents", label: "prodex-app incidents", filter: PRODEX_APP_LIB_FILTERS[2] },
  { suite: "admission", label: "prodex-app admission", filter: PRODEX_APP_LIB_FILTERS[3] },
  { suite: "state", label: "prodex-app state", filter: PRODEX_APP_LIB_FILTERS[4] },
  { suite: "rotation", label: "prodex-app rotation", filter: PRODEX_APP_LIB_FILTERS[5] },
  { suite: "health", label: "prodex-app health", filter: PRODEX_APP_LIB_FILTERS[6] },
  {
    suite: "remainder",
    label: "prodex-app remaining library tests",
    skipFilters: PRODEX_APP_LIB_FILTERS,
  },
]);

const WORKSPACE_SHARD = Object.freeze({
  suite: "workspace",
  label: "workspace and auto-rotate",
});

function collectDuplicates(values) {
  const indexesByValue = new Map();
  values.forEach((value, index) => {
    const indexes = indexesByValue.get(value) ?? [];
    indexes.push(index);
    indexesByValue.set(value, indexes);
  });
  return [...indexesByValue.entries()]
    .filter(([, indexes]) => indexes.length > 1)
    .map(([value, indexes]) => `${value} at indexes ${indexes.join(", ")}`);
}

export function validateShards(shards = PRODEX_APP_LIB_SHARDS) {
  const issues = [];
  if (!Array.isArray(shards) || shards.length === 0) {
    return ["prodex-app shard manifest must be a non-empty array"];
  }

  const suites = [];
  const filters = [];
  let remainder;
  for (const [index, shard] of shards.entries()) {
    if (!shard || typeof shard !== "object" || Array.isArray(shard)) {
      issues.push(`shard[${index}] must be an object`);
      continue;
    }
    if (typeof shard.suite !== "string" || shard.suite.trim() === "") {
      issues.push(`shard[${index}] suite must be a non-empty string`);
    } else {
      suites.push(shard.suite);
    }
    if (typeof shard.label !== "string" || shard.label.trim() === "") {
      issues.push(`shard[${index}] label must be a non-empty string`);
    }
    const hasFilter = Object.hasOwn(shard, "filter");
    const hasSkipFilters = Object.hasOwn(shard, "skipFilters");
    if (hasFilter === hasSkipFilters) {
      issues.push(`shard[${index}] must define exactly one of filter or skipFilters`);
      continue;
    }
    if (hasFilter) {
      if (typeof shard.filter !== "string" || shard.filter.trim() === "") {
        issues.push(`shard[${index}] filter must be a non-empty string`);
      } else {
        filters.push(shard.filter);
      }
    } else if (!Array.isArray(shard.skipFilters) || shard.skipFilters.length === 0) {
      issues.push(`shard[${index}] skipFilters must be a non-empty array`);
    } else {
      remainder = shard;
    }
  }

  for (const duplicate of collectDuplicates(suites)) {
    issues.push(`duplicate shard suite: ${duplicate}`);
  }
  for (const duplicate of collectDuplicates(filters)) {
    issues.push(`duplicate shard filter: ${duplicate}`);
  }
  for (const [index, filter] of filters.entries()) {
    for (const sibling of filters.slice(index + 1)) {
      if (filter.includes(sibling) || sibling.includes(filter)) {
        issues.push(`overlapping shard filters: ${filter} and ${sibling}`);
      }
    }
  }

  if (!remainder) {
    issues.push("prodex-app shard manifest must contain a remainder shard");
  } else if (JSON.stringify(remainder.skipFilters) !== JSON.stringify(PRODEX_APP_LIB_FILTERS)) {
    issues.push("remainder shard must skip every targeted prodex-app filter in manifest order");
  }
  if (JSON.stringify(filters) !== JSON.stringify(PRODEX_APP_LIB_FILTERS)) {
    issues.push("targeted prodex-app filters must cover the manifest filter list exactly once");
  }

  return issues;
}

function matrixEntry(shard, index) {
  return {
    suite: shard.suite,
    label: shard.label,
    save_cache: index === 0,
    filter: shard.filter ?? "",
    skip_filters: shard.skipFilters?.join("\n") ?? "",
  };
}

export function githubMatrix({ includeWorkspace = false } = {}) {
  const include = PRODEX_APP_LIB_SHARDS.map((shard, index) =>
    matrixEntry(shard, includeWorkspace ? index + 1 : index),
  );
  if (includeWorkspace) {
    include.unshift({
      suite: WORKSPACE_SHARD.suite,
      label: WORKSPACE_SHARD.label,
      save_cache: true,
      filter: "",
      skip_filters: "",
    });
  }
  return { include };
}

function stepCommand(shard) {
  if (shard.suite === "workspace") {
    return "node scripts/ci/full-rust-test.mjs --jobs 6 --test-threads 4 --timings --timings-json --no-prodex-app-lib";
  }
  if (shard.filter) {
    return `cargo test --locked -q -p prodex-app --lib --all-features '${shard.filter}' -- --test-threads=1`;
  }
  return [
    "cargo test --locked -q -p prodex-app --lib --all-features -- --test-threads=1",
    ...shard.skipFilters.map((filter) => `--skip '${filter}'`),
  ].join(" ");
}

function parseArgs(argv) {
  const args = {};
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--check") {
      args.check = true;
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--github-matrix") {
      args.githubMatrix = true;
      continue;
    }
    if (value === "--full-test-matrix") {
      args.fullTestMatrix = true;
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
      "Usage: node scripts/ci/prodex-app-test-shards.mjs --check|--dry-run|--github-matrix|--full-test-matrix",
      "",
      "Validates or prints the shared prodex-app library CI shard plan.",
      "--github-matrix emits the app-only GitHub Actions matrix.",
      "--full-test-matrix emits the scheduled full-test matrix including workspace coverage.",
    ].join("\n") + "\n",
  );
}

function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  const selectedModes = [args.check, args.dryRun, args.githubMatrix, args.fullTestMatrix].filter(Boolean).length;
  if (selectedModes !== 1) {
    throw new Error("choose exactly one of --check, --dry-run, --github-matrix, or --full-test-matrix");
  }

  const issues = validateShards();
  if (issues.length > 0) {
    throw new Error(issues.join("\n"));
  }
  if (args.check) {
    process.stdout.write(`prodex-app-test-shards: ${PRODEX_APP_LIB_SHARDS.length} app shard(s), one cache writer\n`);
    return;
  }
  if (args.githubMatrix || args.fullTestMatrix) {
    process.stdout.write(`${JSON.stringify(githubMatrix({ includeWorkspace: args.fullTestMatrix }))}\n`);
    return;
  }

  const shards = args.dryRun ? [WORKSPACE_SHARD, ...PRODEX_APP_LIB_SHARDS] : [];
  process.stdout.write(`dry-run: ${shards.length} full-test shard(s)\n`);
  for (const shard of shards) {
    process.stdout.write(`  ${shard.suite}: ${stepCommand(shard)}\n`);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`prodex-app-test-shards: ${message}\n`);
    process.exitCode = 1;
  }
}
