#!/usr/bin/env node
import {
  cargoIntegrationTestFilterStep,
  defaultJobCount,
  formatStepTimingSummary,
  parsePositiveInteger,
  runStep,
} from "./main-internal-test-runner.mjs";

const AUTO_ROTATE_SHARDS = Object.freeze([
  {
    id: "run-blocked-current",
    label: "run current blocked rotation",
    filters: ["run::run_auto_rotates_active_profile_when_current_is_blocked"],
  },
  {
    id: "run-blocked-flag",
    label: "run auto-rotate flag rotation",
    filters: ["run::run_auto_rotate_flag_rotates_active_profile_when_current_is_blocked"],
  },
  {
    id: "run-explicit-default",
    label: "run explicit profile default rotation",
    filters: ["run::explicit_profile_auto_rotates_by_default"],
  },
  {
    id: "run-without-profile",
    label: "run without profile ready account",
    filters: ["run::run_without_profile_keeps_the_active_ready_account"],
  },
  {
    id: "run-broker-recovery",
    label: "run broker recovery",
    filters: ["run::run_recovers_when_runtime_broker_registry_points_to_a_dead_pid"],
    exclusive: true,
  },
  {
    id: "run-fast-passthrough",
    label: "run fast passthrough cases",
    filters: [
      "run::run_exec_preserves_prompt_and_piped_stdin",
      "run::explicit_profile_can_disable_auto_rotate",
      "run::run_preflight_checks_fallback_profiles_in_parallel",
    ],
  },
  {
    id: "login",
    label: "login",
    filters: ["login::"],
  },
  {
    id: "quota-doctor",
    label: "quota doctor",
    filters: ["quota_doctor::"],
  },
  {
    id: "shared-state-history",
    label: "shared state resume history",
    filters: ["shared_state::run_shares_resume_history_across_managed_profiles"],
  },
  {
    id: "shared-state-memories",
    label: "shared state housekeeping memories",
    filters: ["shared_state::run_shares_housekeeping_memories_across_managed_profiles"],
  },
  {
    id: "shared-state-native",
    label: "shared state native Codex behavior",
    filters: ["shared_state::run_shares_native_codex_behavior_state_across_managed_profiles"],
  },
  {
    id: "shared-state-plugins",
    label: "shared state plugins and memory extensions",
    filters: ["shared_state::run_shares_codex_plugin_and_memory_extension_state_across_managed_profiles"],
  },
  {
    id: "super-mode",
    label: "super mode",
    filters: ["super_mode::"],
  },
]);

function parseArgs(argv) {
  const args = {
    allFeatures: false,
    dryRun: false,
    jobs: defaultJobCount(),
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
    if (value === "--all-features") {
      args.allFeatures = true;
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
      "Usage: node scripts/ci/auto-rotate-shards.mjs [--jobs <n>] [--all-features] [--timings] [--timings-json] [--timings-limit <n>] [--dry-run]",
      "",
      "Runs auto_rotate integration tests as parallel shard groups.",
      "Each shard keeps its selected tests serial with --test-threads=1.",
    ].join("\n") + "\n",
  );
}

function filterLabel(filter) {
  return filter.replaceAll("::", ".").replaceAll(/[^A-Za-z0-9_.-]/g, "_");
}

function shardSteps(shard, options) {
  return shard.filters.map((filter) =>
    cargoIntegrationTestFilterStep(
      `auto-rotate:${shard.id}:${filterLabel(filter)}`,
      "auto_rotate",
      filter,
      ["--test-threads=1"],
      options,
    ),
  );
}

async function runShard(shard, options) {
  const startedAt = Date.now();
  const timings = [];
  process.stdout.write(`auto-rotate:${shard.id}: ${shard.label}\n`);
  for (const step of shardSteps(shard, options)) {
    if (options.dryRun) {
      process.stdout.write(`  ${step.label}: ${step.command} ${step.args.join(" ")}\n`);
      continue;
    }
    timings.push(await runStep(step));
  }
  return {
    attempts: 1,
    elapsedMs: Date.now() - startedAt,
    label: `auto-rotate:${shard.id}`,
    stepTimings: timings,
  };
}

async function runShardsParallel(shards, options) {
  if (options.dryRun) {
    const exclusiveCount = shards.filter((shard) => shard.exclusive).length;
    process.stdout.write(
      `dry-run: ${shards.length} auto-rotate shard(s), jobs=${options.jobs}, exclusive=${exclusiveCount}\n`,
    );
  }

  const exclusiveShards = shards.filter((shard) => shard.exclusive);
  const queue = shards.filter((shard) => !shard.exclusive);
  const failures = [];
  const shardTimings = [];
  const stepTimings = [];
  const workerCount = Math.max(1, Math.min(options.jobs, queue.length || 1));

  for (const shard of exclusiveShards) {
    try {
      const timing = await runShard(shard, options);
      shardTimings.push(timing);
      stepTimings.push(...timing.stepTimings);
    } catch (error) {
      failures.push(error);
    }
  }

  if (failures.length > 0) {
    throw new Error(
      [
        `${failures.length} auto-rotate shard(s) failed:`,
        ...failures.map((error) => `  - ${error instanceof Error ? error.message : String(error)}`),
      ].join("\n"),
    );
  }

  await Promise.all(
    Array.from({ length: workerCount }, async () => {
      while (queue.length > 0) {
        const shard = queue.shift();
        try {
          const timing = await runShard(shard, options);
          shardTimings.push(timing);
          stepTimings.push(...timing.stepTimings);
        } catch (error) {
          failures.push(error);
        }
      }
    }),
  );

  if (failures.length > 0) {
    throw new Error(
      [
        `${failures.length} auto-rotate shard(s) failed:`,
        ...failures.map((error) => `  - ${error instanceof Error ? error.message : String(error)}`),
      ].join("\n"),
    );
  }

  if (options.timings && !options.dryRun) {
    process.stdout.write(
      formatStepTimingSummary(shardTimings, {
        label: "auto-rotate-shards",
        limit: options.timingsLimit,
        json: options.timingsJson,
      }),
    );
    process.stdout.write(
      formatStepTimingSummary(stepTimings, {
        label: "auto-rotate-steps",
        limit: options.timingsLimit,
        json: options.timingsJson,
      }),
    );
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  await runShardsParallel(AUTO_ROTATE_SHARDS, args);
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`auto-rotate-shards: ${message}\n`);
  process.exitCode = 1;
}
