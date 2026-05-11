#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  cargoTomlPath,
  packageVersionPattern,
  readCargoVersion,
  repoRoot,
} from "./common.mjs";
import { RELEASE_COMMIT_PATHS } from "../ci/test-impact-manifest.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));

function parseArgs(argv) {
  const args = {
    commit: true,
    dryRun: false,
    tag: true,
    verify: true,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--version") {
      index += 1;
      args.version = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--no-commit") {
      args.commit = false;
      args.tag = false;
      continue;
    }
    if (value === "--no-tag") {
      args.tag = false;
      continue;
    }
    if (value === "--no-verify") {
      args.verify = false;
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  if (!args.help) {
    if (!args.version) {
      throw new Error("--version is required");
    }
    if (!packageVersionPattern.test(args.version)) {
      throw new Error(`invalid version: ${args.version}`);
    }
  }

  return args;
}

function requiredValue(value, name) {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: npm run release:cut -- --version <semver> [--dry-run] [--no-commit] [--no-tag] [--no-verify]",
      "",
      "Cuts a deterministic release in one command. Prefer npm run release:run for normal releases.",
      "",
      "Pipeline:",
      "  - require a clean worktree",
      "  - bump Cargo.toml [package].version",
      "  - sync Cargo/npm/docs metadata",
      "  - refresh Cargo.lock workspace package versions",
      "  - render CHANGELOG.md as a final release section",
      "  - run release metadata guards",
      "  - create chore(release): release <version>",
      "  - tag the release with the plain <version> tag",
      "",
      "Use --dry-run to print the plan without mutating files.",
    ].join("\n") + "\n",
  );
}

function formatCommand(command, args) {
  return [command, ...args].join(" ");
}

function runStep(label, command, args, options = {}) {
  const dryRun = options.dryRun ?? false;
  const displayCommand = options.displayCommand ?? formatCommand(command, args);
  if (dryRun) {
    process.stdout.write(`dry-run: ${label}: ${displayCommand}\n`);
    return Promise.resolve({ stdout: "", stderr: "", code: 0 });
  }

  return new Promise((resolve, reject) => {
    process.stdout.write(`${label}: ${displayCommand}\n`);
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: {
        ...process.env,
        CARGO_TERM_COLOR: process.env.CARGO_TERM_COLOR ?? "always",
        RUST_BACKTRACE: process.env.RUST_BACKTRACE ?? "1",
        ...(options.env ?? {}),
      },
      stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit",
    });

    let stdout = "";
    let stderr = "";
    if (options.capture) {
      child.stdout?.setEncoding("utf8");
      child.stderr?.setEncoding("utf8");
      child.stdout?.on("data", (chunk) => {
        stdout += chunk;
      });
      child.stderr?.on("data", (chunk) => {
        stderr += chunk;
      });
    }

    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${label} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        const suffix = options.capture && stderr.trim() ? `\n${stderr.trim()}` : "";
        reject(new Error(`${label} exited with code ${code}${suffix}`));
        return;
      }
      resolve({ stdout, stderr, code });
    });
  });
}

async function git(args, options = {}) {
  return runStep(`git ${args[0]}`, "git", args, options);
}

function scriptPath(relativePath) {
  return path.join(scriptDir, "..", "..", relativePath);
}

async function runNodeScript(label, relativePath, args = [], options = {}) {
  return runStep(label, process.execPath, [scriptPath(relativePath), ...args], {
    ...options,
    displayCommand: formatCommand("node", [relativePath, ...args]),
  });
}

async function gitOutput(args) {
  const { stdout } = await git(args, { capture: true });
  return stdout.trimEnd();
}

async function assertCleanWorktree() {
  const status = await gitOutput(["status", "--porcelain"]);
  if (status) {
    throw new Error(`worktree is dirty; commit or stash changes before release-cut\n${status}`);
  }
}

async function tagTarget(tag) {
  try {
    return await gitOutput(["rev-list", "-n", "1", tag]);
  } catch {
    return null;
  }
}

async function pathExists(relativePath) {
  try {
    await fs.access(path.join(repoRoot, relativePath));
    return true;
  } catch {
    return false;
  }
}

async function existingReleaseCommitPaths() {
  const paths = [];
  for (const relativePath of RELEASE_COMMIT_PATHS) {
    if (await pathExists(relativePath)) {
      paths.push(relativePath);
    }
  }
  return paths;
}

async function headRev() {
  return gitOutput(["rev-parse", "HEAD"]);
}

async function headSubject() {
  return gitOutput(["log", "-1", "--format=%s", "HEAD"]);
}

async function setCargoPackageVersion(version) {
  const current = await fs.readFile(cargoTomlPath, "utf8");
  const lines = current.split(/\r?\n/);
  let section = "";
  let changed = false;

  for (let index = 0; index < lines.length; index += 1) {
    const trimmed = lines[index].trim();
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
      section = trimmed;
      continue;
    }
    if (section !== "[package]" || !/^\s*version\s*=/.test(lines[index])) {
      continue;
    }
    const updated = lines[index].replace(/version\s*=\s*"[^"]+"/, `version = "${version}"`);
    if (updated !== lines[index]) {
      lines[index] = updated;
      changed = true;
    }
    break;
  }

  if (!changed && (await readCargoVersion()) !== version) {
    throw new Error("failed to update Cargo.toml [package].version");
  }

  if (changed) {
    await fs.writeFile(cargoTomlPath, lines.join("\n"));
  }
}

async function hasStagedDiff() {
  try {
    await git(["diff", "--cached", "--quiet"]);
    return false;
  } catch {
    return true;
  }
}

function releaseSubject(version) {
  return `chore(release): release ${version}`;
}

async function runReleaseMetadataGuards(version) {
  const subject = releaseSubject(version);
  await runNodeScript("release-metadata-only-staged", "scripts/ci/release-metadata-only-guard.mjs", [
    "--staged",
    "--assume-release",
  ]);
  await runNodeScript("version-metadata-release-staged", "scripts/ci/version-metadata-release-guard.mjs", [
    "--staged",
    "--message",
    subject,
  ]);
  await runNodeScript("release-empty-commit-staged", "scripts/ci/release-empty-commit-guard.mjs", [
    "--staged",
    "--message",
    subject,
  ]);
}

async function tagRelease(version, dryRun) {
  const tag = version;
  const existingTagTarget = await tagTarget(tag);
  const head = await headRev();
  if (existingTagTarget) {
    if (existingTagTarget !== head) {
      throw new Error(`tag ${tag} already exists at ${existingTagTarget}, not HEAD ${head}`);
    }
    process.stdout.write(`release tag ${tag}: already at HEAD\n`);
    return;
  }
  await git(["tag", tag], { dryRun });
}

async function alreadyCut(version) {
  if ((await readCargoVersion()) !== version) {
    return false;
  }
  const target = await tagTarget(version);
  if (!target) {
    return false;
  }
  return target === (await headRev());
}

async function releaseCut(args) {
  const version = args.version;

  if (args.dryRun) {
    process.stdout.write(`dry-run: release-cut ${version}\n`);
    process.stdout.write("dry-run: would require clean worktree\n");
    process.stdout.write("dry-run: would bump Cargo/npm/docs metadata and regenerate changelog\n");
    process.stdout.write("dry-run: would run release guards, commit, and tag unless disabled\n");
    return;
  }

  if (await alreadyCut(version)) {
    process.stdout.write(`release-cut: ${version} already tagged at HEAD\n`);
    return;
  }

  await assertCleanWorktree();
  const currentVersion = await readCargoVersion();
  const tag = await tagTarget(version);
  if (tag) {
    throw new Error(`tag ${version} already exists at ${tag}`);
  }

  if (currentVersion !== version) {
    await setCargoPackageVersion(version);
    await runNodeScript("npm-sync-version", "scripts/npm/sync-version.mjs", ["--root", "npm"]);
    await runNodeScript("npm-sync-docs-version", "scripts/npm/sync-docs-version.mjs");
    await runStep("cargo-update-workspace", "cargo", ["update", "-w"]);
    await runNodeScript("changelog", "scripts/npm/changelog.mjs", ["--release-version", version]);
  } else {
    const subject = await headSubject();
    if (subject !== releaseSubject(version)) {
      throw new Error(
        `Cargo.toml already has ${version}, but HEAD is not "${releaseSubject(version)}"; refusing ambiguous release cut`,
      );
    }
  }

  if (args.verify) {
    await runNodeScript("release-prepare", "scripts/npm/release-prepare.mjs", [
      "--no-cargo-test",
      "--release-version",
      version,
    ]);
  }

  await git(["add", "--", ...(await existingReleaseCommitPaths())]);
  if (!(await hasStagedDiff())) {
    if (args.tag) {
      await tagRelease(version, args.dryRun);
    }
    process.stdout.write(`release-cut: no metadata diff for ${version}\n`);
    return;
  }

  if (args.verify) {
    await runReleaseMetadataGuards(version);
  }

  if (args.commit) {
    await git(["commit", "-m", releaseSubject(version)]);
    if (args.verify) {
      await runNodeScript("release-hygiene-head", "scripts/ci/release-hygiene.mjs", [
        "--commit",
        "HEAD",
        "--no-fixtures",
      ]);
      await runNodeScript("changelog-check", "scripts/npm/changelog.mjs", ["--check"]);
    }
  } else {
    process.stdout.write(`release-cut: staged release metadata for ${version}; commit disabled\n`);
    return;
  }

  if (args.tag) {
    await tagRelease(version, args.dryRun);
    if (args.verify) {
      await runNodeScript("release-tag-changelog-guard", "scripts/ci/release-tag-changelog-guard.mjs", [
        "--rev",
        "HEAD",
      ]);
    }
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  await releaseCut(args);
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`release-cut: ${message}\n`);
  process.exitCode = 1;
}
