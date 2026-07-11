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
  return Math.max(1, Math.min(6, available || 1));
}

export function formatCommand(command, args) {
  return [command, ...args].join(" ");
}

export function cargoFeatureArgs({ allFeatures = false } = {}) {
  return allFeatures ? ["--all-features"] : [];
}

export function cargoTestStep(label, filter, extraArgs = [], options = {}) {
  return {
    label,
    command: "cargo",
    args: ["test", "-p", "prodex-app", ...cargoFeatureArgs(options), "--lib", filter, "--", "--test-threads=1", ...extraArgs],
    failOnZeroTests: true,
  };
}

export function cargoIntegrationTestStep(label, testName, harnessArgs = [], options = {}) {
  return {
    label,
    command: "cargo",
    args: [
      "test",
      "--test",
      testName,
      ...cargoFeatureArgs(options),
      ...(harnessArgs.length > 0 ? ["--", ...harnessArgs] : []),
    ],
    failOnZeroTests: true,
  };
}

export function cargoIntegrationTestFilterStep(label, testName, filter, harnessArgs = [], options = {}) {
  return {
    label,
    command: "cargo",
    args: [
      "test",
      "--test",
      testName,
      ...cargoFeatureArgs(options),
      filter,
      "--",
      ...harnessArgs,
    ],
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

export function formatDurationMs(durationMs) {
  if (!Number.isFinite(durationMs) || durationMs < 0) {
    return "unknown";
  }
  if (durationMs < 1000) {
    return `${Math.round(durationMs)}ms`;
  }

  const totalSeconds = Math.round(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes === 0) {
    return `${seconds}s`;
  }
  return `${minutes}m ${String(seconds).padStart(2, "0")}s`;
}

export function sortedStepTimings(timings) {
  return [...timings].sort((left, right) => right.elapsedMs - left.elapsedMs || left.label.localeCompare(right.label));
}

export function formatStepTimingSummary(timings, { label = "test-runner", limit = 10, json = false } = {}) {
  if (timings.length === 0) {
    return `${label}: no completed step timings\n`;
  }

  const slowest = sortedStepTimings(timings).slice(0, Math.max(1, limit));
  const totalMs = timings.reduce((total, timing) => total + timing.elapsedMs, 0);
  const lines = [
    `${label}: ${timings.length} completed step(s), summed runtime ${formatDurationMs(totalMs)}, slowest ${slowest.length}:`,
    ...slowest.map(
      (timing, index) =>
        `  ${index + 1}. ${timing.label}: ${formatDurationMs(timing.elapsedMs)} (${timing.elapsedMs} ms)`,
    ),
  ];

  if (json) {
    lines.push(
      `${label}: timings-json ${JSON.stringify(
        sortedStepTimings(timings).map((timing) => ({
          label: timing.label,
          elapsedMs: timing.elapsedMs,
          attempts: timing.attempts,
        })),
      )}`,
    );
  }

  return `${lines.join("\n")}\n`;
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
      resolve({
        label: step.label,
        elapsedMs,
      });
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
      const timing = await runStepOnce(step);
      return {
        ...timing,
        attempts: attempt,
      };
    } catch (error) {
      if (attempt === attempts) {
        throw error;
      }
      const delayMs = step.retryDelayMs ?? 5000;
      await new Promise((resolve) => setTimeout(resolve, delayMs));
    }
  }
}

export async function runStepsSerial(steps, { dryRun = false, timingSummary = null } = {}) {
  if (dryRun) {
    process.stdout.write(`dry-run: ${steps.length} serial step(s)\n`);
    for (const step of steps) {
      process.stdout.write(`  ${step.label}: ${formatStep(step)}\n`);
    }
    return [];
  }

  const timings = [];
  for (const step of steps) {
    timings.push(await runStep(step));
  }
  if (timingSummary) {
    process.stdout.write(formatStepTimingSummary(timings, timingSummary));
  }
  return timings;
}

export async function runStepsParallel(steps, { jobs, dryRun = false, timingSummary = null } = {}) {
  const requestedJobs = jobs ?? defaultJobCount();
  if (dryRun) {
    process.stdout.write(`dry-run: ${steps.length} parallel step(s), jobs=${requestedJobs}\n`);
    for (const step of steps) {
      process.stdout.write(`  ${step.label}: ${formatStep(step)}\n`);
    }
    return [];
  }

  const queue = [...steps];
  const failures = [];
  const timings = [];
  const workerCount = Math.max(1, Math.min(requestedJobs, steps.length || 1));
  await Promise.all(
    Array.from({ length: workerCount }, async () => {
      while (queue.length > 0) {
        const step = queue.shift();
        try {
          timings.push(await runStep(step));
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
  if (timingSummary) {
    process.stdout.write(formatStepTimingSummary(timings, timingSummary));
  }
  return timings;
}
