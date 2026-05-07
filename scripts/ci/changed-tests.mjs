#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { git, normalizeGitPath } from "./guard-common.mjs";
import { formatCommand, runStep, runStepsSerial } from "./main-internal-test-runner.mjs";
import { repoRoot } from "../npm/common.mjs";

const VERSION_SYNC_PATHS = Object.freeze(["Cargo.toml", "npm", "README.md", "QUICKSTART.md", "scripts/npm"]);

function parseArgs(argv) {
  const args = {
    dryRun: false,
    head: "HEAD",
    includeUntracked: true,
    includeWorktree: true,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--base") {
      index += 1;
      args.base = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--head") {
      index += 1;
      args.head = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--no-untracked") {
      args.includeUntracked = false;
      continue;
    }
    if (value === "--no-worktree") {
      args.includeWorktree = false;
      args.includeUntracked = false;
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
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

function requiredValue(value, name) {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/changed-tests.mjs [--base <rev>] [--head <rev>] [--no-worktree] [--no-untracked] [--dry-run]",
      "",
      "Runs cheap focused checks selected from changed paths.",
      "",
      "Selection:",
      "  - explicit --base..--head when provided",
      "  - upstream merge-base..HEAD when a tracking branch exists",
      "  - origin/main merge-base..HEAD when available",
      "  - staged, unstaged, and untracked files by default",
      "",
      "Examples:",
      "  npm run ci:changed -- --dry-run",
      "  npm run test:changed -- --base origin/main",
    ].join("\n") + "\n",
  );
}

async function tryGit(args) {
  try {
    return await git(args, { cwd: repoRoot });
  } catch {
    return null;
  }
}

async function revExists(rev) {
  return Boolean(await tryGit(["rev-parse", "--verify", "--quiet", rev]));
}

async function mergeBase(left, right) {
  const result = await tryGit(["merge-base", left, right]);
  return result?.stdout.trim() || null;
}

async function diffNameOnly(args) {
  const result = await tryGit(["diff", "--name-only", ...args]);
  return (result?.stdout ?? "")
    .split(/\r?\n/)
    .filter(Boolean)
    .map(normalizeGitPath);
}

async function upstreamMergeBase(head) {
  const upstreamResult = await tryGit(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
  const upstream = upstreamResult?.stdout.trim();
  if (!upstream) {
    return null;
  }
  const base = await mergeBase(upstream, head);
  return base ? { base, label: `upstream ${upstream}` } : null;
}

async function fallbackMergeBase(head) {
  for (const ref of ["origin/main", "main"]) {
    if (!(await revExists(ref))) {
      continue;
    }
    const base = await mergeBase(ref, head);
    if (base) {
      return { base, label: ref };
    }
  }
  return null;
}

async function selectedBase(args) {
  if (args.base) {
    return { base: args.base, label: "explicit base" };
  }
  return (await upstreamMergeBase(args.head)) ?? (await fallbackMergeBase(args.head));
}

async function changedPaths(args) {
  const paths = new Set();
  const base = await selectedBase(args);
  if (base) {
    for (const filePath of await diffNameOnly([base.base, args.head])) {
      paths.add(filePath);
    }
  }

  if (args.includeWorktree) {
    for (const filePath of await diffNameOnly(["--cached"])) {
      paths.add(filePath);
    }
    for (const filePath of await diffNameOnly([])) {
      paths.add(filePath);
    }
  }

  if (args.includeUntracked) {
    const result = await tryGit(["ls-files", "--others", "--exclude-standard"]);
    for (const filePath of (result?.stdout ?? "").split(/\r?\n/).filter(Boolean).map(normalizeGitPath)) {
      paths.add(filePath);
    }
  }

  return { base, paths: [...paths].sort() };
}

async function pathExists(relativePath) {
  try {
    await fs.access(path.join(repoRoot, relativePath));
    return true;
  } catch {
    return false;
  }
}

async function packageNameForCrateDir(crateDir) {
  const manifestPath = path.join(repoRoot, crateDir, "Cargo.toml");
  const contents = await fs.readFile(manifestPath, "utf8");
  const match = contents.match(/^\s*name\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error(`missing package.name in ${crateDir}/Cargo.toml`);
  }
  return match[1];
}

function addStep(steps, key, step) {
  if (!steps.has(key)) {
    steps.set(key, step);
  }
}

function isSmartContextPath(filePath) {
  return (
    filePath === "crates/prodex-app/src/runtime_proxy/smart_context.rs" ||
    filePath.startsWith("crates/prodex-app/src/runtime_proxy/smart_context/") ||
    filePath === "crates/prodex-app/tests/src/runtime_proxy/smart_context.rs"
  );
}

function isRuntimeHotPath(filePath) {
  return (
    filePath.startsWith("crates/prodex-app/src/runtime_proxy/") ||
    filePath === "crates/prodex-app/src/runtime_launch/proxy_startup.rs" ||
    filePath.startsWith("crates/prodex-runtime-proxy/src/")
  );
}

function isVersionManagedPath(filePath) {
  return (
    filePath === "Cargo.toml" ||
    filePath === "README.md" ||
    filePath === "QUICKSTART.md" ||
    filePath.startsWith("npm/") ||
    filePath.startsWith("scripts/npm/")
  );
}

function isNodeScriptPath(filePath) {
  return (
    (filePath.startsWith("scripts/") || filePath.startsWith("tests/")) &&
    (filePath.endsWith(".mjs") || filePath.endsWith(".cjs") || filePath.endsWith(".js"))
  );
}

function crateDirForPath(filePath) {
  const parts = filePath.split("/");
  if (parts[0] === "crates" && parts.length >= 2) {
    return `crates/${parts[1]}`;
  }
  return null;
}

function rootRustTouched(filePath) {
  return filePath === "Cargo.toml" || filePath === "Cargo.lock" || filePath.startsWith("src/");
}

async function addCargoSteps(steps, paths) {
  const crateDirs = new Set();
  let contextTouched = false;

  for (const filePath of paths) {
    if (isSmartContextPath(filePath)) {
      addStep(steps, "cargo:prodex-app:smart-context", {
        label: "cargo:prodex-app:smart-context",
        command: "cargo",
        args: ["test", "-q", "-p", "prodex-app", "smart_context", "--", "--test-threads=1"],
      });
      continue;
    }

    const crateDir = crateDirForPath(filePath);
    if (crateDir) {
      crateDirs.add(crateDir);
      if (crateDir === "crates/prodex-context") {
        contextTouched = true;
      }
      continue;
    }

    if (rootRustTouched(filePath)) {
      addStep(steps, "cargo:prodex-root", {
        label: "cargo:prodex-root",
        command: "cargo",
        args: ["test", "-q", "-p", "prodex", "--lib", "--", "--test-threads=1"],
      });
    }
  }

  for (const crateDir of [...crateDirs].sort()) {
    const packageName = await packageNameForCrateDir(crateDir);
    addStep(steps, `cargo:${packageName}`, {
      label: `cargo:${packageName}`,
      command: "cargo",
      args: ["test", "-q", "-p", packageName],
    });
  }

  if (contextTouched) {
    addStep(steps, "cargo:prodex-app:smart-context", {
      label: "cargo:prodex-app:smart-context",
      command: "cargo",
      args: ["test", "-q", "-p", "prodex-app", "smart_context", "--", "--test-threads=1"],
    });
  }
}

async function buildSteps(paths) {
  const steps = new Map();

  const markdownPaths = [];
  for (const filePath of paths) {
    if (isNodeScriptPath(filePath) && (await pathExists(filePath))) {
      addStep(steps, `node-check:${filePath}`, {
        label: `node-check:${filePath}`,
        command: "node",
        args: ["--check", filePath],
      });
    }

    if (filePath === "package.json") {
      addStep(steps, "package:changed-aliases", {
        label: "package:changed-aliases",
        command: "node",
        args: [
          "-e",
          "const fs=require('node:fs'); const pkg=JSON.parse(fs.readFileSync('package.json','utf8')); for (const name of ['ci:changed','test:changed']) { if (pkg.scripts?.[name] !== 'node scripts/ci/changed-tests.mjs') throw new Error(`${name} script mismatch`); }",
        ],
      });
    }

    if (filePath.endsWith(".md") && (await pathExists(filePath))) {
      markdownPaths.push(filePath);
    }

    if (filePath === "CHANGELOG.md") {
      addStep(steps, "changelog-check", {
        label: "changelog-check",
        command: "node",
        args: ["scripts/npm/changelog.mjs", "--check"],
      });
    }

    if (isVersionManagedPath(filePath)) {
      addStep(steps, "npm-sync-version", {
        label: "npm-sync-version",
        command: "node",
        args: ["scripts/npm/sync-version.mjs", "--root", "npm"],
        noMutationPaths: VERSION_SYNC_PATHS,
      });
      addStep(steps, "npm-sync-docs-version", {
        label: "npm-sync-docs-version",
        command: "node",
        args: ["scripts/npm/sync-docs-version.mjs"],
        noMutationPaths: VERSION_SYNC_PATHS,
      });
    }

    if (isRuntimeHotPath(filePath)) {
      addStep(steps, "runtime-hotpath-guard", {
        label: "runtime-hotpath-guard",
        command: "node",
        args: ["scripts/ci/runtime-hotpath-guard.mjs"],
      });
    }

    if (
      filePath === "scripts/ci/runtime-test-manifest.mjs" ||
      filePath === "scripts/ci/runtime-test-manifest-guard.mjs" ||
      filePath === ".github/workflows/ci.yml"
    ) {
      addStep(steps, "runtime-test-manifest-guard", {
        label: "runtime-test-manifest-guard",
        command: "node",
        args: ["scripts/ci/runtime-test-manifest-guard.mjs"],
      });
    }

    if (
      filePath === "docs/runtime-policy.md" ||
      filePath === "scripts/docs/runtime-policy.mjs" ||
      filePath.startsWith("crates/prodex-runtime-policy/")
    ) {
      addStep(steps, "docs-runtime-policy-check", {
        label: "docs-runtime-policy-check",
        command: "node",
        args: ["scripts/docs/runtime-policy.mjs", "--check"],
      });
    }
  }

  if (markdownPaths.length > 0) {
    addStep(steps, "docs-lint", {
      label: "docs-lint",
      command: "node",
      args: ["scripts/docs/lint-markdown.mjs", ...markdownPaths.sort()],
    });
  }

  await addCargoSteps(steps, paths);
  return [...steps.values()];
}

async function diffSnapshot(paths) {
  const result = await git(["diff", "--no-ext-diff", "--binary", "HEAD", "--", ...paths], { cwd: repoRoot });
  return result.stdout;
}

async function runStepNoMutation(step) {
  const before = await diffSnapshot(step.noMutationPaths);
  await runStep(step);
  const after = await diffSnapshot(step.noMutationPaths);
  if (before !== after) {
    throw new Error(`${step.label} changed generated metadata; run ${formatCommand(step.command, step.args)} and review the diff`);
  }
}

async function runSelectedSteps(steps, dryRun) {
  if (dryRun) {
    await runStepsSerial(steps, { dryRun: true });
    return;
  }
  for (const step of steps) {
    if (step.noMutationPaths) {
      await runStepNoMutation(step);
      continue;
    }
    await runStep(step);
  }
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const selection = await changedPaths(args);
  const selector = selection.base ? `${selection.base.label} ${selection.base.base}..${args.head}` : "worktree only";
  process.stdout.write(`changed-tests: selector ${selector}${args.includeWorktree ? " + worktree" : ""}\n`);

  if (selection.paths.length === 0) {
    process.stdout.write("changed-tests: no changed paths; nothing to run\n");
    return;
  }

  process.stdout.write(`changed-tests: ${selection.paths.length} changed path(s)\n`);
  for (const filePath of selection.paths) {
    process.stdout.write(`  ${filePath}\n`);
  }

  const steps = await buildSteps(selection.paths);
  if (steps.length === 0) {
    process.stdout.write("changed-tests: no focused local checks mapped for these paths\n");
    return;
  }

  await runSelectedSteps(steps, args.dryRun);
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`changed-tests: ${message}\n`);
  process.exitCode = 1;
}
