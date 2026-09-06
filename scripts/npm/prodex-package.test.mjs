import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import {
  ensureDir,
  mainPackageManifest,
  packageSlug,
  platformPackages,
  platformPackageManifest,
  repoRoot,
  writeJsonFile,
} from "./common.mjs";
import { stagePackages } from "./stage.mjs";

function packageInstallDir(root, packageName) {
  return path.join(root, "node_modules", ...packageName.split("/"));
}

async function writeExecutable(filePath, contents) {
  await ensureDir(path.dirname(filePath));
  await fs.writeFile(filePath, contents);
  await fs.chmod(filePath, 0o755);
}

function cleanEnv(overrides = {}) {
  const env = { ...process.env, ...overrides };
  for (const [key, value] of Object.entries(env)) {
    if (value === undefined) delete env[key];
  }
  return env;
}

async function stageWrapperInstall(version) {
  if (process.platform === "win32") return null;
  const spec = platformPackages.find(
    (entry) => entry.os === process.platform && entry.cpu === process.arch,
  );
  assert.ok(spec, `unsupported test platform ${process.platform}/${process.arch}`);

  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-wrapper-test-"));
  const mainPackageDir = packageInstallDir(root, "@christiandoxa/prodex");
  await ensureDir(mainPackageDir);
  await fs.copyFile(path.join(repoRoot, "npm/prodex/prodex"), path.join(mainPackageDir, "prodex"));
  await fs.chmod(path.join(mainPackageDir, "prodex"), 0o755);
  await writeJsonFile(path.join(mainPackageDir, "package.json"), mainPackageManifest(version));

  const platformPackageDir = packageInstallDir(root, spec.packageName);
  await ensureDir(path.join(platformPackageDir, "vendor"));
  await writeJsonFile(path.join(platformPackageDir, "package.json"), platformPackageManifest(spec, version));
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
  await writeExecutable(externalCodex, "#!/bin/sh\nprintf 'codex 0.153.4\\n'\n");
  return { root, launcherPath: path.join(mainPackageDir, "prodex"), externalDir, externalCodex };
}

function runWrapper(install, env) {
  const result = spawnSync(process.execPath, [install.launcherPath, "--probe"], {
    encoding: "utf8",
    env,
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

test("prodex npm package launches only Prodex and preserves external Codex discovery", async (t) => {
  const install = await stageWrapperInstall("0.0.0-test");
  if (!install) {
    t.skip("wrapper fixture is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));

  const output = runWrapper(
    install,
    cleanEnv({
      PATH: `${install.externalDir}${path.delimiter}${process.env.PATH || ""}`,
      PRODEX_CODEX_BIN: undefined,
    }),
  );

  assert.equal(output.codexBin, null);
  assert.equal(output.pathEntries[0], install.externalDir);
  assert.doesNotMatch(output.pathEntries.join(path.delimiter), /prodex-codex/u);
});

test("prodex npm package preserves an explicit official Codex path", async (t) => {
  const install = await stageWrapperInstall("0.0.0-test");
  if (!install) {
    t.skip("wrapper fixture is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));

  const output = runWrapper(
    install,
    cleanEnv({
      PATH: `${install.externalDir}${path.delimiter}${process.env.PATH || ""}`,
      PRODEX_CODEX_BIN: install.externalCodex,
    }),
  );
  assert.equal(output.codexBin, install.externalCodex);
});

test("staging publishes Prodex binaries only", async (t) => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-stage-test-"));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const inputDir = path.join(root, "input");
  const outputDir = path.join(root, "output");
  const spec = platformPackages.find(
    (entry) => entry.os === process.platform && entry.cpu === process.arch,
  );
  if (!spec) {
    t.skip(`unsupported staging platform ${process.platform}/${process.arch}`);
    return;
  }
  await writeExecutable(path.join(inputDir, spec.target, spec.binaryFileName), "prodex\n");
  await writeExecutable(path.join(inputDir, spec.target, "codex"), "codex\n");

  const { packageDirs } = await stagePackages({ inputDir, outputDir, platformSpecs: [spec] });
  const mainDir = packageDirs.find((dir) => path.basename(dir) === "prodex");
  const platformDir = packageDirs.find((dir) => path.basename(dir) === packageSlug(spec.packageName));
  const mainManifest = JSON.parse(await fs.readFile(path.join(mainDir, "package.json"), "utf8"));
  assert.equal(mainManifest.dependencies, undefined);
  assert.equal(mainManifest.optionalDependencies[spec.packageName], mainManifest.version);
  assert.deepEqual(await fs.readdir(path.join(platformDir, "vendor")), [spec.binaryFileName]);
  await assert.rejects(fs.access(path.join(mainDir, "lib")));
});
