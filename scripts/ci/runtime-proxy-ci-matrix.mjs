#!/usr/bin/env node
import { RUNTIME_CI_WORKFLOW_SHARDS } from "./runtime-test-manifest.mjs";

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

function matrixEntry(shard, index) {
  const suite = requireNonEmptyString(shard?.suite, `workflow shard ${index} suite`);
  const label = requireNonEmptyString(shard?.label, `workflow shard ${index} label`);
  if (!Array.isArray(shard.filters) || shard.filters.length === 0) {
    throw new Error(`workflow shard ${suite} must have one or more filters`);
  }

  const filters = shard.filters.map((filter, filterIndex) => {
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

  return {
    suite,
    label,
    filters: filters.join("\n"),
  };
}

function githubMatrix() {
  return {
    include: RUNTIME_CI_WORKFLOW_SHARDS.map(matrixEntry),
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
