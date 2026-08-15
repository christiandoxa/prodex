#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import { stagePackages } from "../npm/stage.mjs";
import {
  ensureDir,
  gatewaySdkPackageName,
  mainPackageName,
  packageSlug,
  platformPackages,
  repoRoot,
} from "../npm/common.mjs";

async function resolveDefaultBinaryDir(spec) {
  const candidates = [path.join(repoRoot, "target", "release"), path.join(repoRoot, "target", "debug")];
  for (const candidate of candidates) {
    const binaryPath = path.join(candidate, spec.binaryFileName);
    const exists = await fs.access(binaryPath).then(() => true).catch(() => false);
    if (exists) {
      return candidate;
    }
  }
  return path.join(repoRoot, "target", "release");
}

function parseArgs(argv) {
  const args = { binaryDir: null };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--binary-dir") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--binary-dir requires a value");
      }
      args.binaryDir = path.resolve(argv[index]);
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  return args;
}

function packageInstallDir(root, packageName) {
  return path.join(root, "node_modules", ...packageName.split("/"));
}

async function copyStagedPackage(installRoot, packageDir) {
  const manifest = JSON.parse(await fs.readFile(path.join(packageDir, "package.json"), "utf8"));
  const installDir = packageInstallDir(installRoot, manifest.name);
  await ensureDir(path.dirname(installDir));
  await fs.cp(packageDir, installDir, { recursive: true });
  return { dir: installDir, manifest };
}

function runPublishDryRun(stagingDir) {
  const result = spawnSync(
    process.execPath,
    [path.join(repoRoot, "scripts/npm/publish.mjs"), "--root", stagingDir, "--dry-run"],
    { cwd: repoRoot, encoding: "utf8" },
  );
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.stderr.write(result.stdout ?? "");
    process.stderr.write(result.stderr ?? "");
    throw new Error(`npm publish dry-run failed with exit code ${result.status}`);
  }
  return `${result.stdout ?? ""}${result.stderr ?? ""}`;
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/ci/npm-package-smoke.mjs [--binary-dir <target-dir>]",
        "",
        "Stages the host npm packages, runs the repository publish dry-run, and runs prodex --version.",
        "Defaults to target/release and falls back to target/debug when no --binary-dir is provided.",
      ].join("\n") + "\n",
    );
    return;
  }

  const spec = platformPackages.find((entry) => entry.os === process.platform && entry.cpu === process.arch);
  if (!spec) {
    throw new Error(`unsupported runner platform for npm smoke: ${process.platform} ${process.arch}`);
  }
  args.binaryDir ??= await resolveDefaultBinaryDir(spec);

  const smokeRoot = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-npm-smoke-"));
  const artifactDir = path.join(smokeRoot, "artifacts");
  const stagingDir = path.join(smokeRoot, "staging");
  const nativeBinary = path.join(args.binaryDir, spec.binaryFileName);
  const binaryExists = await fs.access(nativeBinary).then(() => true).catch(() => false);
  if (!binaryExists) {
    throw new Error(`missing native binary for smoke test: ${nativeBinary}`);
  }
  await ensureDir(path.join(artifactDir, spec.target));
  await fs.copyFile(nativeBinary, path.join(artifactDir, spec.target, spec.binaryFileName));

  const { version, packageDirs } = await stagePackages({
    inputDir: artifactDir,
    outputDir: stagingDir,
    platformSpecs: [spec],
  });
  const packagesManifest = JSON.parse(
    await fs.readFile(path.join(stagingDir, "packages.json"), "utf8"),
  );
  assert.deepEqual(packagesManifest.packages, [
    packageSlug(spec.packageName),
    packageSlug(mainPackageName),
    packageSlug(gatewaySdkPackageName),
  ]);
  const publishOutput = runPublishDryRun(stagingDir);
  for (const packageName of [spec.packageName, mainPackageName, gatewaySdkPackageName]) {
    assert.ok(publishOutput.includes(packageName), `publish dry-run omitted ${packageName}`);
  }

  const installRoot = path.join(smokeRoot, "install");
  const installedPackages = await Promise.all(
    packageDirs.map((packageDir) => copyStagedPackage(installRoot, packageDir)),
  );
  for (const { manifest } of installedPackages) {
    assert.notEqual(manifest.private, true, `${manifest.name} must be publishable when staged`);
  }

  const mainPackageDir = installedPackages.find(({ manifest }) => manifest.name === mainPackageName).dir;
  const sdkPackageDir = installedPackages.find(({ manifest }) => manifest.name === gatewaySdkPackageName).dir;

  const launcherPath = path.join(mainPackageDir, "prodex");
  const result = spawnSync(process.execPath, [launcherPath, "--version"], {
    cwd: repoRoot,
    encoding: "utf8",
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    process.stderr.write(result.stdout ?? "");
    process.stderr.write(result.stderr ?? "");
    throw new Error(`npm smoke failed with exit code ${result.status}`);
  }

  const combinedOutput = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  if (!combinedOutput.includes(version)) {
    throw new Error(`prodex --version did not report ${version}`);
  }

  const { ProdexGatewayClient } = await import(
    pathToFileURL(path.join(sdkPackageDir, "index.mjs")).href,
  );
  const sdkClient = new ProdexGatewayClient({
    fetch: async () =>
      new Response(JSON.stringify({ object: "gateway.providers", providers: [] }), {
        headers: { "content-type": "application/json" },
      }),
  });
  const providers = await sdkClient.providers();
  assert.equal(providers.object, "gateway.providers");

  process.stdout.write(
    `npm smoke passed for ${packageSlug(spec.packageName)}, ${packageSlug(gatewaySdkPackageName)}@${version} at ${smokeRoot}\n`,
  );
}

await main();
