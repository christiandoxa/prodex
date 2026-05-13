#!/usr/bin/env node
import { spawn } from "node:child_process";
import { pathToFileURL } from "node:url";
import {
  RUNTIME_STRESS_DEFAULT_WEIGHT_SECONDS,
  RUNTIME_STRESS_CONTINUATION_TESTS,
  RUNTIME_STRESS_SERIALIZED_TESTS,
  RUNTIME_STRESS_SKIP_TESTS,
  RUNTIME_STRESS_WEIGHT_HINTS,
} from "./runtime-test-manifest.mjs";

const VALID_SUITES = new Set(["stress", "serialized", "continuation", "all"]);
const SHARD_STRATEGY_ALIASES = new Map([
  ["duration", "weighted"],
  ["durations", "weighted"],
  ["weight", "weighted"],
  ["weighted", "weighted"],
  ["index", "modulo"],
  ["legacy", "modulo"],
  ["modulo", "modulo"],
]);
const ZERO_TESTS_PATTERN = /\brunning 0 tests\b/;

function parsePositiveInteger(value, name) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error(`${name} must be a positive integer`);
  }
  return parsed;
}

function parseShardIndex(value, shardCount) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0 || parsed >= shardCount) {
    throw new Error(`--shard-index must be an integer between 0 and ${shardCount - 1}`);
  }
  return parsed;
}

function parseShardStrategy(value) {
  const strategy = SHARD_STRATEGY_ALIASES.get(value);
  if (!strategy) {
    throw new Error(
      `--shard-strategy must be one of: ${Array.from(SHARD_STRATEGY_ALIASES.keys()).join(", ")}`,
    );
  }
  return strategy;
}

function parseArgs(argv) {
  const args = {
    suite: "all",
    shardCount: 1,
    shardIndex: 0,
    shardStrategy: "weighted",
    dryRun: false,
  };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--suite") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--suite requires a value");
      }
      args.suite = argv[index];
      continue;
    }
    if (value === "--shard-count") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--shard-count requires a value");
      }
      args.shardCount = parsePositiveInteger(argv[index], value);
      continue;
    }
    if (value === "--shard-index") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--shard-index requires a value");
      }
      args.shardIndexRaw = argv[index];
      continue;
    }
    if (value === "--shard-strategy" || value === "--shard-mode") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.shardStrategy = parseShardStrategy(argv[index]);
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
  args.shardIndex = parseShardIndex(args.shardIndexRaw ?? String(args.shardIndex), args.shardCount);
  return args;
}

function formatCommand(command, args) {
  const text = [command, ...args].join(" ");
  const maxLength = 1200;
  if (text.length <= maxLength) {
    return text;
  }
  return `${text.slice(0, maxLength)} ... (${args.length} args, ${text.length} chars)`;
}

function dryRun(command, args, label) {
  process.stdout.write(`dry-run ${label}: ${formatCommand(command, args)}\n`);
}

function run(command, args, label) {
  return new Promise((resolve, reject) => {
    process.stdout.write(`${label}: ${formatCommand(command, args)}\n`);
    const child = spawn(command, args, { stdio: ["inherit", "pipe", "pipe"] });
    let sawZeroTests = false;
    let outputTail = "";
    const inspectOutput = (chunk) => {
      const text = outputTail + chunk;
      if (ZERO_TESTS_PATTERN.test(text)) {
        sawZeroTests = true;
      }
      outputTail = text.slice(-32);
    };
    child.stdout?.setEncoding("utf8");
    child.stderr?.setEncoding("utf8");
    child.stdout?.on("data", (chunk) => {
      process.stdout.write(chunk);
      inspectOutput(chunk);
    });
    child.stderr?.on("data", (chunk) => {
      process.stderr.write(chunk);
      inspectOutput(chunk);
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${label} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(new Error(`${label} exited with code ${code}`));
        return;
      }
      if (sawZeroTests || ZERO_TESTS_PATTERN.test(outputTail)) {
        reject(new Error(`${label} matched no tests (cargo reported "running 0 tests")`));
        return;
      }
      resolve();
    });
  });
}

function capture(command, args, label) {
  return new Promise((resolve, reject) => {
    process.stdout.write(`${label}: ${formatCommand(command, args)}\n`);
    const child = spawn(command, args, { stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout?.setEncoding("utf8");
    child.stderr?.setEncoding("utf8");
    child.stdout?.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr?.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${label} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        process.stderr.write(stderr);
        reject(new Error(`${label} exited with code ${code}`));
        return;
      }
      resolve(stdout);
    });
  });
}

async function retry(label, attemptCount, action) {
  for (let attempt = 1; attempt <= attemptCount; attempt += 1) {
    try {
      await action(attempt);
      return;
    } catch (error) {
      if (attempt === attemptCount) {
        throw error;
      }
      process.stdout.write(`${label}: retrying after transient failure\n`);
      await new Promise((resolve) => setTimeout(resolve, 5000));
    }
  }
}

function skipArgs(testNames) {
  return testNames.flatMap((testName) => ["--skip", testName]);
}

function baseStressArgs(extraSkipTests = []) {
  return [
    "test",
    "-p",
    "prodex-app",
    "--lib",
    "main_internal_tests::runtime_proxy_",
    "--",
    "--test-threads=1",
    ...skipArgs([...RUNTIME_STRESS_SKIP_TESTS, ...extraSkipTests]),
  ];
}

function parseListedTests(output) {
  return output
    .split(/\r?\n/)
    .map((line) => line.match(/^(main_internal_tests::.*): test$/)?.[1])
    .filter(Boolean);
}

function skippedByManifest(testName) {
  return RUNTIME_STRESS_SKIP_TESTS.some((skipName) => testName.includes(skipName));
}

function testLeafName(testName) {
  return testName.split("::").at(-1);
}

function weightHintMatches(hint, testName) {
  if (typeof hint.name === "string") {
    return testLeafName(testName) === hint.name;
  }
  return typeof hint.filter === "string" && testName.includes(hint.filter);
}

export function runtimeStressWeightSeconds(
  testName,
  weightHints = RUNTIME_STRESS_WEIGHT_HINTS,
  defaultWeightSeconds = RUNTIME_STRESS_DEFAULT_WEIGHT_SECONDS,
) {
  return weightHints.reduce((weightSeconds, hint) => {
    if (!weightHintMatches(hint, testName)) {
      return weightSeconds;
    }
    return Math.max(weightSeconds, hint.weightSeconds);
  }, defaultWeightSeconds);
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

function weightedTestRecords(testNames, weightHints, defaultWeightSeconds) {
  return testNames.map((testName, index) => ({
    index,
    testName,
    weightSeconds: runtimeStressWeightSeconds(testName, weightHints, defaultWeightSeconds),
  }));
}

function emptyShards(shardCount) {
  return Array.from({ length: shardCount }, (_, index) => ({
    index,
    tests: [],
    weightSeconds: 0,
  }));
}

export function weightedRuntimeStressShards(
  testNames,
  shardCount,
  weightHints = RUNTIME_STRESS_WEIGHT_HINTS,
  defaultWeightSeconds = RUNTIME_STRESS_DEFAULT_WEIGHT_SECONDS,
) {
  const shards = emptyShards(shardCount);
  const tests = weightedTestRecords(testNames, weightHints, defaultWeightSeconds);

  for (const test of tests.toSorted((left, right) => {
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

export function moduloRuntimeStressShards(
  testNames,
  shardCount,
  weightHints = RUNTIME_STRESS_WEIGHT_HINTS,
  defaultWeightSeconds = RUNTIME_STRESS_DEFAULT_WEIGHT_SECONDS,
) {
  const shards = emptyShards(shardCount);
  for (const test of weightedTestRecords(testNames, weightHints, defaultWeightSeconds)) {
    const target = shards[test.index % shardCount];
    target.tests.push(test);
    target.weightSeconds += test.weightSeconds;
  }
  return shards;
}

function runtimeStressShardPlan(testNames, shardCount, shardStrategy) {
  if (shardStrategy === "modulo") {
    return moduloRuntimeStressShards(testNames, shardCount);
  }
  return weightedRuntimeStressShards(testNames, shardCount);
}

function formatWeight(value) {
  return value.toFixed(1).replace(/\.0$/, "");
}

export function formatRuntimeStressShardPlan(
  shards,
  shardIndex,
  shardStrategy,
  runnableTestCount,
  skippedTestCount,
) {
  const totalWeightSeconds = shards.reduce((total, shard) => total + shard.weightSeconds, 0);
  const lines = shards.map(
    (shard) =>
      `runtime-stress: ${shardStrategy} shard ${shard.index + 1}/${shards.length} has ${shard.tests.length} test(s), estimated ${formatWeight(shard.weightSeconds)}s`,
  );
  const selectedShard = shards[shardIndex];
  lines.push(
    `runtime-stress: selected ${shardStrategy} shard ${shardIndex + 1}/${shards.length} with ${selectedShard.tests.length}/${runnableTestCount} test(s), estimated ${formatWeight(selectedShard.weightSeconds)}/${formatWeight(totalWeightSeconds)}s; manifest skipped ${skippedTestCount}`,
  );
  return lines.join("\n");
}

function printRuntimeStressShardPlan(shards, shardIndex, shardStrategy, runnableTestCount, skippedTestCount) {
  process.stdout.write(
    `${formatRuntimeStressShardPlan(shards, shardIndex, shardStrategy, runnableTestCount, skippedTestCount)}\n`,
  );
}

function assertShardSkipSafety(selectedTests, nonSelectedTests) {
  for (const skipName of nonSelectedTests) {
    const selectedMatch = selectedTests.find((testName) => testName.includes(skipName));
    if (selectedMatch) {
      throw new Error(`shard skip filter would also skip selected test: ${skipName} -> ${selectedMatch}`);
    }
  }
}

async function listRuntimeStressTests() {
  const output = await capture(
    "cargo",
    ["test", "-p", "prodex-app", "--lib", "main_internal_tests::runtime_proxy_", "--", "--list"],
    "runtime-stress:list",
  );
  const tests = parseListedTests(output);
  if (tests.length === 0) {
    throw new Error("runtime-stress:list matched no runtime proxy tests");
  }
  return tests;
}

async function runStressSuite({ shardIndex, shardCount, shardStrategy, dryRun: dryRunMode }) {
  if (shardCount === 1) {
    const args = baseStressArgs();
    if (dryRunMode) {
      dryRun("cargo", args, "runtime-stress");
      return;
    }
    await run("cargo", args, "runtime-stress");
    return;
  }

  const listedTests = await listRuntimeStressTests();
  const runnableTests = listedTests.filter((testName) => !skippedByManifest(testName));
  const shards = runtimeStressShardPlan(runnableTests, shardCount, shardStrategy);
  const selectedShard = shards[shardIndex];
  const selectedTests = selectedShard.tests.map((test) => test.testName);
  if (selectedTests.length === 0) {
    throw new Error(`runtime-stress shard ${shardIndex + 1}/${shardCount} selected no tests`);
  }
  const selected = new Set(selectedTests);
  const nonSelectedTests = runnableTests.filter((testName) => !selected.has(testName));
  assertShardSkipSafety(selectedTests, nonSelectedTests);
  printRuntimeStressShardPlan(
    shards,
    shardIndex,
    shardStrategy,
    runnableTests.length,
    listedTests.length - runnableTests.length,
  );

  const args = baseStressArgs(nonSelectedTests);
  if (dryRunMode) {
    process.stdout.write(`runtime-stress: first selected test: ${selectedTests[0]}\n`);
    dryRun("cargo", args, `runtime-stress:${shardIndex + 1}/${shardCount}`);
    return;
  }
  await run("cargo", args, "runtime-stress");
}

function testsForShard(testNames, shardIndex, shardCount, label) {
  const selectedTests = testNames.filter((_, index) => index % shardCount === shardIndex);
  if (selectedTests.length === 0) {
    throw new Error(`runtime-stress ${label} shard ${shardIndex + 1}/${shardCount} selected no tests`);
  }
  process.stdout.write(
    `${label} shard ${shardIndex + 1}/${shardCount} selected ${selectedTests.length}/${testNames.length} test(s)\n`,
  );
  return selectedTests;
}

async function runSerializedSuite({ shardIndex, shardCount, dryRun: dryRunMode }) {
  const serializedTests = testsForShard(
    RUNTIME_STRESS_SERIALIZED_TESTS,
    shardIndex,
    shardCount,
    "serialized runtime stress",
  );
  await retry("serialized runtime stress", 2, async (attempt) => {
    process.stdout.write(`serialized runtime stress attempt ${attempt}\n`);
    for (const testName of serializedTests) {
      const args = ["test", "-p", "prodex-app", "--lib", testName, "--", "--test-threads=1"];
      if (dryRunMode) {
        dryRun("cargo", args, testName);
      } else {
        await run("cargo", args, testName);
      }
    }
  });
}

async function runContinuationSuite({ shardIndex, shardCount, dryRun: dryRunMode }) {
  const continuationTests = testsForShard(
    RUNTIME_STRESS_CONTINUATION_TESTS,
    shardIndex,
    shardCount,
    "continuation-heavy",
  );
  for (let iteration = 1; iteration <= 2; iteration += 1) {
    process.stdout.write(`continuation-heavy iteration ${iteration}\n`);
    for (const testName of continuationTests) {
      const args = ["test", "-p", "prodex-app", "--lib", testName, "--", "--test-threads=1"];
      if (dryRunMode) {
        dryRun("cargo", args, testName);
      } else {
        await run("cargo", args, testName);
      }
    }
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/ci/runtime-stress.mjs [--suite stress|serialized|continuation|all] [--shard-index <n> --shard-count <n>] [--shard-strategy weighted|modulo] [--dry-run]",
        "",
        "Runs runtime proxy stress shards from the shared runtime CI manifest.",
        "Broad stress sharding defaults to duration-weighted assignment; --shard-strategy modulo keeps the legacy listed-test index split.",
        "Serialized and continuation-heavy suites keep their manifest order modulo split.",
      ].join("\n") + "\n",
    );
    return;
  }

  if (!VALID_SUITES.has(args.suite)) {
    throw new Error(
      `invalid --suite value: ${args.suite}. Expected one of: ${Array.from(VALID_SUITES).join(", ")}`,
    );
  }

  if (args.suite === "stress" || args.suite === "all") {
    await runStressSuite(args);
  }
  if (args.suite === "serialized" || args.suite === "all") {
    await runSerializedSuite(args);
  }
  if (args.suite === "continuation" || args.suite === "all") {
    await runContinuationSuite(args);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    await main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`runtime-stress: ${message}\n`);
    process.exitCode = 1;
  }
}
