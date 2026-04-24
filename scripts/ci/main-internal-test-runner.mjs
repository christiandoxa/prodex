import { spawn } from "node:child_process";
import os from "node:os";

const ZERO_TESTS_PATTERN = /\brunning 0 tests\b/;

export function parsePositiveInteger(value, name) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1) {
    throw new Error(`${name} must be a positive integer`);
  }
  return parsed;
}

export function defaultJobCount() {
  const available = typeof os.availableParallelism === "function" ? os.availableParallelism() : os.cpus().length;
  return Math.max(1, Math.min(4, available || 1));
}

export function formatCommand(command, args) {
  return [command, ...args].join(" ");
}

export function cargoTestStep(label, filter, extraArgs = []) {
  return {
    label,
    command: "cargo",
    args: ["test", "--lib", filter, "--", "--test-threads=1", ...extraArgs],
    failOnZeroTests: true,
  };
}

export function cargoIntegrationTestStep(label, testName, harnessArgs = []) {
  return {
    label,
    command: "cargo",
    args: ["test", "--test", testName, ...(harnessArgs.length > 0 ? ["--", ...harnessArgs] : [])],
    failOnZeroTests: true,
  };
}

export function skipArgs(testNames) {
  return testNames.flatMap((testName) => ["--skip", testName]);
}

export async function loadRuntimeManifest() {
  try {
    return await import("./runtime-test-manifest.mjs");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`warning: runtime test manifest unavailable; manifest-driven skips disabled (${message})\n`);
    return {};
  }
}

export function manifestArray(manifest, exportName, fallback = []) {
  const value = manifest[exportName];
  if (Array.isArray(value)) {
    return value;
  }
  process.stderr.write(`warning: runtime test manifest export ${exportName} unavailable; using fallback\n`);
  return fallback;
}

function formatStep(step) {
  return formatCommand(step.command, step.args);
}

function createPrefixForwarder(label, writer) {
  let tail = "";
  return {
    write(chunk) {
      tail += chunk;
      const lines = tail.split(/\r?\n/);
      tail = lines.pop() ?? "";
      for (const line of lines) {
        writer.write(`[${label}] ${line}\n`);
      }
    },
    flush() {
      if (tail.length > 0) {
        writer.write(`[${label}] ${tail}\n`);
        tail = "";
      }
    },
  };
}

async function runStepOnce(step) {
  return new Promise((resolve, reject) => {
    const startedAt = Date.now();
    process.stdout.write(`${step.label}: ${formatStep(step)}\n`);
    const child = spawn(step.command, step.args, {
      cwd: process.cwd(),
      env: {
        ...process.env,
        CARGO_TERM_COLOR: process.env.CARGO_TERM_COLOR ?? "always",
        RUST_BACKTRACE: process.env.RUST_BACKTRACE ?? "1",
        ...(step.env ?? {}),
      },
      stdio: ["ignore", "pipe", "pipe"],
    });

    let sawZeroTests = false;
    let probeTail = "";
    const inspectOutput = (chunk) => {
      const text = probeTail + chunk;
      if (ZERO_TESTS_PATTERN.test(text)) {
        sawZeroTests = true;
      }
      probeTail = text.slice(-64);
    };

    const stdout = createPrefixForwarder(step.label, process.stdout);
    const stderr = createPrefixForwarder(step.label, process.stderr);

    child.stdout?.setEncoding("utf8");
    child.stderr?.setEncoding("utf8");
    child.stdout?.on("data", (chunk) => {
      inspectOutput(chunk);
      stdout.write(chunk);
    });
    child.stderr?.on("data", (chunk) => {
      inspectOutput(chunk);
      stderr.write(chunk);
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      stdout.flush();
      stderr.flush();
      const elapsedMs = Date.now() - startedAt;
      if (signal) {
        reject(new Error(`${step.label} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(new Error(`${step.label} exited with code ${code}`));
        return;
      }
      if (step.failOnZeroTests && (sawZeroTests || ZERO_TESTS_PATTERN.test(probeTail))) {
        reject(new Error(`${step.label} matched no tests (cargo reported "running 0 tests")`));
        return;
      }
      process.stdout.write(`${step.label}: passed in ${elapsedMs} ms\n`);
      resolve();
    });
  });
}

export async function runStep(step) {
  const attempts = step.attempts ?? 1;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      if (attempt > 1) {
        process.stdout.write(`${step.label}: retry attempt ${attempt}/${attempts}\n`);
      }
      await runStepOnce(step);
      return;
    } catch (error) {
      if (attempt === attempts) {
        throw error;
      }
      const delayMs = step.retryDelayMs ?? 5000;
      await new Promise((resolve) => setTimeout(resolve, delayMs));
    }
  }
}

export async function runStepsSerial(steps, { dryRun = false } = {}) {
  if (dryRun) {
    process.stdout.write(`dry-run: ${steps.length} serial step(s)\n`);
    for (const step of steps) {
      process.stdout.write(`  ${step.label}: ${formatStep(step)}\n`);
    }
    return;
  }

  for (const step of steps) {
    await runStep(step);
  }
}

export async function runStepsParallel(steps, { jobs, dryRun = false } = {}) {
  const requestedJobs = jobs ?? defaultJobCount();
  if (dryRun) {
    process.stdout.write(`dry-run: ${steps.length} parallel step(s), jobs=${requestedJobs}\n`);
    for (const step of steps) {
      process.stdout.write(`  ${step.label}: ${formatStep(step)}\n`);
    }
    return;
  }

  const queue = [...steps];
  const failures = [];
  const workerCount = Math.max(1, Math.min(requestedJobs, steps.length || 1));
  await Promise.all(
    Array.from({ length: workerCount }, async () => {
      while (queue.length > 0) {
        const step = queue.shift();
        try {
          await runStep(step);
        } catch (error) {
          failures.push(error);
        }
      }
    }),
  );

  if (failures.length > 0) {
    throw new Error(
      [
        `${failures.length} step(s) failed:`,
        ...failures.map((error) => `  - ${error instanceof Error ? error.message : String(error)}`),
      ].join("\n"),
    );
  }
}
