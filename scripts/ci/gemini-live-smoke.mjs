#!/usr/bin/env node
import { spawn } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const DEFAULT_TIMEOUT_MS = 180_000;
const DEFAULT_EXTENDED_TIMEOUT_MS = 300_000;

function prodexBinary() {
  if (process.env.PRODEX_BIN) {
    return process.env.PRODEX_BIN;
  }
  return existsSync("target/debug/prodex") ? "target/debug/prodex" : "prodex";
}

function baseArgs() {
  const args = ["s", "--provider", "gemini", "--no-presidio"];
  if (process.env.PRODEX_LIVE_GEMINI_MODEL) {
    args.push("--model", process.env.PRODEX_LIVE_GEMINI_MODEL);
  }
  return args;
}

function simpleCommand(marker) {
  const args = baseArgs();
  args.push("exec", `Reply with exactly one line containing: ${marker}`);
  return { binary: prodexBinary(), args, cwd: process.cwd() };
}

function extendedEditCommand(marker, workspace) {
  const args = baseArgs();
  args.push(
    "exec",
    [
      "In this workspace, create or overwrite gemini-smoke.txt with exactly these two lines:",
      `marker=${marker}`,
      "status=tool-edit-ok",
      "",
      "Then run this verification command from the workspace:",
      "node -e \"const fs=require('fs'); const s=fs.readFileSync('gemini-smoke.txt','utf8'); if(!s.includes('status=tool-edit-ok')) process.exit(2);\"",
      "",
      `Reply with exactly one line containing: ${marker}`,
    ].join("\n"),
  );
  return { binary: prodexBinary(), args, cwd: workspace };
}

function extendedCompactReadCommand(marker, workspace) {
  const args = baseArgs();
  args.push("--context-window", "4096", "--auto-compact-token-limit", "1200");
  const filler = "retain compact smoke context ".repeat(900);
  args.push(
    "exec",
    [
      filler,
      "",
      "Read gemini-smoke.txt from the workspace, run a lightweight verification command if useful, and keep the existing file unchanged.",
      `Reply with exactly one line containing: ${marker}`,
    ].join("\n"),
  );
  return { binary: prodexBinary(), args, cwd: workspace };
}

function timeoutMs(defaultMs) {
  const value = Number(process.env.PRODEX_LIVE_GEMINI_TIMEOUT_MS ?? defaultMs);
  if (!Number.isInteger(value) || value < 10_000) {
    throw new Error("PRODEX_LIVE_GEMINI_TIMEOUT_MS must be an integer of at least 10000");
  }
  return value;
}

function argsForLog(args) {
  return args
    .map((arg) => (arg.length > 180 ? `${arg.slice(0, 180)}...<${arg.length} chars>` : arg))
    .join(" ");
}

async function runProdex({ binary, args, cwd }, marker, label, timeout) {
  process.stdout.write(`gemini-live-smoke ${label} command=${binary} ${argsForLog(args)}\n`);
  return await new Promise((resolve, reject) => {
    const child = spawn(binary, args, {
      cwd,
      env: {
        ...process.env,
        NO_COLOR: "1",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let output = "";
    const collect = (chunk, stream) => {
      const text = chunk.toString();
      output += text;
      stream.write(text);
    };
    child.stdout.on("data", (chunk) => collect(chunk, process.stdout));
    child.stderr.on("data", (chunk) => collect(chunk, process.stderr));

    const timer = setTimeout(() => {
      child.kill("SIGTERM");
      setTimeout(() => {
        if (child.exitCode === null) {
          child.kill("SIGKILL");
        }
      }, 2_000).unref();
    }, timeout);
    timer.unref();

    child.once("error", reject);
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      if (signal) {
        reject(new Error(`prodex terminated by ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(new Error(`prodex exited with code ${code}`));
        return;
      }
      if (!output.includes(marker)) {
        reject(new Error(`Gemini response did not contain marker for ${label}`));
        return;
      }
      resolve(output);
    });
  });
}

async function run() {
  if (process.env.PRODEX_LIVE_GEMINI !== "1") {
    process.stdout.write(
      "gemini-live-smoke skipped: set PRODEX_LIVE_GEMINI=1 to use configured Gemini credentials\n",
    );
    return 0;
  }

  const marker = `PRODEX_GEMINI_LIVE_OK_${Date.now()}`;
  if (process.env.PRODEX_LIVE_GEMINI_EXTENDED !== "1") {
    await runProdex(simpleCommand(marker), marker, "simple", timeoutMs(DEFAULT_TIMEOUT_MS));
    process.stdout.write("gemini-live-smoke passed\n");
    return 0;
  }

  const workspace = mkdtempSync(join(tmpdir(), "prodex-gemini-live-"));
  try {
    writeFileSync(join(workspace, "gemini-smoke.txt"), "status=pending\n");
    const timeout = timeoutMs(DEFAULT_EXTENDED_TIMEOUT_MS);
    await runProdex(extendedEditCommand(marker, workspace), marker, "extended-edit", timeout);
    const file = readFileSync(join(workspace, "gemini-smoke.txt"), "utf8");
    if (!file.includes(`marker=${marker}`) || !file.includes("status=tool-edit-ok")) {
      throw new Error("extended Gemini smoke did not update gemini-smoke.txt as requested");
    }
    await runProdex(
      extendedCompactReadCommand(marker, workspace),
      marker,
      "extended-compact-read",
      timeout,
    );
    process.stdout.write("gemini-live-smoke extended passed\n");
    return 0;
  } finally {
    if (process.env.PRODEX_LIVE_GEMINI_KEEP_WORKSPACE !== "1") {
      rmSync(workspace, { recursive: true, force: true });
    } else {
      process.stdout.write(`gemini-live-smoke workspace kept at ${workspace}\n`);
    }
  }
}

run()
  .then((code) => {
    process.exitCode = code;
  })
  .catch((error) => {
    process.stderr.write(`gemini-live-smoke: ${error.message}\n`);
    process.exitCode = 1;
  });
