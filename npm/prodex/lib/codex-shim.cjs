#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawn } = require("node:child_process");
const { createRequire } = require("node:module");

const requireFromHere = createRequire(__filename);

const PLATFORM_TARGETS = {
  linux: {
    x64: {
      packageName: "@openai/codex-linux-x64",
      targetTriple: "x86_64-unknown-linux-musl",
      binaryFileName: "codex",
    },
    arm64: {
      packageName: "@openai/codex-linux-arm64",
      targetTriple: "aarch64-unknown-linux-musl",
      binaryFileName: "codex",
    },
  },
  darwin: {
    x64: {
      packageName: "@openai/codex-darwin-x64",
      targetTriple: "x86_64-apple-darwin",
      binaryFileName: "codex",
    },
    arm64: {
      packageName: "@openai/codex-darwin-arm64",
      targetTriple: "aarch64-apple-darwin",
      binaryFileName: "codex",
    },
  },
  win32: {
    x64: {
      packageName: "@openai/codex-win32-x64",
      targetTriple: "x86_64-pc-windows-msvc",
      binaryFileName: "codex.exe",
    },
    arm64: {
      packageName: "@openai/codex-win32-arm64",
      targetTriple: "aarch64-pc-windows-msvc",
      binaryFileName: "codex.exe",
    },
  },
};

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

function explainBundledNativeCodexPermissionIssue() {
  const platformTarget = PLATFORM_TARGETS[process.platform]?.[process.arch];
  if (!platformTarget || process.platform === "win32") {
    return;
  }

  let platformPackageJsonPath;
  try {
    platformPackageJsonPath = requireFromHere.resolve(`${platformTarget.packageName}/package.json`);
  } catch {
    return;
  }

  const nativeBinaryPath = path.join(
    path.dirname(platformPackageJsonPath),
    "vendor",
    platformTarget.targetTriple,
    "bin",
    platformTarget.binaryFileName,
  );
  if (!fs.existsSync(nativeBinaryPath)) {
    return;
  }
  try {
    fs.accessSync(nativeBinaryPath, fs.constants.X_OK);
  } catch {
    process.stderr.write(
      [
        `Bundled Codex native binary is not executable: ${nativeBinaryPath}`,
        "Reinstall @christiandoxa/prodex with optional dependencies enabled, or set PRODEX_CODEX_BIN to an existing Codex CLI.",
        "",
      ].join("\n"),
    );
    process.exit(126);
  }
}

const codexBin = resolveCodexBin();
explainBundledNativeCodexPermissionIssue();
const child = spawn(process.execPath, [codexBin, ...process.argv.slice(2)], {
  env: process.env,
  stdio: "inherit",
});

child.on("error", (error) => {
  if (error && error.code === "EACCES") {
    process.stderr.write(
      `${error.message}\nSet PRODEX_CODEX_BIN to an existing Codex CLI or reinstall @christiandoxa/prodex.\n`,
    );
  } else {
    process.stderr.write(`${error.message}\n`);
  }
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});
