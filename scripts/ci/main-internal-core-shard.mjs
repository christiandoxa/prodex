#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { parsePositiveInteger, runStep } from "./main-internal-test-runner.mjs";

const MAIN_INTERNAL_FILTER = "main_internal_tests::";
const CORE_TEST_PATTERN = /^main_internal_tests::.*: test$/;
const DEFAULT_WEIGHT_SECONDS = 1;

// Duration hints come from local timing and recent CI imbalance. Unknown tests are
// cheap pure unit tests and use DEFAULT_WEIGHT_SECONDS.
const TEST_WEIGHT_SECONDS = new Map([
  ["main_internal_tests::info_and_broker::runtime_no_proxy_policy_does_not_leak_into_default_proxy_mode", 5.5],
  ["main_internal_tests::runtime_broker_registry::find_compatible_runtime_broker_registry_discovers_other_broker_key", 5.5],
  ["main_internal_tests::runtime_broker_registry::runtime_broker_command_registers_follower_when_owner_lock_is_busy", 5.5],
  [
    "main_internal_tests::runtime_broker_registry::wait_for_existing_runtime_broker_recovery_or_exit_yields_after_live_unhealthy_registry_clears",
    6.5,
  ],
  ["main_internal_tests::smart_context_and_broker::runtime_rotation_proxy_can_bind_a_requested_listen_addr", 5.5],
  [
    "main_internal_tests::smart_context_and_broker::runtime_smart_context_proxy_disabled_passes_large_tool_output_unchanged",
    5.5,
  ],
  [
    "main_internal_tests::smart_context_and_broker::runtime_smart_context_proxy_rewrites_large_tool_output_and_logs_budget",
    7.0,
  ],
  ["main_internal_tests::update_doctor_launch::runtime_doctor_summary_uses_configured_tail_bytes", 32.5],
]);

function parseShardIndex(value, shardCount) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0 || parsed >= shardCount) {
    throw new Error(`--shard-index must be an integer between 0 and ${shardCount - 1}`);
  }
  return parsed;
}

function requiredValue(argv, index, name) {
  const value = argv[index];
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function parseArgs(argv) {
  const args = {
    shardCount: 1,
    shardIndexRaw: "0",
    dryRun: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--shard-count") {
      index += 1;
      args.shardCount = parsePositiveInteger(requiredValue(argv, index, value), value);
      continue;
    }
    if (value === "--shard-index") {
      index += 1;
      args.shardIndexRaw = requiredValue(argv, index, value);
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

  args.shardIndex = parseShardIndex(args.shardIndexRaw, args.shardCount);
  return args;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/main-internal-core-shard.mjs --shard-index <n> --shard-count <n> [--dry-run]",
      "",
      "Runs one weighted main_internal_tests:: non-runtime-proxy shard.",
      "",
      "The shard planner uses duration hints for known slow tests and greedily balances total estimated runtime.",
    ].join("\n") + "\n",
  );
}

function listedCoreTests() {
  const result = spawnSync("cargo", ["test", "--lib", MAIN_INTERNAL_FILTER, "--", "--list"], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`cargo test --list exited with code ${result.status}`);
  }
  return result.stdout
    .split(/\r?\n/)
    .filter((line) => CORE_TEST_PATTERN.test(line))
    .map((line) => line.replace(/: test$/, ""))
    .filter((testName) => !testName.includes("runtime_proxy_"));
}

function testWeightSeconds(testName) {
  return TEST_WEIGHT_SECONDS.get(testName) ?? DEFAULT_WEIGHT_SECONDS;
}

function compareShardLoad(left, right) {
  if (left.weightSeconds !== right.weightSeconds) {
    return left.weightSeconds - right.weightSeconds;
  }
  if (left.tests.length !== right.tests.length) {
    return left.tests.length - right.tests.length;
  }
  return left.index - right.index;
}

export function weightedCoreShards(testNames, shardCount) {
  const indexedTests = testNames.map((testName, index) => ({
    index,
    testName,
    weightSeconds: testWeightSeconds(testName),
  }));
  const shards = Array.from({ length: shardCount }, (_, index) => ({
    index,
    tests: [],
    weightSeconds: 0,
  }));

  for (const test of indexedTests.toSorted((left, right) => {
    if (left.weightSeconds !== right.weightSeconds) {
      return right.weightSeconds - left.weightSeconds;
    }
    return left.index - right.index;
  })) {
    const target = shards.toSorted(compareShardLoad)[0];
    target.tests.push(test);
    target.weightSeconds += test.weightSeconds;
  }

  for (const shard of shards) {
    shard.tests.sort((left, right) => left.index - right.index);
  }
  return shards;
}

function assertSkipFiltersAreSafe(selectedTests, nonSelectedTests) {
  for (const skipTest of nonSelectedTests) {
    for (const selectedTest of selectedTests) {
      if (selectedTest.includes(skipTest)) {
        throw new Error(`shard skip filter would also skip selected test: ${skipTest} -> ${selectedTest}`);
      }
    }
  }
}

function skipArgs(testNames) {
  return testNames.flatMap((testName) => ["--skip", testName]);
}

function formatWeight(value) {
  return value.toFixed(1).replace(/\.0$/, "");
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const testNames = listedCoreTests();
  if (testNames.length === 0) {
    throw new Error("No main internal core tests found");
  }

  const shards = weightedCoreShards(testNames, args.shardCount);
  const selectedShard = shards[args.shardIndex];
  const selectedTests = selectedShard.tests.map((test) => test.testName);
  if (selectedTests.length === 0) {
    throw new Error(`No tests selected for main internal core shard ${args.shardIndex}/${args.shardCount}`);
  }
  const selectedSet = new Set(selectedTests);
  const nonSelectedTests = testNames.filter((testName) => !selectedSet.has(testName));
  assertSkipFiltersAreSafe(selectedTests, nonSelectedTests);

  for (const shard of shards) {
    process.stdout.write(
      `main-internal-core: shard ${shard.index + 1}/${args.shardCount} has ${shard.tests.length} test(s), estimated ${formatWeight(shard.weightSeconds)}s\n`,
    );
  }
  process.stdout.write(
    `main-internal-core: selected shard ${args.shardIndex + 1}/${args.shardCount} with ${selectedTests.length}/${testNames.length} test(s)\n`,
  );

  const step = {
    label: `main-internal-core:${args.shardIndex + 1}/${args.shardCount}`,
    command: "cargo",
    args: [
      "test",
      "--lib",
      MAIN_INTERNAL_FILTER,
      "--",
      "--test-threads=1",
      "--skip",
      "runtime_proxy_",
      ...skipArgs(nonSelectedTests),
    ],
    failOnZeroTests: true,
  };

  if (args.dryRun) {
    process.stdout.write(`dry-run ${step.label}: ${step.command} ${step.args.join(" ")}\n`);
    return;
  }

  await runStep(step);
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
