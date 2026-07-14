#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import {
  cargoTomlPath,
  gatewaySdkPackageName,
  mainPackageName,
  openaiCodexDependencySpecifier,
  openaiCodexPlatformDependencySpecifier,
  openaiCodexPlatformPackages,
  packageSlug,
  packageVersionPattern,
  platformPackages,
  readCargoVersion,
  readJsonFile,
  repoRoot,
} from "./common.mjs";
import { cargoPublishOrderFromMetadata, readCargoMetadata } from "./release-run-lib.mjs";
import { DOC_METADATA_PATHS, KNOWN_NPM_LOCKFILE_PATHS } from "../ci/test-impact-manifest.mjs";

const DOC_VERSION_PATTERNS = [
  {
    label: "current local version",
    pattern: /(The current local version in this repo is `)([^`]+)(`)/g,
  },
];

function platformRepoDir(spec) {
  return packageSlug(spec.packageName).replace(/^prodex-/, "");
}

function parseArgs(argv) {
  const args = { changelogMode: "strict", dryRun: false, cargoTest: true, releaseVersion: null };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--no-cargo-test") {
      args.cargoTest = false;
      continue;
    }
    if (value === "--ci-changelog-check") {
      args.changelogMode = "ci";
      continue;
    }
    if (value === "--release-version") {
      index += 1;
      args.releaseVersion = argv[index] ?? null;
      if (!args.releaseVersion || !packageVersionPattern.test(args.releaseVersion)) {
        throw new Error("--release-version expects a semver version");
      }
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

function printHelp() {
  process.stdout.write(
    [
      "Usage: npm run release:prepare -- [--dry-run] [--no-cargo-test] [--release-version <semver>]",
      "",
      "Runs release prep guards without publishing or mutating tracked files.",
      "",
      "Checks:",
      "  - Cargo/npm/docs version sync and available lockfiles",
      "  - generated changelog freshness",
      "  - docs markdown lint",
      "  - upstream Codex compatibility baseline",
      "  - runtime test manifest",
      "  - Cargo workspace publish order",
      "  - cargo fmt plus full cargo test all-target compile",
      "",
      "--no-cargo-test skips test binary compilation and runs cargo check instead.",
      "--ci-changelog-check uses the push-facing changelog gate; release prep should normally use the strict default.",
      "--release-version renders and verifies the current version as a final changelog release section before the release commit exists.",
    ].join("\n") + "\n",
  );
}

async function fileExists(relativePath) {
  try {
    await fs.access(path.join(repoRoot, relativePath));
    return true;
  } catch {
    return false;
  }
}

function runStep(label, command, commandArgs, { dryRun }) {
  if (dryRun) {
    process.stdout.write(`dry-run: ${label}: ${[command, ...commandArgs].join(" ")}\n`);
    return Promise.resolve();
  }

  return new Promise((resolve, reject) => {
    process.stdout.write(`${label}: ${[command, ...commandArgs].join(" ")}\n`);
    const child = spawn(command, commandArgs, {
      cwd: repoRoot,
      env: {
        ...process.env,
        CARGO_TERM_COLOR: process.env.CARGO_TERM_COLOR ?? "always",
        RUST_BACKTRACE: process.env.RUST_BACKTRACE ?? "1",
      },
      stdio: "inherit",
    });

    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${label} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(new Error(`${label} exited with code ${code}`));
        return;
      }
      resolve();
    });
  });
}

function expectEqual(errors, label, actual, expected) {
  if (actual !== expected) {
    errors.push(`${label}: expected ${expected}, found ${actual ?? "<missing>"}`);
  }
}

async function checkPackageManifests(version, errors) {
  const mainManifest = await readJsonFile(path.join(repoRoot, "npm/prodex/package.json"));
  expectEqual(errors, `${mainPackageName} version`, mainManifest.version, version);
  expectEqual(errors, `${mainPackageName} private`, mainManifest.private, true);
  expectEqual(
    errors,
    `${mainPackageName} dependency @openai/codex`,
    mainManifest.dependencies?.["@openai/codex"],
    openaiCodexDependencySpecifier,
  );

  for (const spec of platformPackages) {
    expectEqual(
      errors,
      `${mainPackageName} optional dependency ${spec.packageName}`,
      mainManifest.optionalDependencies?.[spec.packageName],
      version,
    );
  }
  for (const spec of openaiCodexPlatformPackages) {
    expectEqual(
      errors,
      `${mainPackageName} optional dependency ${spec.packageName}`,
      mainManifest.optionalDependencies?.[spec.packageName],
      openaiCodexPlatformDependencySpecifier(spec),
    );
  }

  for (const spec of platformPackages) {
    const relativePath = `npm/platforms/${platformRepoDir(spec)}/package.json`;
    const manifest = await readJsonFile(path.join(repoRoot, relativePath));
    expectEqual(errors, `${relativePath} name`, manifest.name, spec.packageName);
    expectEqual(errors, `${relativePath} version`, manifest.version, version);
    expectEqual(errors, `${relativePath} private`, manifest.private, true);
  }

  const gatewayManifest = await readJsonFile(
    path.join(repoRoot, "npm/prodex-gateway-sdk/package.json"),
  );
  expectEqual(errors, `${gatewaySdkPackageName} version`, gatewayManifest.version, version);
  expectEqual(errors, `${gatewaySdkPackageName} private`, gatewayManifest.private, true);
}

async function checkDocs(version, errors) {
  for (const relativePath of DOC_METADATA_PATHS) {
    const contents = await fs.readFile(path.join(repoRoot, relativePath), "utf8");
    for (const { label, pattern } of DOC_VERSION_PATTERNS) {
      pattern.lastIndex = 0;
      for (const match of contents.matchAll(pattern)) {
        const found = match[2];
        if (found !== version) {
          errors.push(`${relativePath} ${label}: expected ${version}, found ${found}`);
        }
      }
    }
  }
}

async function checkCargoManifest(version, errors) {
  const contents = await fs.readFile(cargoTomlPath, "utf8");
  let section = "";
  let workspaceVersion = null;

  for (const rawLine of contents.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line.startsWith("[") && line.endsWith("]")) {
      section = line;
      continue;
    }
    if (section === "[workspace.package]") {
      const match = line.match(/^version\s*=\s*"([^"]+)"/);
      if (match) {
        workspaceVersion = match[1];
      }
      continue;
    }
  }

  expectEqual(errors, "Cargo.toml workspace.package version", workspaceVersion, version);
}

async function checkCargoLock(version, errors) {
  const relativePath = "Cargo.lock";
  if (!(await fileExists(relativePath))) {
    process.stdout.write("release metadata: Cargo.lock absent, lockfile version check skipped\n");
    return;
  }

  const contents = await fs.readFile(path.join(repoRoot, relativePath), "utf8");
  for (const packageName of ["prodex"]) {
    const packageBlock = contents
      .split(/\n\[\[package\]\]\n/)
      .find((block) => new RegExp(`(?:^|\\n)name = "${packageName}"(?:\\n|$)`).test(block));
    const match = packageBlock?.match(/(?:^|\n)version = "([^"]+)"/);
    if (!match) {
      errors.push(`Cargo.lock ${packageName} package entry missing version`);
      continue;
    }
    expectEqual(errors, `Cargo.lock ${packageName} version`, match[1], version);
  }
}

function packageNameForLockEntry(lockPath, entry) {
  if (entry && typeof entry.name === "string") {
    return entry.name;
  }
  if (lockPath === "npm/prodex" || lockPath === "node_modules/@christiandoxa/prodex") {
    return mainPackageName;
  }
  for (const spec of platformPackages) {
    const slug = packageSlug(spec.packageName);
    if (
      lockPath === `npm/platforms/${platformRepoDir(spec)}` ||
      lockPath === `npm/platforms/${slug}` ||
      lockPath === `node_modules/${spec.packageName}`
    ) {
      return spec.packageName;
    }
  }
  return null;
}

function validateNpmLockEntry(relativePath, packageName, entry, version, errors) {
  if (packageName === mainPackageName || platformPackages.some((spec) => spec.packageName === packageName)) {
    expectEqual(errors, `${relativePath} ${packageName} lock version`, entry.version, version);
  }
}

function validateNpmLock(lock, relativePath, version, errors) {
  if (lock.packages && typeof lock.packages === "object") {
    for (const [lockPath, entry] of Object.entries(lock.packages)) {
      const packageName = packageNameForLockEntry(lockPath, entry);
      if (packageName) {
        validateNpmLockEntry(relativePath, packageName, entry, version, errors);
      }
    }
  }

  if (lock.dependencies && typeof lock.dependencies === "object") {
    for (const [packageName, entry] of Object.entries(lock.dependencies)) {
      validateNpmLockEntry(relativePath, packageName, entry, version, errors);
    }
  }
}

async function checkNpmLockfiles(version, errors) {
  const present = [];
  for (const relativePath of KNOWN_NPM_LOCKFILE_PATHS) {
    if (await fileExists(relativePath)) {
      present.push(relativePath);
    }
  }

  if (present.length === 0) {
    process.stdout.write("release metadata: npm lockfile absent, npm lock check skipped\n");
    return;
  }

  for (const relativePath of present) {
    const lock = JSON.parse(await fs.readFile(path.join(repoRoot, relativePath), "utf8"));
    validateNpmLock(lock, relativePath, version, errors);
  }
}

async function checkReleaseMetadata() {
  const version = await readCargoVersion();
  const errors = [];
  await checkCargoManifest(version, errors);
  await checkPackageManifests(version, errors);
  await checkDocs(version, errors);
  await checkCargoLock(version, errors);
  await checkNpmLockfiles(version, errors);

  if (errors.length > 0) {
    throw new Error(
      [
        `release metadata check failed with ${errors.length} issue(s):`,
        ...errors.map((error) => `  - ${error}`),
        "",
        "Run npm run npm:sync-version after bumping Cargo.toml, and cargo update after dependency metadata changes.",
      ].join("\n"),
    );
  }

  process.stdout.write(`release metadata: ok (${version})\n`);
}

async function checkCargoPublishOrder(args) {
  if (args.dryRun) {
    process.stdout.write("dry-run: cargo-publish-order: cargo metadata --locked --no-deps --format-version 1\n");
    return;
  }

  const order = cargoPublishOrderFromMetadata(await readCargoMetadata());
  const first = order[0]?.name ?? "<none>";
  const last = order.at(-1)?.name ?? "<none>";
  process.stdout.write(`cargo publish order: ok (${order.length} package(s), first ${first}, last ${last})\n`);
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  await checkReleaseMetadata();
  await checkCargoPublishOrder(args);
  const changelogArgs = [
    "scripts/npm/changelog.mjs",
    args.changelogMode === "ci" ? "--ci-check" : "--check",
  ];
  if (args.releaseVersion) {
    changelogArgs.push("--release-version", args.releaseVersion);
  }
  await runStep("changelog", "node", changelogArgs, args);
  await runStep("docs-lint", "npm", ["run", "docs:lint"], args);
  await runStep("upstream-compat", "node", ["scripts/compat/check-upstream-baseline.mjs"], args);
  await runStep("runtime-manifest", "npm", ["run", "ci:runtime-manifest"], args);
  await runStep("cargo-fmt", "cargo", ["fmt", "--check"], args);
  if (args.cargoTest) {
    await runStep("cargo-test-compile:all-targets", "cargo", [
      "test",
      "--locked",
      "--workspace",
      "--all-targets",
      "--all-features",
      "--no-run",
    ], args);
  } else {
    await runStep("cargo-check", "cargo", ["check", "--locked", "--workspace", "--all-targets", "--all-features"], args);
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`release-prepare: ${message}\n`);
  process.exitCode = 1;
}
