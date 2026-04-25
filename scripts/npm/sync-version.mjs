#!/usr/bin/env node
import path from "node:path";
import fs from "node:fs/promises";
import {
  mainPackageName,
  openaiCodexDependencySpecifier,
  platformPackages,
  readCargoVersion,
  readJsonFile,
  writeJsonFile,
} from "./common.mjs";

function parseArgs(argv) {
  const args = { root: process.cwd() };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--root") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--root requires a value");
      }
      args.root = path.resolve(argv[index]);
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

async function updatePackageJson(filePath, version) {
  const packageJson = await readJsonFile(filePath);
  if (typeof packageJson.name !== "string") {
    return false;
  }

  const isProdexPackage =
    packageJson.name === mainPackageName ||
    platformPackages.some((spec) => spec.packageName === packageJson.name);
  if (!isProdexPackage) {
    return false;
  }

  let changed = false;
  if (packageJson.version !== version) {
    packageJson.version = version;
    changed = true;
  }

  if (packageJson.name === mainPackageName && packageJson.optionalDependencies) {
    for (const spec of platformPackages) {
      if (packageJson.optionalDependencies[spec.packageName] !== version) {
        packageJson.optionalDependencies[spec.packageName] = version;
        changed = true;
      }
    }
  }

  if (packageJson.name === mainPackageName) {
    packageJson.dependencies ??= {};
    if (packageJson.dependencies["@openai/codex"] !== openaiCodexDependencySpecifier) {
      packageJson.dependencies["@openai/codex"] = openaiCodexDependencySpecifier;
      changed = true;
    }
  }

  if (changed) {
    await writeJsonFile(filePath, packageJson);
  }

  return changed;
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/npm/sync-version.mjs --root <staging-dir>",
        "",
        "Updates prodex npm package.json files in a staging tree so they match Cargo.toml.",
      ].join("\n") + "\n",
    );
    return;
  }

  const version = await readCargoVersion();
  const stack = [args.root];
  let changedCount = 0;

  while (stack.length > 0) {
    const current = stack.pop();
    const entries = await fs.readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      const candidate = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(candidate);
        continue;
      }
      if (entry.isFile() && entry.name === "package.json") {
        if (await updatePackageJson(candidate, version)) {
          changedCount += 1;
        }
      }
    }
  }

  process.stdout.write(`synced ${changedCount} package.json file(s) to ${version}\n`);
}

await main();
