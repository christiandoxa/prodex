#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  copyRepoFile,
  ensureDir,
  gatewaySdkPackageManifest,
  mainPackageManifest,
  packageSlug,
  pathExists,
  platformPackageManifest,
  platformPackages,
  readCargoVersion,
  shellQuote,
  writeJsonFile,
} from "./common.mjs";

function parseArgs(argv) {
  const args = { inputDir: null, outputDir: null };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--input-dir") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--input-dir requires a value");
      }
      args.inputDir = path.resolve(argv[index]);
      continue;
    }
    if (value === "--output-dir") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--output-dir requires a value");
      }
      args.outputDir = path.resolve(argv[index]);
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  if (!args.help && (!args.inputDir || !args.outputDir)) {
    throw new Error("--input-dir and --output-dir are required");
  }

  return args;
}

async function stagePlatformPackage(version, inputDir, outputDir, spec) {
  const packageDir = path.join(outputDir, "packages", packageSlug(spec.packageName));
  const artifactBinary = path.join(inputDir, spec.target, spec.binaryFileName);
  const binaryExists = await pathExists(artifactBinary);
  if (!binaryExists) {
    throw new Error(
      `missing staged binary for ${spec.target}; expected ${artifactBinary}`,
    );
  }

  await ensureDir(path.join(packageDir, "vendor"));
  await fs.copyFile(artifactBinary, path.join(packageDir, "vendor", spec.binaryFileName));
  await fs.chmod(path.join(packageDir, "vendor", spec.binaryFileName), 0o755);
  const codexBinaryName = spec.target.endsWith("-msvc") ? "codex.exe" : "codex";
  const artifactCodexBinary = path.join(inputDir, spec.target, codexBinaryName);
  if (await pathExists(artifactCodexBinary)) {
    await ensureDir(path.join(packageDir, "vendor"));
    await fs.copyFile(artifactCodexBinary, path.join(packageDir, "vendor", codexBinaryName));
    await fs.chmod(path.join(packageDir, "vendor", codexBinaryName), 0o755);
  }
  await writeJsonFile(path.join(packageDir, "package.json"), platformPackageManifest(spec, version));
  await copyRepoFile("LICENSE", path.join(packageDir, "LICENSE"));

  return packageDir;
}

async function stageMainPackage(version, outputDir) {
  const packageDir = path.join(outputDir, "packages", packageSlug("@christiandoxa/prodex"));
  await ensureDir(path.join(packageDir, "lib"));
  await copyRepoFile("LICENSE", path.join(packageDir, "LICENSE"));
  await copyRepoFile("README.md", path.join(packageDir, "README.md"));
  await copyRepoFile("npm/prodex/prodex", path.join(packageDir, "prodex"));
  await copyRepoFile("npm/prodex/lib/codex-shim.cjs", path.join(packageDir, "lib", "codex-shim.cjs"));
  await fs.chmod(path.join(packageDir, "prodex"), 0o755);
  await fs.chmod(path.join(packageDir, "lib", "codex-shim.cjs"), 0o755);
  await writeJsonFile(path.join(packageDir, "package.json"), mainPackageManifest(version));
  return packageDir;
}

async function stageGatewaySdkPackage(version, outputDir) {
  const packageDir = path.join(outputDir, "packages", packageSlug("@christiandoxa/prodex-gateway-sdk"));
  await ensureDir(packageDir);
  await copyRepoFile("LICENSE", path.join(packageDir, "LICENSE"));
  await copyRepoFile("npm/prodex-gateway-sdk/README.md", path.join(packageDir, "README.md"));
  await copyRepoFile("npm/prodex-gateway-sdk/index.mjs", path.join(packageDir, "index.mjs"));
  await copyRepoFile("npm/prodex-gateway-sdk/index.d.ts", path.join(packageDir, "index.d.ts"));
  await writeJsonFile(path.join(packageDir, "package.json"), gatewaySdkPackageManifest(version));
  return packageDir;
}

export async function stagePackages({ inputDir, outputDir, platformSpecs = platformPackages }) {
  const version = await readCargoVersion();
  await ensureDir(path.join(outputDir, "packages"));

  const stagedPackages = [];
  for (const spec of platformSpecs) {
    stagedPackages.push(await stagePlatformPackage(version, inputDir, outputDir, spec));
  }
  stagedPackages.push(await stageMainPackage(version, outputDir));
  stagedPackages.push(await stageGatewaySdkPackage(version, outputDir));

  await fs.writeFile(
    path.join(outputDir, "packages.json"),
    `${JSON.stringify(
      {
        version,
        packages: stagedPackages.map((dir) => path.basename(dir)),
      },
      null,
      2,
    )}\n`,
  );

  return { version, packageDirs: stagedPackages };
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/npm/stage.mjs --input-dir <artifact-dir> --output-dir <staging-dir>",
        "",
        "Stages prodex npm packages into a publishable directory tree.",
      ].join("\n") + "\n",
    );
    return;
  }

  const { packageDirs } = await stagePackages({
    inputDir: args.inputDir,
    outputDir: args.outputDir,
  });

  process.stdout.write(
    `staged ${packageDirs.length} package(s) at ${shellQuote(path.join(args.outputDir, "packages"))}\n`,
  );
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}
