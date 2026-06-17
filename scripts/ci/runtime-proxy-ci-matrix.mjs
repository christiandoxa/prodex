#!/usr/bin/env node
import { RUNTIME_CI_WORKFLOW_SHARDS } from "./runtime-test-manifest.mjs";

const TARGET_MATRIX_JOBS = 16;
const DEFAULT_WEIGHT_SECONDS = 90;
const WORKFLOW_SHARD_WEIGHT_SECONDS = Object.freeze({
  "admission": 111,
  "anthropic-launch": 86,
  "anthropic-request": 86,
  "anthropic-response": 56,
  "anthropic-runtime": 105,
  "continuation-http-followups-affinity": 90,
  "continuation-http-followups-metadata": 113,
  "continuation-http-followups-rotation": 112,
  "continuation-http-tool-compact": 105,
  "continuation-post-commit": 108,
  "continuation-websocket-precommit": 128,
  "doctor-state-persistence": 91,
  "doctor-state-registry": 113,
  "doctor-state-runtime": 116,
  "doctor-summary-guidance": 92,
  "health": 88,
  "incidents": 77,
  "persistence": 66,
  "root": 121,
  "rotation": 90,
  "selection": 92,
  "state": 56,
});

function parseArgs(argv) {
  const args = {};
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--github-matrix") {
      args.githubMatrix = true;
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
      "Usage: node scripts/ci/runtime-proxy-ci-matrix.mjs --github-matrix",
      "",
      "Prints the GitHub Actions matrix for the main-internal-runtime-proxy job.",
    ].join("\n") + "\n",
  );
}

function requireNonEmptyString(value, name) {
  if (typeof value !== "string" || value.trim() !== value || value.length === 0) {
    throw new Error(`${name} must be a non-empty trimmed string`);
  }
  return value;
}

function shardFilters(shard, index) {
  const suite = requireNonEmptyString(shard?.suite, `workflow shard ${index} suite`);
  if (!Array.isArray(shard.filters) || shard.filters.length === 0) {
    throw new Error(`workflow shard ${suite} must have one or more filters`);
  }

  return shard.filters.map((filter, filterIndex) => {
    const filterLabel = requireNonEmptyString(
      filter?.label,
      `workflow shard ${suite} filters[${filterIndex}] label`,
    );
    const filterValue = requireNonEmptyString(
      filter?.filter,
      `workflow shard ${suite} filters[${filterIndex}] filter`,
    );
    return `${filterLabel}|${filterValue}`;
  });
}

function matrixEntry(shard, index) {
  const suite = requireNonEmptyString(shard?.suite, `workflow shard ${index} suite`);
  const label = requireNonEmptyString(shard?.label, `workflow shard ${index} label`);

  return {
    suite,
    label,
    validate_manifest: suite === "root" ? "true" : "false",
    filters: shardFilters(shard, index).join("\n"),
  };
}

function shardWeightSeconds(shard) {
  return WORKFLOW_SHARD_WEIGHT_SECONDS[shard.suite] ?? DEFAULT_WEIGHT_SECONDS;
}

function packedMatrixEntries(shards, targetJobs = TARGET_MATRIX_JOBS) {
  const standaloneRoot = shards.find((shard) => shard.suite === "root");
  const packableShards = shards.filter((shard) => shard.suite !== "root");
  const packCount = Math.max(1, Math.min(targetJobs - (standaloneRoot ? 1 : 0), packableShards.length));
  const packs = Array.from({ length: packCount }, () => ({
    filters: [],
    labels: [],
    suites: [],
    weightSeconds: 0,
  }));

  for (const shard of [...packableShards].sort((left, right) => shardWeightSeconds(right) - shardWeightSeconds(left))) {
    packs.sort((left, right) => left.weightSeconds - right.weightSeconds);
    const pack = packs[0];
    pack.filters.push(...shardFilters(shard, shards.indexOf(shard)));
    pack.labels.push(shard.label);
    pack.suites.push(shard.suite);
    pack.weightSeconds += shardWeightSeconds(shard);
  }

  const entries = [];
  if (standaloneRoot) {
    entries.push(matrixEntry(standaloneRoot, shards.indexOf(standaloneRoot)));
  }

  packs
    .filter((pack) => pack.filters.length > 0)
    .sort((left, right) => right.weightSeconds - left.weightSeconds || left.labels[0].localeCompare(right.labels[0]))
    .forEach((pack, index) => {
      const oneBasedIndex = index + 1;
      entries.push({
        suite: `pack-${String(oneBasedIndex).padStart(2, "0")}`,
        label: pack.labels.length === 1 ? pack.labels[0] : `packed shard ${oneBasedIndex}`,
        validate_manifest: "false",
        filters: pack.filters.join("\n"),
      });
    });

  return entries;
}

function githubMatrix() {
  return {
    include: packedMatrixEntries(RUNTIME_CI_WORKFLOW_SHARDS),
  };
}

const args = parseArgs(process.argv);
if (args.help) {
  printHelp();
} else if (args.githubMatrix) {
  process.stdout.write(`${JSON.stringify(githubMatrix())}\n`);
} else {
  throw new Error("missing required --github-matrix");
}
