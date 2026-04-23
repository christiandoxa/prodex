#!/usr/bin/env node
import { spawn } from "node:child_process";
import { RUNTIME_ENV_PARALLEL_CASES } from "./runtime-test-manifest.mjs";

const ZERO_TESTS_PATTERN = /\brunning 0 tests\b/;

function parseArgs(argv) {
  const args = { runs: 2, testThreads: 4 };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--runs") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--runs requires a value");
      }
      args.runs = Number(argv[index]);
      continue;
    }
    if (value === "--test-threads") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--test-threads requires a value");
      }
      args.testThreads = Number(argv[index]);
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  if (!args.help) {
    if (!Number.isInteger(args.runs) || args.runs < 1) {
      throw new Error("--runs must be a positive integer");
    }
    if (!Number.isInteger(args.testThreads) || args.testThreads < 1) {
      throw new Error("--test-threads must be a positive integer");
    }
  }

  return args;
}

function createZeroTestProbe() {
  let sawZeroTests = false;
  let tail = "";

  return {
    inspect(chunk) {
      const text = tail + chunk;
      if (ZERO_TESTS_PATTERN.test(text)) {
        sawZeroTests = true;
      }
      tail = text.slice(-32);
    },
    sawZeroTests() {
      return sawZeroTests || ZERO_TESTS_PATTERN.test(tail);
    },
  };
}

function run(command, args, label) {
  return new Promise((resolve, reject) => {
    process.stdout.write(`${label}: ${command} ${args.join(" ")}\n`);
    const child = spawn(command, args, { stdio: ["inherit", "pipe", "pipe"] });
    const zeroTestProbe = createZeroTestProbe();

    child.stdout?.setEncoding("utf8");
    child.stderr?.setEncoding("utf8");

    child.stdout?.on("data", (chunk) => {
      process.stdout.write(chunk);
      zeroTestProbe.inspect(chunk);
    });
    child.stderr?.on("data", (chunk) => {
      process.stderr.write(chunk);
      zeroTestProbe.inspect(chunk);
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
      if (zeroTestProbe.sawZeroTests()) {
        reject(new Error(`${label} matched no tests (cargo reported "running 0 tests")`));
        return;
      }
      resolve();
    });
  });
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/ci/runtime-env-parallel.mjs [--runs <n>] [--test-threads <n>]",
        "",
        "Runs env-sensitive runtime proxy tests with parallel test harness scheduling.",
      ].join("\n") + "\n",
    );
    return;
  }

  const threadArg = `--test-threads=${args.testThreads}`;

  for (let iteration = 1; iteration <= args.runs; iteration += 1) {
    process.stdout.write(`env-sensitive parallel guard iteration ${iteration}/${args.runs}\n`);
    for (const testCase of RUNTIME_ENV_PARALLEL_CASES) {
      await run("cargo", ["test", "--lib", testCase.filter, "--", threadArg], testCase.label);
    }
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`runtime-env-parallel: ${message}\n`);
  process.exitCode = 1;
}
