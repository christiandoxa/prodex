import { spawnSync } from "node:child_process";

const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_OUTPUT_BYTES = 4 * 1024 * 1024;
const DIAGNOSTIC_LIMIT = 2_048;

function redactedDiagnostic(value) {
  return String(value ?? "")
    .slice(0, DIAGNOSTIC_LIMIT)
    .replace(/\bBearer\s+\S+/giu, "Bearer <redacted>")
    .replace(/([?&](?:api_?key|token|secret)=)[^&\s]+/giu, "$1<redacted>")
    .trim();
}

function failure(kind, command, detail, result = {}) {
  const stderr = redactedDiagnostic(result.stderr);
  const suffix = stderr ? `; stderr: ${stderr}` : "";
  const error = new Error(`${command} ${kind}: ${detail}${suffix}`);
  error.name = "CheckedSubprocessError";
  error.kind = kind;
  error.status = result.status ?? null;
  error.signal = result.signal ?? null;
  return error;
}

export function runChecked(command, args = [], options = {}) {
  const timeout = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  const maxBuffer = options.maxOutputBytes ?? DEFAULT_MAX_OUTPUT_BYTES;
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: options.env,
    encoding: "utf8",
    timeout,
    maxBuffer,
    windowsHide: true,
  });

  if (result.error) {
    if (result.error.code === "ETIMEDOUT") {
      throw failure("timed out", command, `after ${timeout}ms`, result);
    }
    if (result.error.code === "ENOENT") {
      throw failure("could not start", command, "executable not found", result);
    }
    if (result.error.code === "ENOBUFS") {
      throw failure("output limit exceeded", command, `${maxBuffer} bytes`, result);
    }
    throw failure("could not start", command, result.error.message, result);
  }
  if (result.signal) {
    throw failure("terminated by signal", command, result.signal, result);
  }
  if (result.status !== 0) {
    throw failure("failed", command, `exit status ${result.status}`, result);
  }
  return { stdout: result.stdout, stderr: result.stderr };
}

export function runCheckedJson(command, args = [], options = {}) {
  const result = runChecked(command, args, options);
  try {
    return JSON.parse(result.stdout);
  } catch (error) {
    throw failure("returned invalid JSON", command, error.message, result);
  }
}
