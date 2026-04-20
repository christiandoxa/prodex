#!/usr/bin/env node
import { spawn } from "node:child_process";

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
  const claudeEnvFilter = "main_internal_tests::runtime_proxy_claude_";
  const workerCountFilter = "main_internal_tests::runtime_proxy_worker_count_env_override_beats_policy_file";

  for (let iteration = 1; iteration <= args.runs; iteration += 1) {
    process.stdout.write(`env-sensitive parallel guard iteration ${iteration}/${args.runs}\n`);
    await run(
      "cargo",
      ["test", "--lib", claudeEnvFilter, "--", threadArg],
      "env-sensitive-claude",
    );
    await run(
      "cargo",
      ["test", "--lib", workerCountFilter, "--", threadArg],
      "env-sensitive-worker-count",
    );
  }
}

await main();
