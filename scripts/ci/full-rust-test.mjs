#!/usr/bin/env node
import os from "node:os";
import {
  defaultJobCount,
  formatStepTimingSummary,
  parsePositiveInteger,
  runStepsParallel,
  runStepsSerial,
} from "./main-internal-test-runner.mjs";

const WORKSPACE_SERIAL_SKIP_ARGS = Object.freeze([
  "--skip",
  "login::",
  "--skip",
  "quota_doctor::",
  "--skip",
  "run::",
  "--skip",
  "shared_state::",
  "--skip",
  "super_mode::",
  "--skip",
  "ping::ping_openai_sends_extra_spark_ping_when_profile_has_spark_limit",
  "--skip",
  "ping::ping_openai_sends_ping_to_each_ready_openai_profile",
  "--skip",
  "continuity_failure_reason_metrics_",
  "--skip",
  "rtk::tests::",
]);

function defaultTestThreads() {
  const available = typeof os.availableParallelism === "function" ? os.availableParallelism() : os.cpus().length;
  return Math.max(1, Math.min(4, available || 1));
}

function parseArgs(argv) {
  const args = {
    dryRun: false,
    jobs: defaultJobCount(),
    prebuild: true,
    prodexAppLib: true,
    testThreads: defaultTestThreads(),
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
    if (value === "--test-threads") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--test-threads requires a value");
      }
      args.testThreads = parsePositiveInteger(argv[index], "--test-threads");
      continue;
    }
    if (value === "--no-prebuild") {
      args.prebuild = false;
      continue;
    }
    if (value === "--no-prodex-app-lib") {
      args.prodexAppLib = false;
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
      "Usage: node scripts/ci/full-rust-test.mjs [--jobs <n>] [--test-threads <n>] [--no-prebuild] [--no-prodex-app-lib] [--timings] [--timings-json] [--timings-limit <n>] [--dry-run]",
      "",
      "Runs the full workspace Rust suite in faster partitions.",
      "",
      "Partitions:",
      "  - prebuild all all-features workspace and prodex-app lib test binaries once",
      "  - run workspace-safe all-features tests with parallel libtest threads",
      "  - run global-cache broker-log metrics tests serial",
      "  - run temp-executable rtk wrapper tests serial",
      "  - run auto_rotate integration tests as parallel serial shards",
      "  - run every prodex-app lib test serially (the crate disables implicit workspace tests)",
    ].join("\n") + "\n",
  );
}

function timingSummary(args, label) {
  if (!args.timings) {
    return null;
  }
  return {
    json: args.timingsJson,
    label,
    limit: args.timingsLimit,
  };
}

function prebuildSteps(args) {
  return [
    {
      label: "prebuild:workspace-all-features",
      command: "cargo",
      args: [
        "test",
        "--locked",
        "--workspace",
        ...(!args.prodexAppLib ? ["--exclude", "prodex-app"] : []),
        "--all-features",
        "--no-run",
      ],
    },
    ...(args.prodexAppLib
      ? [
          {
            label: "prebuild:prodex-app-lib-all-features",
            command: "cargo",
            args: ["test", "--locked", "-p", "prodex-app", "--lib", "--all-features", "--no-run"],
          },
        ]
      : []),
  ];
}

function workspaceSteps(args) {
  return [
    {
      label: "workspace:parallel-safe",
      command: "cargo",
      args: [
        "test",
        "--locked",
        "-q",
        "--workspace",
        ...(!args.prodexAppLib ? ["--exclude", "prodex-app"] : []),
        "--all-features",
        "--",
        `--test-threads=${args.testThreads}`,
        ...WORKSPACE_SERIAL_SKIP_ARGS,
      ],
    },
    {
      label: "workspace:broker-log-cache-serial",
      command: "cargo",
      args: [
        "test",
        "--locked",
        "-q",
        "-p",
        "prodex-runtime-broker-log",
        "--lib",
        "--all-features",
        "continuity_failure_reason_metrics_",
        "--",
        "--test-threads=1",
      ],
      failOnZeroTests: true,
    },
    {
      label: "workspace:caveman-rtk-serial",
      command: "cargo",
      args: [
        "test",
        "--locked",
        "-q",
        "-p",
        "prodex-optional-tools",
        "--lib",
        "--all-features",
        "rtk::tests::",
        "--",
        "--test-threads=1",
      ],
      failOnZeroTests: true,
    },
  ];
}

function autoRotateStep(args) {
  return {
    label: "auto-rotate-shards",
    command: "node",
    args: [
      "scripts/ci/auto-rotate-shards.mjs",
      "--all-features",
      "--jobs",
      String(args.jobs),
      ...(args.timings ? ["--timings", "--timings-limit", String(args.timingsLimit)] : []),
      ...(args.timingsJson ? ["--timings-json"] : []),
    ],
  };
}

function prodexAppStep() {
  return {
    label: "prodex-app:all-lib-tests-serial",
    command: "cargo",
    args: [
      "test",
      "--locked",
      "-q",
      "-p",
      "prodex-app",
      "--lib",
      "--all-features",
      "--",
      "--test-threads=1",
    ],
    failOnZeroTests: true,
  };
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const completed = [];
  if (args.prebuild) {
    completed.push(
      ...(await runStepsSerial(prebuildSteps(args), {
        dryRun: args.dryRun,
        timingSummary: timingSummary(args, "full-rust-test:prebuild"),
      })),
    );
  }

  completed.push(
    ...(await runStepsParallel([...workspaceSteps(args), autoRotateStep(args)], {
      dryRun: args.dryRun,
      jobs: 3,
      timingSummary: timingSummary(args, "full-rust-test:test-partitions"),
    })),
  );
  if (args.prodexAppLib) {
    completed.push(
      ...(await runStepsSerial([prodexAppStep()], {
        dryRun: args.dryRun,
        timingSummary: timingSummary(args, "full-rust-test:prodex-app"),
      })),
    );
  }

  if (args.timings && !args.dryRun) {
    process.stdout.write(
      formatStepTimingSummary(completed, {
        label: "full-rust-test",
        limit: args.timingsLimit,
        json: args.timingsJson,
      }),
    );
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`full-rust-test: ${message}\n`);
  process.exitCode = 1;
}
