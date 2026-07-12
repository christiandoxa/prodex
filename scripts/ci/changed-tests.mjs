#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { git, normalizeGitPath } from "./guard-common.mjs";
import { formatCommand, runStep, runStepsSerial } from "./main-internal-test-runner.mjs";
import {
  CHANGELOG_TEST_PATH,
  CAPTURE_REPLAY_FIXTURE_TESTS_PATH,
  PACKAGE_SCRIPT_ALIASES,
  PATH_GROUP_NAMES,
  RELEASE_RUN_TEST_PATH,
  UPSTREAM_COMPAT_SCRIPT_PATHS,
  VERSION_SYNC_PATHS,
  WATCH_UPSTREAM_FIXTURE_TESTS_PATH,
  pathMatchesGroup,
} from "./test-impact-manifest.mjs";
import { workspacePackageNameForCrateDir } from "./workspace-metadata.mjs";
import { repoRoot } from "../npm/common.mjs";

const ENTERPRISE_ID_BOUNDARY_CRATES = new Set([
  "prodex-application",
  "prodex-authn",
  "prodex-authz",
  "prodex-config",
  "prodex-control-plane",
  "prodex-domain",
  "prodex-gateway-core",
  "prodex-gateway-http",
  "prodex-observability",
  "prodex-provider-core",
  "prodex-provider-spi",
  "prodex-storage",
  "prodex-storage-postgres",
  "prodex-storage-redis",
  "prodex-storage-sqlite",
]);

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
  return await workspacePackageNameForCrateDir(crateDir);
}

function addStep(steps, key, step) {
  if (!steps.has(key)) {
    steps.set(key, step);
  }
}

async function addNodeCheckStep(steps, filePath) {
  if (!(await pathExists(filePath))) {
    return;
  }
  addStep(steps, `node-check:${filePath}`, {
    label: `node-check:${filePath}`,
    command: "node",
    args: ["--check", filePath],
  });
}

async function addUpstreamCompatSteps(steps) {
  for (const scriptPath of UPSTREAM_COMPAT_SCRIPT_PATHS) {
    await addNodeCheckStep(steps, scriptPath);
  }
  addStep(steps, "upstream-baseline-guard", {
    label: "upstream-baseline-guard",
    command: "node",
    args: ["scripts/compat/check-upstream-baseline.mjs"],
  });
  if (await pathExists(CAPTURE_REPLAY_FIXTURE_TESTS_PATH)) {
    addStep(steps, "capture-replay-fixtures", {
      label: "capture-replay-fixtures",
      command: "node",
      args: [CAPTURE_REPLAY_FIXTURE_TESTS_PATH],
    });
  }
  if (await pathExists(WATCH_UPSTREAM_FIXTURE_TESTS_PATH)) {
    addStep(steps, "watch-upstream-fixtures", {
      label: "watch-upstream-fixtures",
      command: "node",
      args: [WATCH_UPSTREAM_FIXTURE_TESTS_PATH],
    });
  }
}

function isSmartContextPath(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.smartContext);
}

function isRuntimeHotPath(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.runtimeHotPath);
}

function isVersionManagedPath(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.versionManaged);
}

function isNodeScriptPath(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.nodeScript);
}

function isSizeGuardRelevantPath(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.sizeGuardRelevant);
}

function isEnterpriseIdBoundaryRelevantPath(filePath) {
  if (
    filePath === "package.json" ||
    filePath === "scripts/ci/changed-tests.mjs" ||
    filePath === "scripts/ci/enterprise-id-boundary-guard.mjs" ||
    filePath === "scripts/ci/preflight.mjs" ||
    filePath === "scripts/ci/test-impact-manifest.json" ||
    filePath === "docs/enterprise-readiness-audit.md"
  ) {
    return true;
  }

  const parts = filePath.split("/");
  return (
    parts[0] === "crates" &&
    ENTERPRISE_ID_BOUNDARY_CRATES.has(parts[1]) &&
    parts[2] === "src" &&
    filePath.endsWith(".rs")
  );
}

function isEnterpriseDocsGuardRelevantPath(filePath) {
  return (
    filePath === "package.json" ||
    filePath === ".github/workflows/ci.yml" ||
    filePath === "scripts/ci/changed-tests.mjs" ||
    filePath === "scripts/ci/enterprise-docs-guard.mjs" ||
    filePath === "scripts/ci/backup-restore-drill.mjs" ||
    filePath === "scripts/ci/backup-restore-drill.test.mjs" ||
    filePath === "scripts/ci/storage-postgres-proof.mjs" ||
    filePath === "scripts/ci/test-impact-manifest.json" ||
    filePath === "docs/enterprise-readiness-audit.md" ||
    filePath === "docs/testing.md" ||
    filePath === "docs/threat-model.md" ||
    filePath === "docs/migration-guide.md"
  );
}

function isReleaseGuardFixturesRelevantPath(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.releaseGuardRelevant);
}

function isUpstreamCompatRelevantPath(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.upstreamCompatRelevant);
}

function isChurnHygieneRelevantPath(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.churnHygieneRelevant);
}

function isGatewaySecurityRelevantPath(filePath) {
  return (
    filePath === "package.json" ||
    filePath === "scripts/ci/changed-tests.mjs" ||
    filePath === "scripts/ci/test-impact-manifest.json" ||
    filePath.startsWith("crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_") ||
    filePath.startsWith("crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_")
  );
}

function isReleaseRunRelevantPath(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.releaseRunRelevant);
}

function isChangelogRelevantPath(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.changelogRelevant);
}

function crateDirForPath(filePath) {
  const parts = filePath.split("/");
  if (parts[0] === "crates" && parts.length >= 2) {
    return `crates/${parts[1]}`;
  }
  return null;
}

function rootRustTouched(filePath) {
  return pathMatchesGroup(filePath, PATH_GROUP_NAMES.rootRust);
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
        args: ["check", "-q", "-p", "prodex", "--all-targets", "--all-features"],
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

export async function buildSteps(paths) {
  const steps = new Map();

  const markdownPaths = [];
  for (const filePath of paths) {
    if (isNodeScriptPath(filePath)) {
      await addNodeCheckStep(steps, filePath);
    }

    if (filePath === "package.json") {
      addStep(steps, "package:changed-aliases", {
        label: "package:changed-aliases",
        command: "node",
        args: [
          "-e",
          "const fs=require('node:fs'); const pkg=JSON.parse(fs.readFileSync('package.json','utf8')); const expected=JSON.parse(process.argv[1]); for (const [name, command] of Object.entries(expected)) { if (pkg.scripts?.[name] !== command) throw new Error(`${name} script mismatch`); }",
          JSON.stringify(PACKAGE_SCRIPT_ALIASES),
        ],
      });
    }

    if (isSizeGuardRelevantPath(filePath)) {
      addStep(steps, "size-guard", {
        label: "size-guard",
        command: "node",
        args: ["scripts/ci/size-guard.mjs"],
      });
      addStep(steps, "size-guard-fixtures", {
        label: "size-guard-fixtures",
        command: "node",
        args: ["--test", "scripts/ci/size-guard.test.mjs"],
      });
      addStep(steps, "allow-attribute-guard", {
        label: "allow-attribute-guard",
        command: "node",
        args: ["scripts/ci/allow-attribute-guard.mjs"],
      });
      addStep(steps, "env-mutation-guard", {
        label: "env-mutation-guard",
        command: "node",
        args: ["scripts/ci/env-mutation-guard.mjs"],
      });
    }

    if (isEnterpriseIdBoundaryRelevantPath(filePath)) {
      addStep(steps, "enterprise-id-boundary-guard", {
        label: "enterprise-id-boundary-guard",
        command: "node",
        args: ["scripts/ci/enterprise-id-boundary-guard.mjs"],
      });
    }

    if (isEnterpriseDocsGuardRelevantPath(filePath)) {
      addStep(steps, "enterprise-docs-guard", {
        label: "enterprise-docs-guard",
        command: "node",
        args: ["scripts/ci/enterprise-docs-guard.mjs"],
      });
    }

    if (isGatewaySecurityRelevantPath(filePath)) {
      addStep(steps, "gateway-security-smoke", {
        label: "gateway-security-smoke",
        command: "npm",
        args: ["run", "ci:gateway-security-smoke"],
      });
    }

    if (isReleaseGuardFixturesRelevantPath(filePath)) {
      addStep(steps, "release-guard-fixtures", {
        label: "release-guard-fixtures",
        command: "node",
        args: ["scripts/ci/release-guard-fixture-tests.mjs"],
      });
      addStep(steps, "release-cut-fixtures", {
        label: "release-cut-fixtures",
        command: "node",
        args: ["scripts/ci/release-cut-fixture-tests.mjs"],
      });
      addStep(steps, "release-hygiene-dry-run", {
        label: "release-hygiene-dry-run",
        command: "node",
        args: ["scripts/ci/release-hygiene.mjs", "--dry-run"],
      });
    }

    if (isUpstreamCompatRelevantPath(filePath)) {
      await addUpstreamCompatSteps(steps);
    }

    if (isChurnHygieneRelevantPath(filePath)) {
      addStep(steps, "churn-hygiene-fixtures", {
        label: "churn-hygiene-fixtures",
        command: "node",
        args: ["scripts/ci/churn-hygiene-fixture-tests.mjs"],
      });
    }

    if (isReleaseRunRelevantPath(filePath)) {
      addStep(steps, "release-run-tests", {
        label: "release-run-tests",
        command: "node",
        args: ["--test", RELEASE_RUN_TEST_PATH],
      });
    }

    if (isChangelogRelevantPath(filePath)) {
      addStep(steps, "changelog-tests", {
        label: "changelog-tests",
        command: "node",
        args: ["--test", CHANGELOG_TEST_PATH],
      });
    }

    if (filePath.endsWith(".md") && (await pathExists(filePath))) {
      markdownPaths.push(filePath);
    }

    if (filePath === "CHANGELOG.md") {
      addStep(steps, "changelog-ci-check", {
        label: "changelog-ci-check",
        command: "node",
        args: ["scripts/npm/changelog.mjs", "--ci-check"],
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

    if (pathMatchesGroup(filePath, PATH_GROUP_NAMES.runtimeManifestRelevant)) {
      addStep(steps, "runtime-test-manifest-guard", {
        label: "runtime-test-manifest-guard",
        command: "node",
        args: ["scripts/ci/runtime-test-manifest-guard.mjs"],
      });
    }

    if (pathMatchesGroup(filePath, PATH_GROUP_NAMES.runtimePolicyDocsRelevant)) {
      addStep(steps, "docs-runtime-policy-self-test", {
        label: "docs-runtime-policy-self-test",
        command: "node",
        args: ["scripts/docs/runtime-policy.mjs", "--self-test"],
      });
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

function churnHygieneSteps(selection, args) {
  const steps = [];
  if (selection.base) {
    steps.push({
      label: "churn-hygiene:range",
      command: "node",
      args: [
        "scripts/ci/churn-hygiene.mjs",
        "--check",
        "--base",
        selection.base.base,
        "--head",
        args.head,
      ],
    });
  }
  if (args.includeWorktree) {
    steps.push(
      {
        label: "churn-hygiene:staged",
        command: "node",
        args: ["scripts/ci/churn-hygiene.mjs", "--check", "--staged"],
      },
      {
        label: "churn-hygiene:worktree",
        command: "node",
        args: ["scripts/ci/churn-hygiene.mjs", "--check", "--worktree"],
      },
    );
  }
  return steps;
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

  const steps = [...churnHygieneSteps(selection, args), ...(await buildSteps(selection.paths))];
  if (steps.length === 0) {
    process.stdout.write("changed-tests: no focused local checks mapped for these paths\n");
    return;
  }

  await runSelectedSteps(steps, args.dryRun);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    await main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`changed-tests: ${message}\n`);
    process.exitCode = 1;
  }
}
