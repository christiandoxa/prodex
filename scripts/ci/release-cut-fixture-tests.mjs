#!/usr/bin/env node
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  mainPackageName,
  openaiCodexPlatformDependencySpecifier,
  openaiCodexPlatformPackages,
  platformPackages,
  packageSlug,
} from "../npm/common.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");
const releaseCutPath = path.join(repoRoot, "scripts/npm/release-cut.mjs");

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd ?? repoRoot,
      env: options.env ?? process.env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      resolve({
        code: signal ? null : code,
        command: [command, ...args].join(" "),
        signal,
        stdout,
        stderr,
      });
    });
  });
}

async function git(fixtureRoot, args) {
  const result = await run("git", args, { cwd: fixtureRoot });
  if (result.signal || result.code !== 0) {
    throw new Error(
      [
        `git ${args.join(" ")} failed`,
        result.stdout.trim() ? `stdout: ${result.stdout.trim()}` : null,
        result.stderr.trim() ? `stderr: ${result.stderr.trim()}` : null,
      ]
        .filter(Boolean)
        .join("\n"),
    );
  }
  return result.stdout.trimEnd();
}

async function writeFile(fixtureRoot, relativePath, contents) {
  const filePath = path.join(fixtureRoot, relativePath);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, contents);
}

async function appendFile(fixtureRoot, relativePath, contents) {
  const filePath = path.join(fixtureRoot, relativePath);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.appendFile(filePath, contents);
}

function cargoToml(version) {
  return [
    "[package]",
    'name = "prodex"',
    `version = "${version}"`,
    'edition = "2024"',
    "",
    "[workspace]",
    "members = []",
    'resolver = "3"',
    "",
    "[workspace.package]",
    `version = "${version}"`,
    'edition = "2024"',
    "",
  ].join("\n");
}

function docs(version, label) {
  return [
    `# ${label}`,
    "",
    `The current local version in this repo is \`${version}\`.`,
    "",
  ].join("\n");
}

async function writePackageManifests(fixtureRoot, version) {
  const optionalDependencies = Object.fromEntries(
    [
      ...platformPackages.map((spec) => [spec.packageName, version]),
      ...openaiCodexPlatformPackages.map((spec) => [
        spec.packageName,
        openaiCodexPlatformDependencySpecifier(spec),
      ]),
    ],
  );
  await writeFile(
    fixtureRoot,
    "npm/prodex/package.json",
    `${JSON.stringify(
      {
        name: mainPackageName,
        version,
        dependencies: {
          "@openai/codex": "0.0.0-fixture",
        },
        optionalDependencies,
      },
      null,
      2,
    )}\n`,
  );

  for (const spec of platformPackages) {
    const platformDir = packageSlug(spec.packageName).replace(/^prodex-/, "");
    await writeFile(
      fixtureRoot,
      `npm/platforms/${platformDir}/package.json`,
      `${JSON.stringify({ name: spec.packageName, version }, null, 2)}\n`,
    );
  }
}

async function setupFixtureRepo() {
  const fixtureRoot = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-release-cut-"));
  await git(fixtureRoot, ["init", "-q"]);
  await git(fixtureRoot, ["config", "user.name", "Prodex Fixture"]);
  await git(fixtureRoot, ["config", "user.email", "fixtures@example.invalid"]);
  await writeFile(fixtureRoot, "Cargo.toml", cargoToml("0.1.0"));
  await writeFile(fixtureRoot, "src/lib.rs", "pub fn fixture() -> &'static str { \"fixture\" }\n");
  await writeFile(
    fixtureRoot,
    "CHANGELOG.md",
    ["# Changelog", "", "Generated from fixture history.", "", "## 0.1.0 - 2026-01-01", "", "- Initial fixture.", ""].join("\n"),
  );
  await writeFile(fixtureRoot, "README.md", docs("0.1.0", "Fixture README"));
  await writeFile(fixtureRoot, "QUICKSTART.md", docs("0.1.0", "Fixture Quickstart"));
  await writePackageManifests(fixtureRoot, "0.1.0");
  await git(fixtureRoot, ["add", "."]);
  await git(fixtureRoot, ["commit", "-m", "feat: seed release cut fixture"]);
  await git(fixtureRoot, ["tag", "0.1.0"]);
  return fixtureRoot;
}

async function releaseCut(fixtureRoot, version, extraArgs = []) {
  return run(process.execPath, [releaseCutPath, "--version", version, "--no-verify", ...extraArgs], {
    cwd: repoRoot,
    env: {
      ...process.env,
      PRODEX_REPO_ROOT: fixtureRoot,
    },
  });
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertExit(result, expected, label) {
  if (!result.signal && result.code === expected) {
    return;
  }
  throw new Error(
    [
      `${label}: expected exit ${expected}, got ${result.signal ? `signal ${result.signal}` : result.code}`,
      `command: ${result.command}`,
      result.stdout.trim() ? `stdout: ${result.stdout.trim()}` : null,
      result.stderr.trim() ? `stderr: ${result.stderr.trim()}` : null,
    ]
      .filter(Boolean)
      .join("\n"),
  );
}

async function readJson(fixtureRoot, relativePath) {
  return JSON.parse(await fs.readFile(path.join(fixtureRoot, relativePath), "utf8"));
}

async function assertClean(fixtureRoot, label) {
  const status = await git(fixtureRoot, ["status", "--porcelain"]);
  assert(status === "", `${label}: expected clean worktree, found ${JSON.stringify(status)}`);
}

async function assertVersionSynced(fixtureRoot, version) {
  const cargoTomlText = await fs.readFile(path.join(fixtureRoot, "Cargo.toml"), "utf8");
  assert(cargoTomlText.includes(`version = "${version}"`), `Cargo.toml missing ${version}`);

  for (const relativePath of ["README.md", "QUICKSTART.md"]) {
    const contents = await fs.readFile(path.join(fixtureRoot, relativePath), "utf8");
    assert(contents.includes(`\`${version}\``), `${relativePath} local version missing ${version}`);
  }

  const mainManifest = await readJson(fixtureRoot, "npm/prodex/package.json");
  assert(mainManifest.version === version, `npm/prodex/package.json version mismatch: ${mainManifest.version}`);
  assert(
    mainManifest.dependencies?.["@openai/codex"] === "latest",
    "npm/prodex/package.json @openai/codex dependency was not normalized",
  );
  for (const spec of platformPackages) {
    assert(
      mainManifest.optionalDependencies?.[spec.packageName] === version,
      `optional dependency ${spec.packageName} mismatch`,
    );
    const platformDir = packageSlug(spec.packageName).replace(/^prodex-/, "");
    const platformManifest = await readJson(fixtureRoot, `npm/platforms/${platformDir}/package.json`);
    assert(platformManifest.version === version, `${spec.packageName} version mismatch: ${platformManifest.version}`);
  }
  for (const spec of openaiCodexPlatformPackages) {
    assert(
      mainManifest.optionalDependencies?.[spec.packageName] === openaiCodexPlatformDependencySpecifier(spec),
      `optional dependency ${spec.packageName} mismatch`,
    );
  }

  const changelog = await fs.readFile(path.join(fixtureRoot, "CHANGELOG.md"), "utf8");
  assert(new RegExp(`^## ${version} - `, "m").test(changelog), `CHANGELOG.md missing ${version} release heading`);
}

async function testCutsReleaseAndIsIdempotent() {
  const fixtureRoot = await setupFixtureRepo();
  try {
    const first = await releaseCut(fixtureRoot, "0.2.0");
    assertExit(first, 0, "first release cut");
    await assertVersionSynced(fixtureRoot, "0.2.0");
    await assertClean(fixtureRoot, "first release cut");

    const head = await git(fixtureRoot, ["rev-parse", "HEAD"]);
    const subject = await git(fixtureRoot, ["log", "-1", "--format=%s", "HEAD"]);
    const tagTarget = await git(fixtureRoot, ["rev-list", "-n", "1", "0.2.0"]);
    const commitCount = await git(fixtureRoot, ["rev-list", "--count", "HEAD"]);
    const releaseFiles = (await git(fixtureRoot, ["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]))
      .split(/\r?\n/)
      .filter(Boolean);

    assert(subject === "chore(release): release 0.2.0", `unexpected release subject: ${subject}`);
    assert(tagTarget === head, "release tag does not point at HEAD");
    assert(releaseFiles.length > 0, "release cut created an empty commit");
    for (const expectedPath of ["Cargo.toml", "CHANGELOG.md", "README.md", "QUICKSTART.md", "npm/prodex/package.json"]) {
      assert(releaseFiles.includes(expectedPath), `release commit missing ${expectedPath}`);
    }

    const second = await releaseCut(fixtureRoot, "0.2.0");
    assertExit(second, 0, "idempotent release cut");
    assert(second.stdout.includes("already tagged at HEAD"), "idempotent release did not report existing tag at HEAD");
    assert((await git(fixtureRoot, ["rev-parse", "HEAD"])) === head, "idempotent release changed HEAD");
    assert((await git(fixtureRoot, ["rev-list", "--count", "HEAD"])) === commitCount, "idempotent release added a commit");
    await assertClean(fixtureRoot, "idempotent release cut");
  } finally {
    await cleanupFixture(fixtureRoot);
  }
}

async function testRejectsDuplicateTagOnDifferentCommit() {
  const fixtureRoot = await setupFixtureRepo();
  try {
    await git(fixtureRoot, ["tag", "0.2.0"]);
    const before = await git(fixtureRoot, ["rev-parse", "HEAD"]);
    const result = await releaseCut(fixtureRoot, "0.2.0");
    assertExit(result, 1, "duplicate tag release cut");
    assert(result.stderr.includes("tag 0.2.0 already exists"), "duplicate tag error missing expected text");
    assert((await git(fixtureRoot, ["rev-parse", "HEAD"])) === before, "duplicate tag failure changed HEAD");
    await assertClean(fixtureRoot, "duplicate tag release cut");
  } finally {
    await cleanupFixture(fixtureRoot);
  }
}

async function testRejectsDirtyWorktreeBeforeMutation() {
  const fixtureRoot = await setupFixtureRepo();
  try {
    await appendFile(fixtureRoot, "README.md", "\nDirty local edit.\n");
    const result = await releaseCut(fixtureRoot, "0.2.0");
    assertExit(result, 1, "dirty worktree release cut");
    assert(result.stderr.includes("worktree is dirty"), "dirty worktree error missing expected text");

    const cargoTomlText = await fs.readFile(path.join(fixtureRoot, "Cargo.toml"), "utf8");
    assert(cargoTomlText.includes('version = "0.1.0"'), "dirty failure mutated Cargo.toml");
    const tagList = await git(fixtureRoot, ["tag", "--list", "0.2.0"]);
    assert(tagList === "", "dirty failure created release tag");
  } finally {
    await cleanupFixture(fixtureRoot);
  }
}

async function testRejectsAmbiguousExistingVersion() {
  const fixtureRoot = await setupFixtureRepo();
  try {
    await writeFile(fixtureRoot, "Cargo.toml", cargoToml("0.2.0"));
    await git(fixtureRoot, ["add", "Cargo.toml"]);
    await git(fixtureRoot, ["commit", "-m", "chore: manual version bump"]);
    const before = await git(fixtureRoot, ["rev-parse", "HEAD"]);
    const result = await releaseCut(fixtureRoot, "0.2.0");
    assertExit(result, 1, "ambiguous existing version release cut");
    assert(result.stderr.includes("refusing ambiguous release cut"), "ambiguous version error missing expected text");
    assert((await git(fixtureRoot, ["rev-parse", "HEAD"])) === before, "ambiguous failure changed HEAD");
    const tagList = await git(fixtureRoot, ["tag", "--list", "0.2.0"]);
    assert(tagList === "", "ambiguous failure created release tag");
    await assertClean(fixtureRoot, "ambiguous existing version release cut");
  } finally {
    await cleanupFixture(fixtureRoot);
  }
}

async function cleanupFixture(fixtureRoot) {
  if (process.env.PRODEX_KEEP_RELEASE_CUT_FIXTURES === "1") {
    process.stdout.write(`release cut fixture kept at ${fixtureRoot}\n`);
    return;
  }
  await fs.rm(fixtureRoot, { recursive: true, force: true });
}

const tests = [
  ["cuts release and is idempotent", testCutsReleaseAndIsIdempotent],
  ["rejects duplicate tag on different commit", testRejectsDuplicateTagOnDifferentCommit],
  ["rejects dirty worktree before mutation", testRejectsDirtyWorktreeBeforeMutation],
  ["rejects ambiguous existing version", testRejectsAmbiguousExistingVersion],
];

let failures = 0;
for (const [name, test] of tests) {
  try {
    await test();
    process.stdout.write(`ok - ${name}\n`);
  } catch (error) {
    failures += 1;
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`not ok - ${name}\n${message}\n`);
  }
}

if (failures > 0) {
  process.stderr.write(`release cut fixtures: ${failures} failed\n`);
  process.exitCode = 1;
} else {
  process.stdout.write(`release cut fixtures: ${tests.length} passed\n`);
}
