#!/usr/bin/env node
import {
  defaultJobCount,
  parsePositiveInteger,
  runStepsSerial,
} from "./main-internal-test-runner.mjs";

function parseArgs(argv) {
  const args = {
    churnCheck: process.env.PRODEX_PREFLIGHT_CHURN_REPORT_ONLY !== "1",
    churnIgnoreBefore: process.env.PRODEX_PREFLIGHT_CHURN_IGNORE_BEFORE || null,
    churnRange: process.env.PRODEX_PREFLIGHT_CHURN_RANGE || null,
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
    if (value === "--churn-check") {
      args.churnCheck = true;
      continue;
    }
    if (value === "--no-churn-check") {
      args.churnCheck = false;
      continue;
    }
    if (value === "--churn-report-only") {
      args.churnCheck = false;
      continue;
    }
    if (value === "--churn-ignore-before") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.churnIgnoreBefore = argv[index];
      continue;
    }
    if (value === "--churn-range") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.churnRange = argv[index];
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
      "Usage: node scripts/ci/preflight.mjs [--jobs <n>] [--no-tests] [--serial] [--churn-check|--churn-report-only] [--churn-range <range>] [--churn-ignore-before <rev>] [--dry-run]",
      "",
      "Runs the practical local preflight gate before pushing.",
      "",
      "Default checks:",
      "  - release hygiene, Rust size/allow guards, crate boundaries, runtime hot-path, churn hygiene, manifest-owned version sync",
      "  - docs lint, upstream baseline, runtime manifest, fmt, cargo check",
      "  - cargo clippy --locked --all-targets --all-features -- -D warnings",
      "  - npm run test:fast -- --tests-only --no-prebuild",
      "",
      "Churn hygiene fails locally and in CI by default. Use --churn-report-only or PRODEX_PREFLIGHT_CHURN_REPORT_ONLY=1 for exploratory dry runs.",
      "",
      "Options:",
      "  --jobs <n>        set test:fast child process parallelism",
      "  --serial          also run npm run test:serial -- --suite all",
      "  --no-tests        skip test:fast; useful when only checking metadata/clippy",
      "  --churn-check        fail when churn hygiene thresholds are exceeded; default unless report-only env is set",
      "  --churn-report-only  force churn hygiene report-only mode",
      "  --no-churn-check     deprecated alias for --churn-report-only",
      "  --churn-range <range>  pass an explicit git range to churn hygiene",
      "  --churn-ignore-before <rev>  pass a reviewed historical baseline to churn hygiene; latest-tag is supported",
      "  --dry-run         print the command plan without running it",
    ].join("\n") + "\n",
  );
}

function preflightSteps(args) {
  const churnArgs = ["scripts/ci/churn-hygiene.mjs", ...(args.churnCheck ? ["--check"] : ["--report-only"])];
  if (args.churnRange) {
    churnArgs.push("--range", args.churnRange);
  }
  if (args.churnIgnoreBefore) {
    churnArgs.push("--ignore-before", args.churnIgnoreBefore);
  }
  const steps = [
    {
      label: "release-hygiene",
      command: "node",
      args: ["scripts/ci/release-hygiene.mjs"],
    },
    {
      label: "size-guard",
      command: "node",
      args: ["scripts/ci/size-guard.mjs"],
    },
    {
      label: "allow-attribute-guard",
      command: "node",
      args: ["scripts/ci/allow-attribute-guard.mjs"],
    },
    {
      label: "env-mutation-guard",
      command: "node",
      args: ["scripts/ci/env-mutation-guard.mjs"],
    },
    {
      label: "crate-boundary-guard",
      command: "node",
      args: ["scripts/ci/crate-boundary-guard.mjs"],
    },
    {
      label: "runtime-hotpath-guard",
      command: "node",
      args: ["scripts/ci/runtime-hotpath-guard.mjs"],
    },
    {
      label: args.churnCheck ? "churn-hygiene-check" : "churn-hygiene-report",
      command: "node",
      args: churnArgs,
    },
    {
      label: "metadata-preflight",
      command: "node",
      args: ["scripts/npm/release-prepare.mjs", "--no-cargo-test", "--ci-changelog-check"],
    },
    {
      label: "generated-metadata-clean",
      command: "node",
      args: ["scripts/ci/generated-metadata-clean.mjs"],
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
