#!/usr/bin/env node
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { repoRoot } from "../npm/common.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const guardDir = scriptDir;

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
        stdout: stdout.trim(),
        stderr: stderr.trim(),
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
        result.stdout ? `stdout: ${result.stdout}` : null,
        result.stderr ? `stderr: ${result.stderr}` : null,
      ]
        .filter(Boolean)
        .join("\n"),
    );
  }
  return result.stdout;
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

async function commit(fixtureRoot, subject, options = {}) {
  await git(fixtureRoot, ["add", "."]);
  const args = ["commit", "-m", subject];
  if (options.allowEmpty) {
    args.splice(1, 0, "--allow-empty");
  }
  await git(fixtureRoot, args);
  return (await git(fixtureRoot, ["rev-parse", "HEAD"])).trim();
}

async function setupFixtureRepo() {
  const fixtureRoot = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-release-guards-"));
  await git(fixtureRoot, ["init", "-q"]);
  await git(fixtureRoot, ["config", "user.name", "Prodex Fixture"]);
  await git(fixtureRoot, ["config", "user.email", "fixtures@example.invalid"]);
  await writeFile(
    fixtureRoot,
    "Cargo.toml",
    [
      "[package]",
      'name = "prodex-fixture"',
      'version = "0.1.0"',
      'edition = "2024"',
      "",
      "[dependencies]",
      'tokio = "1.52.2"',
      "",
    ].join("\n"),
  );
  await writeFile(
    fixtureRoot,
    "CHANGELOG.md",
    ["# Changelog", "", "## 0.1.0 - 2026-01-01", "", "- Initial fixture.", ""].join("\n"),
  );
  await writeFile(fixtureRoot, "README.md", "Fixture README\n");
  await writeFile(
    fixtureRoot,
    "QUICKSTART.md",
    [
      "Fixture quickstart",
      "",
      "The current local version in this repo is `0.1.0`:",
      "",
      "```bash",
      "npm install -g @christiandoxa/prodex@0.1.0",
      "```",
      "",
    ].join("\n"),
  );
  await writeFile(fixtureRoot, "src/lib.rs", "pub fn fixture() -> &'static str { \"fixture\" }\n");
  await writeFile(
    fixtureRoot,
    "npm/prodex/package.json",
    `${JSON.stringify({ name: "@christiandoxa/prodex", version: "0.1.0" }, null, 2)}\n`,
  );
  await commit(fixtureRoot, "test: seed release guard fixture");
  return fixtureRoot;
}

async function buildFixtures(fixtureRoot) {
  await writeFile(
    fixtureRoot,
    "Cargo.toml",
    [
      "[package]",
      'name = "prodex-fixture"',
      'version = "0.2.0"',
      'edition = "2024"',
      "",
      "[dependencies]",
      'tokio = "1.52.2"',
      "",
    ].join("\n"),
  );
  await appendFile(fixtureRoot, "src/lib.rs", "pub fn mixed_release_change() {}\n");
  const mixedRelease = await commit(fixtureRoot, "chore(release): prepare 0.2.0");

  await writeFile(
    fixtureRoot,
    "Cargo.toml",
    [
      "[package]",
      'name = "prodex-fixture"',
      'version = "0.2.0"',
      'edition = "2024"',
      "",
      "[dependencies]",
      'tokio = "1.52.3"',
      "",
    ].join("\n"),
  );
  const dependencyBump = await commit(fixtureRoot, "chore(deps): bump tokio from 1.52.2 to 1.52.3");

  await appendFile(fixtureRoot, "README.md", "\n## Feature docs\n\nCodex environment setup notes.\n");
  await appendFile(fixtureRoot, "QUICKSTART.md", "\n## Feature docs\n\nCodex environment setup notes.\n");
  const docsFeatureEdit = await commit(fixtureRoot, "docs: document codex environments");

  await writeFile(
    fixtureRoot,
    "QUICKSTART.md",
    [
      "Fixture quickstart",
      "",
      "The current local version in this repo is `0.2.0`:",
      "",
      "```bash",
      "npm install -g @christiandoxa/prodex@0.2.0",
      "```",
      "",
    ].join("\n"),
  );
  const docsVersionEdit = await commit(fixtureRoot, "docs: update install version");

  const emptyRelease = await commit(fixtureRoot, "chore(release): release 0.2.0", { allowEmpty: true });

  await writeFile(
    fixtureRoot,
    "CHANGELOG.md",
    ["# Changelog", "", "## 0.2.0 - Unreleased", "", "- Refresh-only fixture.", ""].join("\n"),
  );
  const changelogOnlyNoise = await commit(fixtureRoot, "docs(changelog): refresh budget test split notes");

  await appendFile(fixtureRoot, "scripts/npm/changelog.mjs", "export const fixture = true;\n");
  const changelogScriptFix = await commit(fixtureRoot, "fix(changelog): preserve release rendering");

  await appendFile(fixtureRoot, "CHANGELOG.md", "\nGenerated output drift.\n");
  await appendFile(fixtureRoot, "scripts/npm/changelog.mjs", "export const filtered = true;\n");
  const changelogGeneratorDrift = await commit(
    fixtureRoot,
    "fix(changelog): omit internal maintenance",
  );

  const duplicateBase = (await git(fixtureRoot, ["rev-parse", "HEAD"])).trim();
  await appendFile(fixtureRoot, "CHANGELOG.md", "\n## 0.3.0 - 2026-01-02\n\n- First release marker.\n");
  const duplicateOne = await commit(fixtureRoot, "chore(release): release 0.3.0");
  await appendFile(fixtureRoot, "README.md", "\nDuplicate release marker.\n");
  const duplicateTwo = await commit(fixtureRoot, "chore(release): release 0.3.0");

  await appendFile(fixtureRoot, "CHANGELOG.md", "\n## 0.4.0 - 2026-01-03\n\n- Tagged release marker.\n");
  const taggedParent = (await git(fixtureRoot, ["rev-parse", "HEAD"])).trim();
  const taggedRelease = await commit(fixtureRoot, "chore(release): release 0.4.0");
  await git(fixtureRoot, ["tag", "0.4.0", taggedRelease]);

  await appendFile(fixtureRoot, "CHANGELOG.md", "\n## 0.4.1 - 2026-01-03\n\n- Bad tagged release marker.\n");
  const badSubjectTaggedRelease = await commit(fixtureRoot, "feat: tag release 0.4.1 incorrectly");
  await git(fixtureRoot, ["tag", "0.4.1", badSubjectTaggedRelease]);

  const normalBase = (await git(fixtureRoot, ["rev-parse", "HEAD"])).trim();
  await appendFile(fixtureRoot, "CHANGELOG.md", "\n## 0.5.0 - Unreleased\n\n- Prepare marker.\n");
  const normalPrepare = await commit(fixtureRoot, "chore(release): prepare 0.5.0");
  await appendFile(fixtureRoot, "CHANGELOG.md", "\nRelease marker.\n");
  const normalRelease = await commit(fixtureRoot, "chore(release): release 0.5.0");

  return [
    {
      name: "mixed release metadata fails metadata-only guard",
      script: "release-metadata-only-guard.mjs",
      args: ["--commit", mixedRelease],
      expectedExit: 1,
    },
    {
      name: "mixed release metadata fails version metadata guard",
      script: "version-metadata-release-guard.mjs",
      args: ["--commit", mixedRelease],
      expectedExit: 1,
    },
    {
      name: "docs feature edits pass version metadata guard",
      script: "version-metadata-release-guard.mjs",
      args: ["--commit", docsFeatureEdit],
      expectedExit: 0,
    },
    {
      name: "docs version snippet edits fail version metadata guard",
      script: "version-metadata-release-guard.mjs",
      args: ["--commit", docsVersionEdit],
      expectedExit: 1,
    },
    {
      name: "cargo dependency bumps pass version metadata guard",
      script: "version-metadata-release-guard.mjs",
      args: ["--commit", dependencyBump],
      expectedExit: 0,
    },
    {
      name: "empty release commit fails empty commit guard",
      script: "release-empty-commit-guard.mjs",
      args: ["--commit", emptyRelease],
      expectedExit: 1,
    },
    {
      name: "changelog-only refresh fails noise guard",
      script: "changelog-noise-guard.mjs",
      args: ["--commit", changelogOnlyNoise],
      expectedExit: 1,
    },
    {
      name: "changelog script fix passes noise guard",
      script: "changelog-noise-guard.mjs",
      args: ["--commit", changelogScriptFix],
      expectedExit: 0,
    },
    {
      name: "changelog generator drift passes version metadata guard",
      script: "version-metadata-release-guard.mjs",
      args: ["--commit", changelogGeneratorDrift],
      expectedExit: 0,
    },
    {
      name: "duplicate release range fails duplicate guard",
      script: "release-duplicate-version-guard.mjs",
      args: ["--range", `${duplicateBase}..${duplicateTwo}`],
      expectedExit: 1,
      dependsOn: [duplicateOne],
    },
    {
      name: "tagged release has changelog section and release subject",
      script: "release-tag-changelog-guard.mjs",
      args: ["--rev", "0.4.0"],
      expectedExit: 0,
    },
    {
      name: "tagged release with non-release subject fails tag guard",
      script: "release-tag-changelog-guard.mjs",
      args: ["--rev", "0.4.1"],
      expectedExit: 1,
    },
    {
      name: "current tagged range has matching changelog state",
      script: "release-tag-changelog-guard.mjs",
      args: ["--range", `${taggedParent}..${taggedRelease}`],
      expectedExit: 0,
    },
    {
      name: "normal prepare and release range passes duplicate guard",
      script: "release-duplicate-version-guard.mjs",
      args: ["--range", `${normalBase}..${normalRelease}`],
      expectedExit: 0,
      dependsOn: [normalPrepare],
    },
  ];
}

async function runGuard(fixtureRoot, { script, args }) {
  const commandArgs = [path.join(guardDir, script), ...args];
  return run(process.execPath, commandArgs, {
    cwd: repoRoot,
    env: {
      ...process.env,
      PRODEX_REPO_ROOT: fixtureRoot,
    },
  });
}

function printFailure(fixture, result) {
  process.stderr.write(
    [
      `not ok - ${fixture.name}`,
      `  expected exit ${fixture.expectedExit}, got ${result.signal ? `signal ${result.signal}` : result.code}`,
      `  command: ${result.command}`,
      result.stdout ? `  stdout: ${result.stdout}` : null,
      result.stderr ? `  stderr: ${result.stderr}` : null,
    ]
      .filter(Boolean)
      .join("\n") + "\n",
  );
}

async function main() {
  const fixtureRoot = await setupFixtureRepo();
  try {
    const fixtures = await buildFixtures(fixtureRoot);
    let failures = 0;
    for (const fixture of fixtures) {
      const result = await runGuard(fixtureRoot, fixture);
      if (!result.signal && result.code === fixture.expectedExit) {
        process.stdout.write(`ok - ${fixture.name}\n`);
        continue;
      }

      failures += 1;
      printFailure(fixture, result);
    }

    if (failures > 0) {
      process.stderr.write(`release guard fixtures: ${failures} failed\n`);
      process.exitCode = 1;
    } else {
      process.stdout.write(`release guard fixtures: ${fixtures.length} passed\n`);
    }
  } finally {
    if (process.env.PRODEX_KEEP_RELEASE_GUARD_FIXTURES !== "1") {
      await fs.rm(fixtureRoot, { recursive: true, force: true });
    } else {
      process.stdout.write(`release guard fixtures kept at ${fixtureRoot}\n`);
    }
  }
}

await main();
