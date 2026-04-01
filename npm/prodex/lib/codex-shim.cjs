#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawn } = require("node:child_process");
const { createRequire } = require("node:module");

const requireFromHere = createRequire(__filename);

function resolveCodexBin() {
  let packageJsonPath;
  try {
    packageJsonPath = requireFromHere.resolve("@openai/codex/package.json");
  } catch {
    process.stderr.write(
      "Unable to locate @openai/codex. Reinstall @christiandoxa/prodex so its runtime dependency is present.\n",
    );
    process.exit(1);
  }

  const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
  if (typeof packageJson.bin === "string") {
    return path.resolve(path.dirname(packageJsonPath), packageJson.bin);
  }
  if (packageJson.bin && typeof packageJson.bin === "object") {
    const candidate = packageJson.bin.codex ?? Object.values(packageJson.bin)[0];
    if (typeof candidate === "string") {
      return path.resolve(path.dirname(packageJsonPath), candidate);
    }
  }
  return path.resolve(path.dirname(packageJsonPath), "bin", "codex.js");
}

const codexBin = resolveCodexBin();
const child = spawn(process.execPath, [codexBin, ...process.argv.slice(2)], {
  env: process.env,
  stdio: "inherit",
});

child.on("error", (error) => {
  process.stderr.write(`${error.message}\n`);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});
