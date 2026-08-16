#!/usr/bin/env node
import { RUNTIME_CI_WORKFLOW_SHARDS } from "./runtime-test-manifest.mjs";

const RUNTIME_STRESS_BROAD_SHARDS = Object.freeze(
  Array.from({ length: 5 }, (_, shard) => ({
    suite: "stress",
    id: `stress-${shard + 1}`,
    label: `weighted broad shard ${shard + 1} of 5`,
    shard,
    shard_count: 5,
  })),
);
const RUNTIME_STRESS_QUARANTINE_SHARDS = Object.freeze([
  { suite: "serialized", id: "serialized-1", label: "serialized shard 1 of 2", shard: 0, shard_count: 2 },
  { suite: "serialized", id: "serialized-2", label: "serialized shard 2 of 2", shard: 1, shard_count: 2 },
  { suite: "continuation", id: "continuation-1", label: "continuation shard 1 of 2", shard: 0, shard_count: 2 },
  { suite: "continuation", id: "continuation-2", label: "continuation shard 2 of 2", shard: 1, shard_count: 2 },
]);
function parseArgs(argv) {
  const args = {};
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--github-matrix") {
      args.githubMatrix = true;
      continue;
    }
    if (value === "--github-stress-matrix") {
      args.githubStressMatrix = true;
      continue;
    }
    if (value === "--event-name") {
      index += 1;
      if (!argv[index]) throw new Error("--event-name requires a value");
      args.eventName = argv[index];
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
      "       node scripts/ci/runtime-proxy-ci-matrix.mjs --github-stress-matrix --event-name <name>",
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
    save_cache: suite === "root",
    filters: shardFilters(shard, index).join("\n"),
  };
}

function githubMatrix() {
  return {
    include: RUNTIME_CI_WORKFLOW_SHARDS.map(matrixEntry),
  };
}

function githubStressMatrix(eventName) {
  if (!new Set(["push", "pull_request", "schedule", "workflow_dispatch"]).has(eventName)) {
    throw new Error(`unsupported CI event name: ${eventName ?? "missing"}`);
  }
  const broad = eventName === "schedule" || eventName === "workflow_dispatch";
  const shards = [
    ...(broad ? RUNTIME_STRESS_BROAD_SHARDS : []),
    ...RUNTIME_STRESS_QUARANTINE_SHARDS,
  ];
  return {
    include: shards.map((shard, index) => ({ ...shard, save_cache: index === 0 })),
  };
}

const args = parseArgs(process.argv);
if (args.help) {
  printHelp();
} else if (args.githubMatrix) {
  process.stdout.write(`${JSON.stringify(githubMatrix())}\n`);
} else if (args.githubStressMatrix) {
  process.stdout.write(`${JSON.stringify(githubStressMatrix(args.eventName))}\n`);
} else {
  throw new Error("missing required matrix mode");
}
