#!/usr/bin/env node
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import {
  copyRepoFile,
  ensureDir,
  mainPackageManifest,
  packageSlug,
  platformPackages,
  platformPackageManifest,
  readCargoVersion,
  repoRoot,
  writeJsonFile,
} from "../npm/common.mjs";

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

  if (!args.help && !args.binaryDir) {
    throw new Error("--binary-dir is required");
  }

  return args;
}

function packageInstallDir(root, packageName) {
  return path.join(root, "node_modules", ...packageName.split("/"));
}

async function stageMainPackage(version, smokeRoot) {
  const packageDir = packageInstallDir(smokeRoot, "@christiandoxa/prodex");
  await ensureDir(path.join(packageDir, "lib"));
  await copyRepoFile("README.md", path.join(packageDir, "README.md"));
  await copyRepoFile("LICENSE", path.join(packageDir, "LICENSE"));
  await copyRepoFile("npm/prodex/prodex", path.join(packageDir, "prodex"));
  await copyRepoFile("npm/prodex/lib/codex-shim.cjs", path.join(packageDir, "lib", "codex-shim.cjs"));
  await fs.chmod(path.join(packageDir, "prodex"), 0o755);
  await fs.chmod(path.join(packageDir, "lib", "codex-shim.cjs"), 0o755);
  await writeJsonFile(path.join(packageDir, "package.json"), mainPackageManifest(version));
  return packageDir;
}

async function stagePlatformPackage(version, smokeRoot, spec, binaryDir) {
  const packageDir = packageInstallDir(smokeRoot, spec.packageName);
  const nativeBinary = path.join(binaryDir, spec.binaryFileName);
  const exists = await fs.access(nativeBinary).then(() => true).catch(() => false);
  if (!exists) {
    throw new Error(`missing native binary for smoke test: ${nativeBinary}`);
  }

  await ensureDir(path.join(packageDir, "vendor"));
  await copyRepoFile("LICENSE", path.join(packageDir, "LICENSE"));
  await fs.copyFile(nativeBinary, path.join(packageDir, "vendor", spec.binaryFileName));
  await fs.chmod(path.join(packageDir, "vendor", spec.binaryFileName), 0o755);
  await writeJsonFile(path.join(packageDir, "package.json"), platformPackageManifest(spec, version));
  return packageDir;
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/ci/npm-package-smoke.mjs --binary-dir <release-target-dir>",
        "",
        "Stages a local npm package tree and runs prodex --version against it.",
      ].join("\n") + "\n",
    );
    return;
  }

  const version = await readCargoVersion();
  const spec = platformPackages.find((entry) => entry.os === process.platform && entry.cpu === process.arch);
  if (!spec) {
    throw new Error(`unsupported runner platform for npm smoke: ${process.platform} ${process.arch}`);
  }

  const smokeRoot = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-npm-smoke-"));
  const mainPackageDir = await stageMainPackage(version, smokeRoot);
  await stagePlatformPackage(version, smokeRoot, spec, args.binaryDir);

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

  process.stdout.write(`npm smoke passed for ${packageSlug(spec.packageName)} at ${smokeRoot}\n`);
}

await main();
