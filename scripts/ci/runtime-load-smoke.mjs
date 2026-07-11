#!/usr/bin/env node
import { spawn } from "node:child_process";

const DEFAULT_TIMEOUT_MS = 30_000;

function parseArgs(argv) {
  const args = {
    mode: "mock",
    prodex: "target/debug/prodex",
    requests: 32,
    concurrency: 4,
    timeoutMs: DEFAULT_TIMEOUT_MS,
    dryRun: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (
      value === "--mode" ||
      value === "--prodex" ||
      value === "--requests" ||
      value === "--concurrency" ||
      value === "--timeout-ms"
    ) {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      const key = value.slice(2).replace(/-([a-z])/g, (_, char) => char.toUpperCase());
      args[key] = argv[index];
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  args.requests = Number(args.requests);
  args.concurrency = Number(args.concurrency);
  args.timeoutMs = Number(args.timeoutMs);
  if (!Number.isInteger(args.requests) || args.requests < 1) {
    throw new Error("--requests must be a positive integer");
  }
  if (!Number.isInteger(args.concurrency) || args.concurrency < 1) {
    throw new Error("--concurrency must be a positive integer");
  }
  if (!Number.isInteger(args.timeoutMs) || args.timeoutMs < 1_000) {
    throw new Error("--timeout-ms must be at least 1000");
  }
  if (!["mock", "proxy"].includes(args.mode)) {
    throw new Error("--mode must be mock or proxy");
  }
  return args;
}

function commandFor(args) {
  const command = [
    "tests/load/runtime-proxy-load.mjs",
    "--scenario",
    "baseline",
    "--start-mock",
    "--requests",
    String(args.requests),
    "--concurrency",
    String(args.concurrency),
    "--max-error-rate",
    "0",
    "--max-admission-pressure-rate",
    "0",
  ];

  if (args.mode === "proxy") {
    command.push(
      "--start-proxy",
      "--prodex",
      args.prodex,
      "--profiles",
      "2",
      "--max-ttft-p95-ms",
      "1500",
    );
  } else {
    command.push("--max-ttft-p95-ms", "350");
  }

  return command;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/runtime-load-smoke.mjs [--mode mock|proxy] [--requests N] [--concurrency N]",
      "",
      "Runs a bounded baseline load smoke through tests/load/runtime-proxy-load.mjs.",
      "Default mock mode is CI-safe and does not build or launch prodex.",
      "Proxy mode is for local preflight after building target/debug/prodex.",
    ].join("\n") + "\n",
  );
}

async function run(args) {
  const command = commandFor(args);
  process.stdout.write(`runtime-load-smoke command=node ${command.join(" ")}\n`);
  if (args.dryRun) {
    return 0;
  }

  return await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, command, {
      stdio: "inherit",
    });
    const timeout = setTimeout(() => {
      child.kill("SIGTERM");
      setTimeout(() => {
        if (child.exitCode === null) {
          child.kill("SIGKILL");
        }
      }, 2_000).unref();
    }, args.timeoutMs);
    timeout.unref();

    child.once("error", reject);
    child.once("exit", (code, signal) => {
      clearTimeout(timeout);
      if (signal) {
        resolve(1);
        return;
      }
      resolve(code ?? 1);
    });
  });
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  process.exitCode = await run(args);
}

main().catch((error) => {
  process.stderr.write(`runtime-load-smoke: ${error.message}\n`);
  process.exitCode = 1;
});
