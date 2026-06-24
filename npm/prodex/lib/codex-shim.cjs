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

function currentPlatformTarget() {
  return PLATFORM_TARGETS[process.platform]?.[process.arch] ?? null;
}

function resolveOpenAiCodexPackageRoot() {
  let packageJsonPath;
  try {
    packageJsonPath = requireFromHere.resolve("@openai/codex/package.json");
  } catch {
    return null;
  }
  return path.dirname(packageJsonPath);
}

function codexManagedEnv() {
  const packageRoot = resolveOpenAiCodexPackageRoot();
  if (!packageRoot) {
    return process.env;
  }
  return {
    ...process.env,
    CODEX_MANAGED_BY_NPM: "1",
    CODEX_MANAGED_PACKAGE_ROOT: fs.realpathSync(packageRoot),
  };
}

function ensureNativeCodexExecutable(nativeBinaryPath) {
  try {
    fs.accessSync(nativeBinaryPath, fs.constants.X_OK);
    return;
  } catch {
    repairNativeCodexExecutablePermissionBestEffort(nativeBinaryPath);
  }

  try {
    fs.accessSync(nativeBinaryPath, fs.constants.X_OK);
    return;
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

function repairNativeCodexExecutablePermissionBestEffort(nativeBinaryPath) {
  if (process.platform === "win32") {
    return;
  }
  try {
    const stats = fs.statSync(nativeBinaryPath);
    if (!stats.isFile()) {
      return;
    }
    fs.chmodSync(nativeBinaryPath, (stats.mode & 0o777) | 0o755);
  } catch {
    // The explicit executable check below reports the actionable failure.
  }
}

function resolveNativeCodexCommand() {
  const platformTarget = currentPlatformTarget();
  if (!platformTarget) {
    return null;
  }

  let platformPackageJsonPath;
  try {
    platformPackageJsonPath = requireFromHere.resolve(`${platformTarget.packageName}/package.json`);
  } catch {
    return null;
  }

  const nativeBinaryPath = path.join(
    path.dirname(platformPackageJsonPath),
    "vendor",
    platformTarget.targetTriple,
    "bin",
    platformTarget.binaryFileName,
  );
  if (!fs.existsSync(nativeBinaryPath)) {
    process.stderr.write(
      [
        `Missing bundled Codex native binary at ${nativeBinaryPath}`,
        "Reinstall @christiandoxa/prodex with optional dependencies enabled, or set PRODEX_CODEX_BIN to an existing Codex CLI.",
        "",
      ].join("\n"),
    );
    process.exit(1);
  }
  ensureNativeCodexExecutable(nativeBinaryPath);

  return {
    command: nativeBinaryPath,
    args: process.argv.slice(2),
    env: codexManagedEnv(),
  };
}

function resolveCodexCommand() {
  const nativeCommand = resolveNativeCodexCommand();
  if (nativeCommand) {
    return nativeCommand;
  }

  process.stderr.write(
    [
      "Unable to locate bundled Codex native package for this platform.",
      "Reinstall @christiandoxa/prodex with optional dependencies enabled, or set PRODEX_CODEX_BIN to an existing Codex CLI.",
      "Prodex does not fall back to @openai/codex/bin/codex.js because npm/Node version skew can make Codex load the wrong optional native package.",
      "",
    ].join("\n"),
  );
  process.exit(1);
}

const codexCommand = resolveCodexCommand();
const child = spawn(codexCommand.command, codexCommand.args, {
  env: codexCommand.env,
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
