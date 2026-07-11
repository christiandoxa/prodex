#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");
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

function platformSpec() {
  return PLATFORM_TARGETS[process.platform]?.[process.arch] || null;
}

function isExecutableFile(filePath) {
  try {
    const stats = fs.statSync(filePath);
    if (!stats.isFile()) {
      return false;
    }
    fs.accessSync(filePath, fs.constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

function tryMakeExecutable(filePath) {
  if (process.platform === "win32") {
    return;
  }
  try {
    const stats = fs.statSync(filePath);
    if (stats.isFile()) {
      fs.chmodSync(filePath, (stats.mode & 0o777) | 0o755);
    }
  } catch {
    // The explicit executable check below reports the actionable failure.
  }
}

function managedCodexPackageRoot(fallbackRoot) {
  try {
    return fs.realpathSync(path.dirname(requireFromHere.resolve("@openai/codex/package.json")));
  } catch {
    return fs.realpathSync(fallbackRoot);
  }
}

function nativePackagePaths(spec) {
  const packageJsonPath = requireFromHere.resolve(`${spec.packageName}/package.json`);
  const packageRoot = path.dirname(packageJsonPath);
  return {
    packageRoot,
    managedPackageRoot: managedCodexPackageRoot(packageRoot),
    binaryPath: path.join(
      packageRoot,
      "vendor",
      spec.targetTriple,
      "bin",
      spec.binaryFileName,
    ),
  };
}

function resolveNativeCodexCommand() {
  const spec = platformSpec();
  if (!spec) {
    return {
      ok: false,
      reason: `Unsupported Codex platform ${process.platform}/${process.arch}.`,
    };
  }

  let binaryPath;
  let packageRoot;
  let managedPackageRoot;
  try {
    ({ binaryPath, packageRoot, managedPackageRoot } = nativePackagePaths(spec));
  } catch (error) {
    return {
      ok: false,
      reason: [
        "Unable to locate bundled Codex native package for this platform.",
        `Missing bundled Codex native package ${spec.packageName}.`,
        `Original error: ${error && error.message ? error.message : String(error)}`,
      ].join("\n"),
    };
  }

  tryMakeExecutable(binaryPath);

  if (!isExecutableFile(binaryPath)) {
    return {
      ok: false,
      reason: `Bundled Codex native binary is not executable: ${binaryPath}`,
      binaryPath,
    };
  }

  return {
    ok: true,
    command: binaryPath,
    args: process.argv.slice(2),
    env: {
      CODEX_MANAGED_BY_NPM: "1",
      CODEX_MANAGED_PACKAGE_ROOT: managedPackageRoot,
    },
  };
}

function pathEntries() {
  return (process.env.PATH || "")
    .split(path.delimiter)
    .filter((entry) => entry.length > 0);
}

function commandNames(command) {
  if (process.platform !== "win32") {
    return [command];
  }
  const pathext = (process.env.PATHEXT || ".EXE;.CMD;.BAT;.COM")
    .split(";")
    .filter(Boolean);
  const lower = command.toLowerCase();
  if (pathext.some((ext) => lower.endsWith(ext.toLowerCase()))) {
    return [command];
  }
  return [command, ...pathext.map((ext) => `${command}${ext}`)];
}

function isSelfShim(candidate) {
  try {
    if (fs.realpathSync(candidate) === fs.realpathSync(__filename)) {
      return true;
    }
  } catch {
    // Continue with the lightweight content check.
  }
  try {
    const prefix = fs.readFileSync(candidate, "utf8").slice(0, 4096);
    return prefix.includes(__filename);
  } catch {
    return false;
  }
}

function candidateKey(candidate) {
  try {
    return fs.realpathSync(candidate).toLowerCase();
  } catch {
    return path.resolve(candidate).toLowerCase();
  }
}

function externalCodexCandidates() {
  const seen = new Set();
  const candidates = [];
  for (const dir of pathEntries()) {
    for (const name of commandNames("codex")) {
      const candidate = path.join(dir, name);
      const key = candidateKey(candidate);
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      candidates.push(candidate);
    }
  }
  return candidates;
}

function summarizeSkippedExternalCandidates(skipped) {
  if (skipped.length === 0) {
    return "No codex candidates were found on PATH.";
  }
  const shown = skipped.slice(0, 8).map(({ candidate, reason }) => `- ${candidate} (${reason})`);
  if (skipped.length > shown.length) {
    shown.push(`- ... ${skipped.length - shown.length} more skipped candidates`);
  }
  return ["Searched PATH for external codex candidates:", ...shown].join("\n");
}

function resolveExternalCodexCommand() {
  const skipped = [];
  for (const candidate of externalCodexCandidates()) {
    if (isSelfShim(candidate)) {
      skipped.push({ candidate, reason: "Prodex Codex shim" });
      continue;
    }
    if (!isExecutableFile(candidate)) {
      skipped.push({ candidate, reason: "not executable" });
      continue;
    }
    return {
      ok: true,
      command: candidate,
      args: process.argv.slice(2),
      searched: skipped,
    };
  }
  return {
    ok: false,
    reason: [
      "No executable external codex was found on PATH.",
      summarizeSkippedExternalCandidates(skipped),
    ].join("\n"),
    searched: skipped,
  };
}


function autoInstallCodexEnabled() {
  return ["1", "true", "yes"].includes(
    (process.env.PRODEX_CODEX_AUTO_INSTALL || "").toLowerCase(),
  );
}

function npmCommandNames() {
  return commandNames("npm");
}

function resolveNpmCommand() {
  const skipped = [];
  for (const dir of pathEntries()) {
    for (const name of npmCommandNames()) {
      const candidate = path.join(dir, name);
      if (!isExecutableFile(candidate)) {
        skipped.push({ candidate, reason: "not executable" });
        continue;
      }
      return { ok: true, command: candidate, searched: skipped };
    }
  }
  return { ok: false, reason: "No executable npm was found on PATH.", searched: skipped };
}

function installExternalCodexWithNpm() {
  if (!autoInstallCodexEnabled()) {
    return { ok: false, reason: "PRODEX_CODEX_AUTO_INSTALL is not enabled." };
  }

  const npmCommand = resolveNpmCommand();
  if (!npmCommand.ok) {
    return npmCommand;
  }

  process.stderr.write(
    [
      "No executable external codex was found; attempting npm install because PRODEX_CODEX_AUTO_INSTALL=1.",
      "Running: npm install -g @openai/codex@latest",
      "",
    ].join("\n"),
  );

  const result = spawnSync(npmCommand.command, ["install", "-g", "@openai/codex@latest"], {
    stdio: "inherit",
    env: process.env,
  });

  if (result.error) {
    return {
      ok: false,
      reason: `npm install failed to start: ${result.error.message}`,
    };
  }
  if (result.status !== 0) {
    return {
      ok: false,
      reason: `npm install exited with status ${result.status}`,
    };
  }

  return resolveExternalCodexCommand();
}

function resolveCodexCommand() {
  const nativeCommand = resolveNativeCodexCommand();
  if (nativeCommand.ok) {
    return nativeCommand;
  }

  let externalCommand = resolveExternalCodexCommand();
  if (!externalCommand.ok && autoInstallCodexEnabled()) {
    const installCommand = installExternalCodexWithNpm();
    if (installCommand.ok) {
      externalCommand = installCommand;
    } else {
      externalCommand = {
        ...externalCommand,
        reason: [externalCommand.reason, installCommand.reason].filter(Boolean).join("\n"),
      };
    }
  }

  if (externalCommand.ok) {
    process.stderr.write(
      [
        "Bundled Codex is unavailable; falling back to external codex from PATH.",
        nativeCommand.reason,
        summarizeSkippedExternalCandidates(externalCommand.searched || []),
        `External codex: ${externalCommand.command}`,
        "Set PRODEX_CODEX_BIN to pin a specific Codex executable.",
        "",
      ].join("\n"),
    );
    return externalCommand;
  }

  process.stderr.write(
    [
      nativeCommand.reason || "Unable to locate bundled Codex native package for this platform.",
      "Reinstall @christiandoxa/prodex with optional dependencies enabled, set PRODEX_CODEX_BIN to an existing Codex CLI, or set PRODEX_CODEX_AUTO_INSTALL=1 to let Prodex run npm install -g @openai/codex@latest.",
      "Prodex does not fall back to @openai/codex/bin/codex.js because npm/Node version skew can make Codex load the wrong optional native package.",
      externalCommand.reason,
      "",
    ].join("\n"),
  );
  process.exit(1);
}

function spawnCodex(codexCommand, retriedExternalFallback = false) {
  const child = spawn(codexCommand.command, codexCommand.args, {
    stdio: "inherit",
    env: {
      ...process.env,
      ...(codexCommand.env || {}),
    },
  });

  child.on("error", (error) => {
    if (error && error.code === "EACCES" && !retriedExternalFallback) {
      let externalCommand = resolveExternalCodexCommand();
      if (!externalCommand.ok && autoInstallCodexEnabled()) {
        const installCommand = installExternalCodexWithNpm();
        if (installCommand.ok) {
          externalCommand = installCommand;
        }
      }
      if (externalCommand.ok && externalCommand.command !== codexCommand.command) {
        process.stderr.write(
          [
            `${error.message}`,
            "Bundled Codex could not be executed; falling back to external codex from PATH.",
            summarizeSkippedExternalCandidates(externalCommand.searched || []),
            `External codex: ${externalCommand.command}`,
            "Set PRODEX_CODEX_BIN to pin a specific Codex executable.",
            "",
          ].join("\n"),
        );
        spawnCodex(externalCommand, true);
        return;
      }
    }

    if (error && error.code === "EACCES") {
      process.stderr.write(
        `${error.message}\nSet PRODEX_CODEX_BIN to an existing Codex CLI, set PRODEX_CODEX_AUTO_INSTALL=1 to let Prodex run npm install -g @openai/codex@latest, or reinstall @christiandoxa/prodex.\n`,
      );
      process.exit(126);
      return;
    }

    process.stderr.write(`${error && error.message ? error.message : String(error)}\n`);
    process.exit(1);
  });

  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 1);
  });
}

spawnCodex(resolveCodexCommand());
