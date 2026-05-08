#!/usr/bin/env node
import { pathToFileURL } from "node:url";
import { runStepsSerial } from "./main-internal-test-runner.mjs";
import { requiredValue } from "./release-guard-common.mjs";

const RANGE_GUARDS = Object.freeze([
  "release-metadata-only-guard.mjs",
  "version-metadata-release-guard.mjs",
  "release-empty-commit-guard.mjs",
  "release-duplicate-version-guard.mjs",
]);

function parseArgs(argv) {
  const args = {
    dryRun: false,
    fixtures: true,
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
    if (value === "--no-fixtures") {
      args.fixtures = false;
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  if (!args.help) {
    validateSelectorArgs(args);
  }
  return args;
}

function validateSelectorArgs(args) {
  const selectors = [
    Boolean(args.range),
    Boolean(args.base || args.head),
    Boolean(args.commit),
  ].filter(Boolean).length;

  if (selectors > 1) {
    throw new Error("choose only one selector: --range, --base/--head, or --commit");
  }
  if ((args.base || args.head) && !(args.base && args.head)) {
    throw new Error("--base and --head must be used together");
  }
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/release-hygiene.mjs [selector] [--no-fixtures] [--dry-run]",
      "",
      "Runs the complete release hygiene gate as one serial check.",
      "",
      "Selectors:",
      "  --range <rev-range>       inspect each commit in a git range",
      "  --base <rev> --head <rev> inspect each commit in base..head",
      "  --commit <rev>            inspect one commit",
      "",
      "Options:",
      "  --no-fixtures             skip historical release guard fixtures",
      "  --dry-run                 print the guard plan without running it",
      "  --help                    print this help",
      "",
      "Default selector: each guard's default, normally HEAD.",
    ].join("\n") + "\n",
  );
}

function rangeGuardSelectorArgs(args) {
  if (args.range) {
    return ["--range", args.range];
  }
  if (args.base && args.head) {
    return ["--base", args.base, "--head", args.head];
  }
  if (args.commit) {
    return ["--commit", args.commit];
  }
  return [];
}

function tagGuardSelectorArgs(args) {
  if (args.range) {
    return ["--range", args.range];
  }
  if (args.base && args.head) {
    return ["--base", args.base, "--head", args.head];
  }
  if (args.commit) {
    return ["--rev", args.commit];
  }
  return [];
}

export function releaseHygieneSteps(args = {}) {
  const selectorArgs = rangeGuardSelectorArgs(args);
  const steps = RANGE_GUARDS.map((script) => ({
    label: script.replace(/\.mjs$/, ""),
    command: "node",
    args: [`scripts/ci/${script}`, ...selectorArgs],
  }));

  steps.push({
    label: "release-tag-changelog-guard",
    command: "node",
    args: ["scripts/ci/release-tag-changelog-guard.mjs", ...tagGuardSelectorArgs(args)],
  });

  if (args.fixtures !== false) {
    steps.push({
      label: "release-guard-fixtures",
      command: "node",
      args: ["scripts/ci/release-guard-fixture-tests.mjs"],
    });
  }

  return steps;
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  await runStepsSerial(releaseHygieneSteps(args), { dryRun: args.dryRun });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    await main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`release-hygiene: ${message}\n`);
    process.exitCode = 1;
  }
}
