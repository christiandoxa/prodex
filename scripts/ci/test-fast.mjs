#!/usr/bin/env node
import {
  cargoIntegrationTestStep,
  cargoTestStep,
  defaultJobCount,
  parsePositiveInteger,
  runStepsParallel,
  runStepsSerial,
} from "./main-internal-test-runner.mjs";

const CHECK_STEPS = [
  {
    label: "fmt",
    command: "cargo",
    args: ["fmt", "--check"],
  },
  {
    label: "docs-lint",
    command: "node",
    args: ["scripts/docs/lint-markdown.mjs"],
  },
];

const CARGO_CHECK_STEP = {
  label: "cargo-check",
  command: "cargo",
  args: ["check", "--locked", "--workspace", "--all-targets", "--all-features"],
};

const PREBUILD_STEPS = [
  {
    label: "prebuild:lib-tests",
    command: "cargo",
    args: ["test", "-p", "prodex-app", "--lib", "--no-run"],
  },
  {
    label: "prebuild:test:auto-rotate",
    command: "cargo",
    args: ["test", "--test", "auto_rotate", "--no-run"],
  },
];

const SAFE_CARGO_TEST_STEPS = [
  {
    label: "crate:codex-config",
    command: "cargo",
    args: ["test", "-p", "prodex-codex-config", "--lib", "--", "--test-threads=1"],
    failOnZeroTests: true,
  },
  {
    label: "crate:redaction",
    command: "cargo",
    args: ["test", "-p", "prodex-redaction", "--lib", "--", "--test-threads=1"],
    failOnZeroTests: true,
  },
  {
    label: "crate:runtime-metrics",
    command: "cargo",
    args: ["test", "-p", "prodex-runtime-metrics", "--lib", "--", "--test-threads=1"],
    failOnZeroTests: true,
  },
  {
    label: "crate:secret-store",
    command: "cargo",
    args: ["test", "-p", "prodex-secret-store", "--lib", "--", "--test-threads=1"],
    failOnZeroTests: true,
  },
  {
    label: "crate:terminal-ui",
    command: "cargo",
    args: ["test", "-p", "prodex-terminal-ui", "--lib", "--", "--test-threads=1"],
    failOnZeroTests: true,
  },
  {
    label: "crate:runtime-policy",
    command: "cargo",
    args: ["test", "-p", "prodex-runtime-policy", "--lib", "--", "--test-threads=1"],
    failOnZeroTests: true,
  },
  {
    label: "crate:audit-log",
    command: "cargo",
    args: ["test", "-p", "prodex-audit-log", "--lib", "--", "--test-threads=1"],
    failOnZeroTests: true,
  },
  cargoTestStep("lib:app-commands", "app_commands::"),
  cargoTestStep("lib:compat-replay", "compat_replay_tests::"),
  cargoTestStep("lib:profile-identity", "profile_identity::"),
  cargoTestStep("lib:quota-support", "quota_support::"),
  cargoTestStep("lib:runtime-background", "runtime_background::"),
  cargoTestStep("lib:runtime-claude", "runtime_claude::"),
  cargoTestStep("lib:runtime-config", "runtime_config::"),
  cargoTestStep("lib:runtime-doctor", "runtime_doctor::"),
  cargoTestStep("lib:runtime-launch", "runtime_launch::"),
  cargoTestStep("lib:test-env-guard", "test_env_guard_tests::"),
  cargoIntegrationTestStep("test:auto-rotate", "auto_rotate"),
];

function isCiEnv() {
  const value = process.env.CI;
  return Boolean(value && value !== "0" && value.toLowerCase() !== "false");
}

function parseArgs(argv) {
  const args = {
    jobs: defaultJobCount(),
    checks: true,
    cargoCheck: true,
    prebuild: !isCiEnv(),
    tests: true,
    dryRun: false,
    timings: false,
    timingsJson: false,
    timingsLimit: 10,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--jobs" || value === "-j") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.jobs = parsePositiveInteger(argv[index], value);
      continue;
    }
    if (value === "--checks-only") {
      args.tests = false;
      continue;
    }
    if (value === "--tests-only") {
      args.checks = false;
      args.cargoCheck = false;
      continue;
    }
    if (value === "--no-cargo-check") {
      args.cargoCheck = false;
      continue;
    }
    if (value === "--prebuild") {
      args.prebuild = true;
      continue;
    }
    if (value === "--no-prebuild") {
      args.prebuild = false;
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--timings") {
      args.timings = true;
      continue;
    }
    if (value === "--timings-json") {
      args.timings = true;
      args.timingsJson = true;
      continue;
    }
    if (value === "--timings-limit") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--timings-limit requires a value");
      }
      args.timingsLimit = parsePositiveInteger(argv[index], "--timings-limit");
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
      "Usage: node scripts/ci/test-fast.mjs [--jobs <n>] [--checks-only|--tests-only] [--no-cargo-check] [--prebuild|--no-prebuild] [--timings] [--timings-json] [--timings-limit <n>] [--dry-run]",
      "",
      "Runs local fast checks plus independent safe cargo test shards.",
      "Prebuild is enabled by default outside CI to warm cargo test binaries once before parallel shards.",
      "",
      "Includes:",
      "  - cargo fmt --check",
      "  - docs markdown lint",
      "  - cargo check --locked --workspace --all-targets --all-features",
      "  - optional cargo test --no-run prebuild for cargo test shards",
      "  - safe lib/integration cargo test shards as separate child processes",
      "",
      "Options:",
      "  --prebuild     force prebuild even when CI=true",
      "  --no-prebuild  skip prebuild when measuring cold parallel behavior",
      "  --timings      print a slowest-step duration summary after each runner phase",
      "  --timings-json include a single-line JSON timing payload in each summary",
      "",
      "Quarantined runtime, profile, env-sensitive, continuation-heavy, and global-state shards stay in test:serial.",
    ].join("\n") + "\n",
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const timingSummary = args.timings
    ? { label: "test-fast", limit: args.timingsLimit, json: args.timingsJson }
    : null;

  if (args.checks) {
    await runStepsParallel(CHECK_STEPS, {
      jobs: Math.min(args.jobs, CHECK_STEPS.length),
      dryRun: args.dryRun,
      timingSummary,
    });
  }
  if (args.cargoCheck) {
    await runStepsSerial([CARGO_CHECK_STEP], { dryRun: args.dryRun, timingSummary });
  }
  if (args.tests && args.prebuild) {
    process.stdout.write("test-fast: prebuilding cargo test binaries before parallel shards\n");
    await runStepsSerial(PREBUILD_STEPS, { dryRun: args.dryRun, timingSummary });
  }
  if (args.tests) {
    await runStepsParallel(SAFE_CARGO_TEST_STEPS, { jobs: args.jobs, dryRun: args.dryRun, timingSummary });
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`test-fast: ${message}\n`);
  process.exitCode = 1;
}
