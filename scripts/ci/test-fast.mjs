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
  args: ["check", "--locked", "--all-targets", "--all-features"],
};

const SAFE_CARGO_TEST_STEPS = [
  cargoTestStep("lib:app-commands", "app_commands::"),
  cargoTestStep("lib:audit-log", "audit_log::"),
  cargoTestStep("lib:codex-config", "codex_config::"),
  cargoTestStep("lib:compat-replay", "compat_replay_tests::"),
  cargoTestStep("lib:profile-identity", "profile_identity::"),
  cargoTestStep("lib:quota-support", "quota_support::"),
  cargoTestStep("lib:runtime-background", "runtime_background::"),
  cargoTestStep("lib:runtime-claude", "runtime_claude::"),
  cargoTestStep("lib:runtime-config", "runtime_config::"),
  cargoTestStep("lib:runtime-doctor", "runtime_doctor::"),
  cargoTestStep("lib:runtime-launch", "runtime_launch::"),
  cargoTestStep("lib:runtime-metrics", "runtime_metrics::"),
  cargoTestStep("lib:runtime-policy", "runtime_policy::"),
  cargoTestStep("lib:secret-store", "secret_store::"),
  cargoTestStep("lib:test-env-guard", "test_env_guard_tests::"),
  cargoIntegrationTestStep("test:auto-rotate", "auto_rotate"),
];

function parseArgs(argv) {
  const args = {
    jobs: defaultJobCount(),
    checks: true,
    cargoCheck: true,
    tests: true,
    dryRun: false,
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

  return args;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/test-fast.mjs [--jobs <n>] [--checks-only|--tests-only] [--no-cargo-check] [--dry-run]",
      "",
      "Runs local fast checks plus independent safe cargo test shards.",
      "",
      "Includes:",
      "  - cargo fmt --check",
      "  - docs markdown lint",
      "  - cargo check --locked --all-targets --all-features",
      "  - safe lib/integration cargo test shards as separate child processes",
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

  if (args.checks) {
    await runStepsParallel(CHECK_STEPS, { jobs: Math.min(args.jobs, CHECK_STEPS.length), dryRun: args.dryRun });
  }
  if (args.cargoCheck) {
    await runStepsSerial([CARGO_CHECK_STEP], { dryRun: args.dryRun });
  }
  if (args.tests) {
    await runStepsParallel(SAFE_CARGO_TEST_STEPS, { jobs: args.jobs, dryRun: args.dryRun });
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`test-fast: ${message}\n`);
  process.exitCode = 1;
}
