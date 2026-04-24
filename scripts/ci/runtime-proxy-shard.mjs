#!/usr/bin/env node
import { spawn } from "node:child_process";
import {
  createWriteStream,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const DEFAULT_FILTER = "main_internal_tests::runtime_proxy_";
const DEFAULT_TIMEOUT_MINUTES = 25;
const DEFAULT_TEST_THREADS = 1;
const RUNTIME_LOG_POINTER = "prodex-runtime-latest.path";
const RUNTIME_LOG_PREFIX = "prodex-runtime";
const ZERO_TESTS_PATTERN = /\brunning 0 tests\b/;

function parseArgs(argv) {
  const args = {
    filter: DEFAULT_FILTER,
    testThreads: DEFAULT_TEST_THREADS,
    timeoutMinutes: DEFAULT_TIMEOUT_MINUTES,
    artifactDir: path.resolve("target/ci/runtime-proxy"),
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--filter") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--filter requires a value");
      }
      args.filter = argv[index];
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
    if (value === "--timeout-minutes") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--timeout-minutes requires a value");
      }
      args.timeoutMinutes = Number(argv[index]);
      continue;
    }
    if (value === "--artifact-dir") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--artifact-dir requires a value");
      }
      args.artifactDir = path.resolve(argv[index]);
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  if (!args.help) {
    if (!Number.isInteger(args.testThreads) || args.testThreads < 1) {
      throw new Error("--test-threads must be a positive integer");
    }
    if (!Number.isFinite(args.timeoutMinutes) || args.timeoutMinutes <= 0) {
      throw new Error("--timeout-minutes must be a positive number");
    }
  }

  return args;
}

function formatCommand(command, args) {
  return [command, ...args].join(" ");
}

function tailLines(text, lineCount) {
  const lines = text.split(/\r?\n/);
  while (lines.length > 0 && lines.at(-1) === "") {
    lines.pop();
  }
  return lines.slice(-lineCount).join("\n");
}

function newestRuntimeLogPath(logDir) {
  const entries = readdirSync(logDir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.startsWith(RUNTIME_LOG_PREFIX) && entry.name.endsWith(".log"))
    .map((entry) => {
      const entryPath = path.join(logDir, entry.name);
      return {
        path: entryPath,
        mtimeMs: statSync(entryPath).mtimeMs,
      };
    })
    .sort((left, right) => right.mtimeMs - left.mtimeMs || right.path.localeCompare(left.path));
  return entries[0]?.path ?? null;
}

async function collectFailureDiagnostics({
  artifactDir,
  logDir,
  cargoLogPath,
  elapsedMs,
  filter,
  testThreads,
  timeoutMinutes,
  timedOut,
  exitCode,
  signal,
}) {
  const lines = [
    `filter=${filter}`,
    `test_threads=${testThreads}`,
    `timeout_minutes=${timeoutMinutes}`,
    `elapsed_ms=${elapsedMs}`,
    `timed_out=${timedOut}`,
    `exit_code=${exitCode ?? "null"}`,
    `signal=${signal ?? "null"}`,
    `artifact_dir=${artifactDir}`,
    `runtime_log_dir=${logDir}`,
    `cargo_log=${cargoLogPath}`,
  ];

  let latestLogPath = null;
  const pointerPath = path.join(logDir, RUNTIME_LOG_POINTER);
  if (statExists(pointerPath)) {
    const pointerTarget = readFileSync(pointerPath, "utf8").trim();
    if (pointerTarget) {
      latestLogPath = pointerTarget;
      lines.push(`runtime_log_pointer=${pointerPath}`);
      lines.push(`runtime_log_pointer_target=${pointerTarget}`);
    }
  } else {
    lines.push(`runtime_log_pointer=${pointerPath}`);
    lines.push("runtime_log_pointer_target=missing");
  }

  if (!latestLogPath && statExists(logDir)) {
    latestLogPath = newestRuntimeLogPath(logDir);
  }

  if (statExists(logDir)) {
    const logEntries = readdirSync(logDir)
      .sort((left, right) => left.localeCompare(right))
      .map((entry) => `log_entry=${path.join(logDir, entry)}`);
    lines.push(...logEntries);
  }

  if (latestLogPath && statExists(latestLogPath)) {
    const latestLog = await readFile(latestLogPath, "utf8");
    const latestLogTail = tailLines(latestLog, 200);
    const latestLogTailPath = path.join(artifactDir, "latest-runtime-log-tail.txt");
    await writeFile(latestLogTailPath, `${latestLogTail}\n`);
    lines.push(`latest_runtime_log=${latestLogPath}`);
    lines.push(`latest_runtime_log_tail=${latestLogTailPath}`);
    process.stdout.write("runtime proxy shard: latest runtime log tail\n");
    process.stdout.write(`${latestLogTail}\n`);
  } else {
    lines.push("latest_runtime_log=missing");
    process.stdout.write("runtime proxy shard: no runtime log found\n");
  }

  await writeFile(path.join(artifactDir, "diagnostics.txt"), `${lines.join("\n")}\n`);
}

function statExists(targetPath) {
  try {
    statSync(targetPath);
    return true;
  } catch {
    return false;
  }
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
      tail = text.slice(-64);
    },
    sawZeroTests() {
      return sawZeroTests || ZERO_TESTS_PATTERN.test(tail);
    },
  };
}

async function runShard(args) {
  const artifactDir = args.artifactDir;
  const logDir = path.join(artifactDir, "runtime-logs");
  rmSync(artifactDir, { recursive: true, force: true });
  mkdirSync(logDir, { recursive: true });

  const cargoArgs = [
    "test",
    "--lib",
    args.filter,
    "--",
    `--test-threads=${args.testThreads}`,
  ];
  const command = formatCommand("cargo", cargoArgs);
  const cargoLogPath = path.join(artifactDir, "cargo-test.log");
  const metadataPath = path.join(artifactDir, "metadata.txt");
  const timeoutMs = Math.round(args.timeoutMinutes * 60_000);
  const startedAt = new Date().toISOString();
  writeFileSync(
    metadataPath,
    [
      `started_at=${startedAt}`,
      `timeout_minutes=${args.timeoutMinutes}`,
      `artifact_dir=${artifactDir}`,
      `runtime_log_dir=${logDir}`,
      `command=${command}`,
    ].join("\n") + "\n",
  );

  process.stdout.write(`runtime proxy shard: ${command}\n`);
  process.stdout.write(`runtime proxy shard: runtime logs -> ${logDir}\n`);
  process.stdout.write(`runtime proxy shard: timeout -> ${args.timeoutMinutes} minute(s)\n`);

  const child = spawn("cargo", cargoArgs, {
    cwd: process.cwd(),
    detached: process.platform !== "win32",
    env: {
      ...process.env,
      CARGO_TERM_COLOR: process.env.CARGO_TERM_COLOR ?? "always",
      PRODEX_RUNTIME_LOG_DIR: logDir,
      RUST_BACKTRACE: process.env.RUST_BACKTRACE ?? "1",
      RUST_TEST_THREADS: String(args.testThreads),
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const cargoLog = createWriteStream(cargoLogPath, { flags: "a" });
  const zeroTestProbe = createZeroTestProbe();

  const forward = (stream, writer) =>
    new Promise((resolve, reject) => {
      if (!stream) {
        resolve();
        return;
      }
      stream.setEncoding("utf8");
      stream.on("data", (chunk) => {
        zeroTestProbe.inspect(chunk);
        writer.write(chunk);
        cargoLog.write(chunk);
      });
      stream.on("error", reject);
      stream.on("end", resolve);
    });

  const stdoutForward = forward(child.stdout, process.stdout);
  const stderrForward = forward(child.stderr, process.stderr);

  let timedOut = false;
  let killTimer = null;
  const timeoutHandle = setTimeout(() => {
    timedOut = true;
    process.stderr.write(
      `runtime proxy shard: exceeded ${args.timeoutMinutes} minute(s); terminating test process\n`,
    );
    try {
      if (process.platform === "win32") {
        child.kill("SIGTERM");
      } else {
        process.kill(-child.pid, "SIGTERM");
      }
    } catch {
      child.kill("SIGTERM");
    }
    killTimer = setTimeout(() => {
      try {
        if (process.platform === "win32") {
          child.kill("SIGKILL");
        } else {
          process.kill(-child.pid, "SIGKILL");
        }
      } catch {
        child.kill("SIGKILL");
      }
    }, 10_000);
  }, timeoutMs);

  const result = await new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", (code, signal) => resolve({ code, signal }));
  });

  clearTimeout(timeoutHandle);
  if (killTimer) {
    clearTimeout(killTimer);
  }
  await Promise.all([stdoutForward, stderrForward]);
  await new Promise((resolve) => cargoLog.end(resolve));

  const elapsedMs = Date.now() - Date.parse(startedAt);
  writeFileSync(
    metadataPath,
    [
      `started_at=${startedAt}`,
      `finished_at=${new Date().toISOString()}`,
      `elapsed_ms=${elapsedMs}`,
      `timeout_minutes=${args.timeoutMinutes}`,
      `artifact_dir=${artifactDir}`,
      `runtime_log_dir=${logDir}`,
      `command=${command}`,
      `exit_code=${result.code ?? "null"}`,
      `signal=${result.signal ?? "null"}`,
      `timed_out=${timedOut}`,
    ].join("\n") + "\n",
  );

  if (result.code === 0 && !timedOut && !zeroTestProbe.sawZeroTests()) {
    process.stdout.write(`runtime proxy shard: passed in ${elapsedMs} ms\n`);
    return;
  }

  if (zeroTestProbe.sawZeroTests()) {
    process.stderr.write('runtime proxy shard: matched no tests (cargo reported "running 0 tests")\n');
  }

  await collectFailureDiagnostics({
    artifactDir,
    logDir,
    cargoLogPath,
    elapsedMs,
    filter: args.filter,
    testThreads: args.testThreads,
    timeoutMinutes: args.timeoutMinutes,
    timedOut,
    exitCode: result.code,
    signal: result.signal,
  });

  if (timedOut) {
    process.exitCode = 124;
    return;
  }
  process.exitCode = zeroTestProbe.sawZeroTests() ? 1 : (result.code ?? 1);
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/ci/runtime-proxy-shard.mjs [--filter <cargo-filter>] [--test-threads <n>] [--timeout-minutes <n>] [--artifact-dir <path>]",
        "",
        "Runs the runtime proxy CI shard once with fixed logging, timeout, and failure diagnostics.",
      ].join("\n") + "\n",
    );
    return;
  }

  await runShard(args);
}

await main();
