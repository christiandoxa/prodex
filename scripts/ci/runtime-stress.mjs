#!/usr/bin/env node
import { spawn } from "node:child_process";
import {
  RUNTIME_STRESS_CONTINUATION_TESTS,
  RUNTIME_STRESS_SERIALIZED_TESTS,
  RUNTIME_STRESS_SKIP_TESTS,
} from "./runtime-test-manifest.mjs";

const VALID_SUITES = new Set(["stress", "serialized", "continuation", "all"]);
const ZERO_TESTS_PATTERN = /\brunning 0 tests\b/;

function parseArgs(argv) {
  const args = { suite: "all" };
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
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function run(command, args, label) {
  return new Promise((resolve, reject) => {
    process.stdout.write(`${label}: ${command} ${args.join(" ")}\n`);
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

async function runStressSuite() {
  const args = [
    "test",
    "--lib",
    "main_internal_tests::runtime_proxy_",
    "--",
    "--test-threads=1",
    ...RUNTIME_STRESS_SKIP_TESTS.flatMap((testName) => ["--skip", testName]),
  ];
  await run("cargo", args, "runtime-stress");
}

async function runSerializedSuite() {
  await retry("serialized runtime stress", 2, async (attempt) => {
    process.stdout.write(`serialized runtime stress attempt ${attempt}\n`);
    for (const testName of RUNTIME_STRESS_SERIALIZED_TESTS) {
      await run("cargo", ["test", "--lib", `main_internal_tests::${testName}`, "--", "--test-threads=1"], testName);
    }
  });
}

async function runContinuationSuite() {
  for (let iteration = 1; iteration <= 2; iteration += 1) {
    process.stdout.write(`continuation-heavy iteration ${iteration}\n`);
    for (const testName of RUNTIME_STRESS_CONTINUATION_TESTS) {
      await run("cargo", ["test", "--lib", `main_internal_tests::${testName}`, "--", "--test-threads=1"], testName);
    }
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/ci/runtime-stress.mjs [--suite stress|serialized|continuation|all]",
        "",
        "Runs runtime proxy stress shards from the shared runtime CI manifest.",
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
    await runStressSuite();
  }
  if (args.suite === "serialized" || args.suite === "all") {
    await runSerializedSuite();
  }
  if (args.suite === "continuation" || args.suite === "all") {
    await runContinuationSuite();
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`runtime-stress: ${message}\n`);
  process.exitCode = 1;
}
