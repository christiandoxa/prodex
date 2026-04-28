#!/usr/bin/env node
import {
  defaultJobCount,
  parsePositiveInteger,
  runStepsSerial,
} from "./main-internal-test-runner.mjs";

function parseArgs(argv) {
  const args = {
    dryRun: false,
    jobs: defaultJobCount(),
    fastTests: true,
    serial: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--jobs" || value === "-j") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.jobs = parsePositiveInteger(argv[index], value);
      continue;
    }
    if (value === "--no-tests") {
      args.fastTests = false;
      continue;
    }
    if (value === "--serial") {
      args.serial = true;
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
      "Usage: node scripts/ci/preflight.mjs [--jobs <n>] [--no-tests] [--serial] [--dry-run]",
      "",
      "Runs the practical local preflight gate before pushing or release prep.",
      "",
      "Default checks:",
      "  - release metadata-only, runtime hot-path, churn hygiene, version sync",
      "  - docs lint, upstream baseline, runtime manifest, fmt, cargo check",
      "  - cargo clippy --locked --all-targets --all-features -- -D warnings",
      "  - npm run test:fast -- --tests-only --no-prebuild",
      "",
      "Options:",
      "  --jobs <n>  set test:fast child process parallelism",
      "  --serial    also run npm run test:serial -- --suite all",
      "  --no-tests  skip test:fast; useful when only checking metadata/clippy",
      "  --dry-run   print the command plan without running it",
    ].join("\n") + "\n",
  );
}

function preflightSteps(args) {
  const steps = [
    {
      label: "release-metadata-only-guard",
      command: "node",
      args: ["scripts/ci/release-metadata-only-guard.mjs"],
    },
    {
      label: "runtime-hotpath-guard",
      command: "node",
      args: ["scripts/ci/runtime-hotpath-guard.mjs"],
    },
    {
      label: "churn-hygiene-report",
      command: "node",
      args: ["scripts/ci/churn-hygiene.mjs"],
    },
    {
      label: "release-preflight",
      command: "node",
      args: ["scripts/npm/release-prepare.mjs", "--no-cargo-test"],
    },
    {
      label: "clippy",
      command: "cargo",
      args: ["clippy", "--locked", "--all-targets", "--all-features", "--", "-D", "warnings"],
    },
  ];

  if (args.fastTests) {
    steps.push({
      label: "test-fast",
      command: "node",
      args: ["scripts/ci/test-fast.mjs", "--tests-only", "--no-prebuild", "--jobs", String(args.jobs)],
    });
  }

  if (args.serial) {
    steps.push({
      label: "test-serial",
      command: "node",
      args: ["scripts/ci/test-serial.mjs", "--suite", "all"],
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

  await runStepsSerial(preflightSteps(args), { dryRun: args.dryRun });
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`preflight: ${message}\n`);
  process.exitCode = 1;
}
