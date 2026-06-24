import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import {
  copyRepoFile,
  ensureDir,
  mainPackageManifest,
  platformPackages,
  platformPackageManifest,
  writeJsonFile,
} from "./common.mjs";

function packageInstallDir(root, packageName) {
  return path.join(root, "node_modules", ...packageName.split("/"));
}

function cleanEnv(overrides = {}) {
  const env = { ...process.env, ...overrides };
  for (const [key, value] of Object.entries(env)) {
    if (value === undefined) {
      delete env[key];
    }
  }
  return env;
}

function hostOpenAiCodexPlatformSpec() {
  const specs = {
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
  return specs[process.platform]?.[process.arch] ?? null;
}

async function writeExecutable(filePath, contents) {
  await ensureDir(path.dirname(filePath));
  await fs.writeFile(filePath, contents);
  await fs.chmod(filePath, 0o755);
}

async function stageCodexShimInstall() {
  if (process.platform === "win32") {
    return null;
  }
  const spec = hostOpenAiCodexPlatformSpec();
  if (!spec) {
    return null;
  }

  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-codex-shim-test-"));
  const mainPackageDir = packageInstallDir(root, "@christiandoxa/prodex");
  await ensureDir(path.join(mainPackageDir, "lib"));
  await copyRepoFile(
    "npm/prodex/lib/codex-shim.cjs",
    path.join(mainPackageDir, "lib", "codex-shim.cjs"),
  );

  const openAiCodexDir = packageInstallDir(root, "@openai/codex");
  await ensureDir(path.join(openAiCodexDir, "bin"));
  await writeJsonFile(path.join(openAiCodexDir, "package.json"), {
    name: "@openai/codex",
    version: "0.0.0-test",
    bin: {
      codex: "bin/codex.js",
    },
  });
  await writeExecutable(
    path.join(openAiCodexDir, "bin", "codex.js"),
    ["#!/usr/bin/env node", "throw new Error('codex js fallback should not run');", ""].join(
      "\n",
    ),
  );

  const platformPackageDir = packageInstallDir(root, spec.packageName);
  const nativeBinaryPath = path.join(
    platformPackageDir,
    "vendor",
    spec.targetTriple,
    "bin",
    spec.binaryFileName,
  );
  await writeJsonFile(path.join(platformPackageDir, "package.json"), {
    name: spec.packageName,
    version: "0.0.0-test",
  });
  await writeExecutable(
    nativeBinaryPath,
    [
      "#!/usr/bin/env node",
      "console.log(JSON.stringify({",
      "  argv: process.argv.slice(2),",
      "  managedByNpm: process.env.CODEX_MANAGED_BY_NPM || null,",
      "  managedPackageRoot: process.env.CODEX_MANAGED_PACKAGE_ROOT || null,",
      "}));",
      "",
    ].join("\n"),
  );

  return {
    root,
    shimPath: path.join(mainPackageDir, "lib", "codex-shim.cjs"),
    nativeBinaryPath,
    platformPackageDir,
    openAiCodexDir,
  };
}

async function stageWrapperInstall(version) {
  if (process.platform === "win32") {
    return null;
  }

  const spec = platformPackages.find(
    (entry) => entry.os === process.platform && entry.cpu === process.arch,
  );
  assert.ok(spec, `unsupported test platform ${process.platform} ${process.arch}`);

  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-wrapper-test-"));
  const mainPackageDir = packageInstallDir(root, "@christiandoxa/prodex");
  await ensureDir(path.join(mainPackageDir, "lib"));
  await copyRepoFile("npm/prodex/prodex", path.join(mainPackageDir, "prodex"));
  await copyRepoFile(
    "npm/prodex/lib/codex-shim.cjs",
    path.join(mainPackageDir, "lib", "codex-shim.cjs"),
  );
  await fs.chmod(path.join(mainPackageDir, "prodex"), 0o755);
  await fs.chmod(path.join(mainPackageDir, "lib", "codex-shim.cjs"), 0o755);
  await writeJsonFile(path.join(mainPackageDir, "package.json"), mainPackageManifest(version));

  const platformPackageDir = packageInstallDir(root, spec.packageName);
  await ensureDir(path.join(platformPackageDir, "vendor"));
  await writeJsonFile(
    path.join(platformPackageDir, "package.json"),
    platformPackageManifest(spec, version),
  );
  await writeExecutable(
    path.join(platformPackageDir, "vendor", spec.binaryFileName),
    [
      "#!/usr/bin/env node",
      "console.log(JSON.stringify({",
      "  codexBin: process.env.PRODEX_CODEX_BIN || null,",
      "  pathEntries: (process.env.PATH || '').split(require('node:path').delimiter),",
      "}));",
      "",
    ].join("\n"),
  );

  const externalDir = path.join(root, "external-bin");
  const externalCodex = path.join(externalDir, "codex");
  await writeExecutable(
    externalCodex,
    ["#!/bin/sh", "echo external codex should not be executed", "exit 77", ""].join("\n"),
  );

  return {
    root,
    launcherPath: path.join(mainPackageDir, "prodex"),
    externalDir,
    externalCodex,
  };
}

function runWrapper(install, env) {
  const result = spawnSync(process.execPath, [install.launcherPath, "--probe"], {
    encoding: "utf8",
    env,
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

test("prodex npm wrapper uses bundled Codex shim by default", async (t) => {
  const install = await stageWrapperInstall("0.0.0-test");
  if (!install) {
    t.skip("wrapper fake native binary test is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));

  const output = runWrapper(
    install,
    cleanEnv({
      PATH: `${install.externalDir}${path.delimiter}${process.env.PATH || ""}`,
      PRODEX_CODEX_BIN: undefined,
      PRODEX_CODEX_RESOLUTION: undefined,
    }),
  );

  assert.equal(output.codexBin, "codex");
  assert.match(output.pathEntries[0], /prodex-codex-/);
  assert.notEqual(output.pathEntries[0], install.externalDir);
});

test("prodex npm wrapper can opt into external Codex explicitly", async (t) => {
  const install = await stageWrapperInstall("0.0.0-test");
  if (!install) {
    t.skip("wrapper fake native binary test is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));

  const explicitBin = runWrapper(
    install,
    cleanEnv({
      PATH: `${install.externalDir}${path.delimiter}${process.env.PATH || ""}`,
      PRODEX_CODEX_BIN: install.externalCodex,
      PRODEX_CODEX_RESOLUTION: undefined,
    }),
  );
  assert.equal(explicitBin.codexBin, install.externalCodex);
  assert.equal(explicitBin.pathEntries[0], install.externalDir);

  const explicitMode = runWrapper(
    install,
    cleanEnv({
      PATH: `${install.externalDir}${path.delimiter}${process.env.PATH || ""}`,
      PRODEX_CODEX_BIN: undefined,
      PRODEX_CODEX_RESOLUTION: "external",
    }),
  );
  assert.equal(explicitMode.codexBin, install.externalCodex);
  assert.equal(explicitMode.pathEntries[0], install.externalDir);
});

test("prodex Codex shim prefers direct native Codex package", async (t) => {
  const install = await stageCodexShimInstall();
  if (!install) {
    t.skip("Codex shim native package test is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));

  const result = spawnSync(process.execPath, [install.shimPath, "run", "--probe"], {
    encoding: "utf8",
    env: cleanEnv({}),
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const output = JSON.parse(result.stdout);
  assert.deepEqual(output.argv, ["run", "--probe"]);
  assert.equal(output.managedByNpm, "1");
  assert.equal(output.managedPackageRoot, await fs.realpath(install.openAiCodexDir));
});

test("prodex Codex shim fails fast when bundled native package is missing", async (t) => {
  const install = await stageCodexShimInstall();
  if (!install) {
    t.skip("Codex shim native package test is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));
  await fs.rm(install.platformPackageDir, { recursive: true, force: true });

  const result = spawnSync(process.execPath, [install.shimPath, "--version"], {
    encoding: "utf8",
    env: cleanEnv({}),
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Unable to locate bundled Codex native package/);
  assert.ok(result.stderr.includes("does not fall back to @openai/codex/bin/codex.js"));
  assert.equal(result.stdout, "");
});

test("prodex Codex shim repairs non-executable native Codex binary", async (t) => {
  const install = await stageCodexShimInstall();
  if (!install) {
    t.skip("Codex shim native package test is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));
  await fs.chmod(install.nativeBinaryPath, 0o644);

  const result = spawnSync(process.execPath, [install.shimPath, "--version"], {
    encoding: "utf8",
    env: cleanEnv({}),
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const mode = (await fs.stat(install.nativeBinaryPath)).mode;
  assert.ok(mode & 0o111);
});
