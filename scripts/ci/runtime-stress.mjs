#!/usr/bin/env node
import { spawn } from "node:child_process";
import {
  RUNTIME_STRESS_CONTINUATION_TESTS,
  RUNTIME_STRESS_SERIALIZED_TESTS,
  RUNTIME_STRESS_SKIP_TESTS,
} from "./runtime-test-manifest.mjs";

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
    const child = spawn(command, args, { stdio: "inherit" });
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

await main();
