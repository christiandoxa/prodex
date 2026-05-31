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
  "continuity_failure_reason_metrics_",
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
      "Usage: node scripts/ci/full-rust-test.mjs [--jobs <n>] [--test-threads <n>] [--no-prebuild] [--timings] [--timings-json] [--timings-limit <n>] [--dry-run]",
      "",
      "Runs the full workspace Rust suite in faster partitions.",
      "",
      "Partitions:",
      "  - prebuild all all-features workspace test binaries once",
      "  - run workspace-safe all-features tests with parallel libtest threads",
      "  - run global-cache broker-log metrics tests serial",
      "  - run auto_rotate integration tests as parallel serial shards",
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

function prebuildSteps() {
  return [
    {
      label: "prebuild:workspace-all-features",
      command: "cargo",
      args: ["test", "--workspace", "--all-features", "--no-run"],
    },
  ];
}

function workspaceSteps(args) {
  return [
    {
      label: "workspace:parallel-safe",
      command: "cargo",
      args: [
        "test",
        "-q",
        "--workspace",
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

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const completed = [];
  if (args.prebuild) {
    completed.push(
      ...(await runStepsSerial(prebuildSteps(), {
        dryRun: args.dryRun,
        timingSummary: timingSummary(args, "full-rust-test:prebuild"),
      })),
    );
  }

  completed.push(
    ...(await runStepsParallel(workspaceSteps(args), {
      dryRun: args.dryRun,
      jobs: 2,
      timingSummary: timingSummary(args, "full-rust-test:workspace"),
    })),
  );

  completed.push(
    ...(await runStepsSerial([autoRotateStep(args)], {
      dryRun: args.dryRun,
      timingSummary: timingSummary(args, "full-rust-test:auto-rotate"),
    })),
  );

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
