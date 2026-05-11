#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import { pathToFileURL } from "node:url";
import {
  cargoTomlPath,
  packageVersionPattern,
  platformPackages,
  readCargoVersion,
  repoRoot,
} from "./common.mjs";
import { RELEASE_COMMIT_PATHS } from "../ci/test-impact-manifest.mjs";

const defaultStateFile = path.join(repoRoot, "target", "release-run", "state.json");
const defaultBranch = "main";
const defaultCiWorkflow = "ci.yml";
const defaultPublishWorkflow = "npm-publish.yml";
const mutatingMetadataSteps = new Set(["bump", "sync"]);
const githubRepoSteps = new Set(["watch-ci", "trigger-publish", "watch-publish"]);

export const releaseSteps = [
  "bump",
  "sync",
  "test",
  "commit",
  "push",
  "watch-ci",
  "trigger-publish",
  "watch-publish",
  "verify",
];

function usage() {
  return [
    "Usage: npm run release:run -- [options]",
    "",
    "Mandatory idempotent release runner. It owns version bump, generated metadata, final changelog rendering, validation, commit, push, CI watch, publish dispatch, and verify.",
    "It never runs npm publish locally; publishing is only triggered through .github/workflows/npm-publish.yml.",
    "Do not manually refresh CHANGELOG.md for release commits; release:run renders it with --release-version and validates it through release:prepare.",
    "",
    "Options:",
    "  --version <x.y.z>          bump Cargo.toml to this version before sync",
    "  --dry-run                  print commands and API actions without mutating or dispatching",
    "  --resume                   skip steps recorded complete in the state file",
    "  --state-file <path>        resume state path (default: target/release-run/state.json)",
    "  --from <step>              start at a step",
    "  --to <step>                stop after a step",
    "  --only <a,b,c>             run only selected steps in canonical order",
    "  --branch <name>            branch/ref for push, CI watch, and workflow dispatch (default: main)",
    "  --remote <name>            git remote for push (default: origin)",
    "  --ci-workflow <file>       workflow to watch after push (default: ci.yml)",
    "  --publish-workflow <file>  workflow to dispatch/watch (default: npm-publish.yml)",
    "  --ci-timeout-minutes <n>   CI watch timeout (default: 90)",
    "  --publish-timeout-minutes <n> publish workflow watch timeout (default: 120)",
    "  --poll-seconds <n>         workflow polling interval (default: 30)",
    "  --gh-retries <n>           gh API retry attempts for timeout-like failures (default: 5)",
    "  --no-cargo-test            pass --no-cargo-test to release:prepare",
    "  --skip-verify-npm          skip npm registry verification",
    "  --skip-verify-github       skip GitHub release verification",
    "",
    "Steps:",
    `  ${releaseSteps.join(", ")}`,
    "",
    "Examples:",
    "  npm run release:run -- --dry-run --version 0.93.0",
    "  npm run release:run -- --resume --from watch-ci",
    "  npm run release:run -- --only trigger-publish,watch-publish,verify --resume",
  ].join("\n");
}

export function parseArgs(argv) {
  const args = {
    branch: defaultBranch,
    remote: "origin",
    ciWorkflow: defaultCiWorkflow,
    publishWorkflow: defaultPublishWorkflow,
    ciTimeoutMinutes: 90,
    publishTimeoutMinutes: 120,
    pollSeconds: 30,
    ghRetries: 5,
    dryRun: false,
    resume: false,
    cargoTest: true,
    skipVerifyNpm: false,
    skipVerifyGithub: false,
    stateFile: defaultStateFile,
    steps: [...releaseSteps],
  };

  let fromStep = null;
  let toStep = null;
  let onlySteps = null;

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--resume") {
      args.resume = true;
      continue;
    }
    if (value === "--no-cargo-test") {
      args.cargoTest = false;
      continue;
    }
    if (value === "--skip-verify-npm") {
      args.skipVerifyNpm = true;
      continue;
    }
    if (value === "--skip-verify-github") {
      args.skipVerifyGithub = true;
      continue;
    }

    const stringOptions = new Map([
      ["--version", "version"],
      ["--state-file", "stateFile"],
      ["--branch", "branch"],
      ["--remote", "remote"],
      ["--ci-workflow", "ciWorkflow"],
      ["--publish-workflow", "publishWorkflow"],
      ["--from", "from"],
      ["--to", "to"],
      ["--only", "only"],
    ]);
    if (stringOptions.has(value)) {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      const key = stringOptions.get(value);
      if (key === "from") {
        fromStep = argv[index];
      } else if (key === "to") {
        toStep = argv[index];
      } else if (key === "only") {
        onlySteps = argv[index].split(",").map((step) => step.trim()).filter(Boolean);
      } else if (key === "stateFile") {
        args.stateFile = path.resolve(argv[index]);
      } else {
        args[key] = argv[index];
      }
      continue;
    }

    const numberOptions = new Map([
      ["--ci-timeout-minutes", "ciTimeoutMinutes"],
      ["--publish-timeout-minutes", "publishTimeoutMinutes"],
      ["--poll-seconds", "pollSeconds"],
      ["--gh-retries", "ghRetries"],
    ]);
    if (numberOptions.has(value)) {
      index += 1;
      const parsed = Number.parseInt(argv[index] ?? "", 10);
      if (!Number.isFinite(parsed) || parsed <= 0) {
        throw new Error(`${value} requires a positive integer`);
      }
      args[numberOptions.get(value)] = parsed;
      continue;
    }

    throw new Error(`unknown argument: ${value}`);
  }

  if (args.version && !packageVersionPattern.test(args.version)) {
    throw new Error(`invalid --version: ${args.version}`);
  }

  args.steps = selectSteps({ fromStep, toStep, onlySteps });
  return args;
}

export function selectSteps({ fromStep = null, toStep = null, onlySteps = null } = {}) {
  const assertStep = (step, label) => {
    if (!releaseSteps.includes(step)) {
      throw new Error(`unknown ${label} step: ${step}`);
    }
  };

  if (onlySteps) {
    for (const step of onlySteps) {
      assertStep(step, "--only");
    }
    return releaseSteps.filter((step) => onlySteps.includes(step));
  }

  let start = 0;
  let end = releaseSteps.length - 1;
  if (fromStep) {
    assertStep(fromStep, "--from");
    start = releaseSteps.indexOf(fromStep);
  }
  if (toStep) {
    assertStep(toStep, "--to");
    end = releaseSteps.indexOf(toStep);
  }
  if (start > end) {
    throw new Error("--from must not come after --to");
  }
  return releaseSteps.slice(start, end + 1);
}

export function releaseSubject(version) {
  return `chore(release): release ${version}`;
}

export function pendingStepsForRun(steps = releaseSteps, { resume = false, completed = {} } = {}) {
  if (!resume) {
    return [...steps];
  }
  return steps.filter((step) => !completed?.[step]);
}

export function shouldRequireCleanWorktree({ steps = releaseSteps, resume = false, completed = {}, dryRun = false } = {}) {
  if (dryRun) {
    return false;
  }
  return pendingStepsForRun(steps, { resume, completed }).some((step) => mutatingMetadataSteps.has(step));
}

export function shouldResolveGithubRepo({
  steps = releaseSteps,
  resume = false,
  completed = {},
  skipVerifyGithub = false,
} = {}) {
  return pendingStepsForRun(steps, { resume, completed }).some(
    (step) => githubRepoSteps.has(step) || (step === "verify" && !skipVerifyGithub),
  );
}

export function isGhTimeoutError(message) {
  return [
    /context deadline exceeded/i,
    /i\/o timeout/i,
    /TLS handshake timeout/i,
    /timeout awaiting response headers/i,
    /client\.timeout/i,
    /operation timed out/i,
    /\btimed out\b/i,
    /\b504\b/,
    /\b502\b/,
    /\b503\b/,
  ].some((pattern) => pattern.test(message));
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function pathExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function readState(stateFile) {
  if (!(await pathExists(stateFile))) {
    return { completed: {}, version: null };
  }
  return JSON.parse(await fs.readFile(stateFile, "utf8"));
}

async function writeState(stateFile, state) {
  await fs.mkdir(path.dirname(stateFile), { recursive: true });
  await fs.writeFile(stateFile, `${JSON.stringify(state, null, 2)}\n`);
}

function shellCommand(command, args) {
  return [command, ...args].join(" ");
}

function runCommand(command, args, { dryRun = false, capture = false, env = {} } = {}) {
  if (dryRun) {
    process.stdout.write(`dry-run: ${shellCommand(command, args)}\n`);
    return Promise.resolve(capture ? "" : undefined);
  }

  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: {
        ...process.env,
        ...env,
      },
      stdio: capture ? ["ignore", "pipe", "pipe"] : "inherit",
    });
    let stdout = "";
    let stderr = "";
    if (capture) {
      child.stdout.on("data", (chunk) => {
        stdout += chunk;
      });
      child.stderr.on("data", (chunk) => {
        stderr += chunk;
      });
    }
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${shellCommand(command, args)} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        const detail = capture && stderr.trim() ? `\n${stderr.trim()}` : "";
        reject(new Error(`${shellCommand(command, args)} exited with code ${code}${detail}`));
        return;
      }
      resolve(capture ? stdout : undefined);
    });
  });
}

async function ghApi(endpoint, { method = "GET", fields = {}, retries = 5, dryRun = false } = {}) {
  const args = ["api", "--method", method, endpoint];
  for (const [key, value] of Object.entries(fields)) {
    args.push("-f", `${key}=${value}`);
  }
  if (dryRun) {
    process.stdout.write(`dry-run: gh ${args.join(" ")}\n`);
    return null;
  }

  for (let attempt = 1; ; attempt += 1) {
    try {
      const stdout = await runCommand("gh", args, { capture: true });
      if (!stdout.trim()) {
        return null;
      }
      return JSON.parse(stdout);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (attempt >= retries || !isGhTimeoutError(message)) {
        throw error;
      }
      const delayMs = Math.min(30_000, 1_000 * 2 ** (attempt - 1));
      process.stderr.write(`gh api timeout-like failure, retrying ${attempt}/${retries} after ${delayMs}ms\n`);
      await sleep(delayMs);
    }
  }
}

function parseGitHubRepo(remoteUrl) {
  const trimmed = remoteUrl.trim();
  const sshMatch = trimmed.match(/^git@github\.com:([^/]+\/[^/.]+)(?:\.git)?$/);
  if (sshMatch) {
    return sshMatch[1];
  }
  const httpsMatch = trimmed.match(/^https:\/\/github\.com\/([^/]+\/[^/.]+)(?:\.git)?$/);
  if (httpsMatch) {
    return httpsMatch[1];
  }
  throw new Error(`cannot infer GitHub repo from remote URL: ${trimmed}`);
}

async function githubRepo(remote) {
  const remoteUrl = await runCommand("git", ["remote", "get-url", remote], { capture: true });
  return parseGitHubRepo(remoteUrl);
}

async function currentHead() {
  return (await runCommand("git", ["rev-parse", "HEAD"], { capture: true })).trim();
}

async function assertCleanWorktree() {
  const status = (await runCommand("git", ["status", "--porcelain"], { capture: true })).trimEnd();
  if (status) {
    throw new Error(`worktree is dirty; commit or stash changes before release-run mutates release metadata\n${status}`);
  }
}

async function bumpVersion(version, dryRun) {
  if (!version) {
    process.stdout.write(`bump: no --version provided; current version ${await readCargoVersion()}\n`);
    return;
  }
  const current = await readCargoVersion();
  if (current === version) {
    process.stdout.write(`bump: Cargo.toml already at ${version}\n`);
    return;
  }

  if (dryRun) {
    process.stdout.write(`dry-run: bump Cargo.toml ${current} -> ${version}\n`);
    return;
  }

  const original = await fs.readFile(cargoTomlPath, "utf8");
  let section = "";
  const lines = original.split(/\r?\n/).map((line) => {
    const trimmed = line.trim();
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
      section = trimmed;
      return line;
    }
    if ((section === "[package]" || section === "[workspace.package]") && /^\s*version\s*=/.test(line)) {
      return line.replace(/version\s*=\s*"[^"]+"/, `version = "${version}"`);
    }
    return line;
  });
  await fs.writeFile(cargoTomlPath, lines.join("\n"));
  process.stdout.write(`bump: Cargo.toml ${current} -> ${version}\n`);
}

async function syncReleaseMetadata(version, args) {
  await runCommand("npm", ["run", "npm:sync-version"], args);
  await runCommand("npm", ["run", "changelog", "--", "--release-version", version], args);
  await runCommand("node", ["scripts/npm/changelog.mjs", "--check", "--release-version", version], args);
  await runCommand("cargo", ["update", "--workspace"], args);
}

async function runReleaseTests(version, args) {
  const releasePrepareArgs = ["run", "release:prepare"];
  if (!args.cargoTest) {
    releasePrepareArgs.push("--", "--no-cargo-test");
  } else {
    releasePrepareArgs.push("--");
  }
  releasePrepareArgs.push("--release-version", version);
  await runCommand("npm", releasePrepareArgs, args);
}

async function commitRelease(version, args) {
  const headSubject = (await runCommand("git", ["log", "-1", "--pretty=%s"], { capture: true })).trim();
  const message = releaseSubject(version);
  if (headSubject === message) {
    process.stdout.write(`commit: HEAD already has release commit ${message}\n`);
    return;
  }

  const pathsToAdd = [];
  for (const relativePath of RELEASE_COMMIT_PATHS) {
    if (await pathExists(path.join(repoRoot, relativePath))) {
      pathsToAdd.push(relativePath);
    }
  }
  const cratesDir = path.join(repoRoot, "crates");
  for (const entry of await fs.readdir(cratesDir, { withFileTypes: true })) {
    const relativePath = `crates/${entry.name}/Cargo.toml`;
    if (entry.isDirectory() && await pathExists(path.join(repoRoot, relativePath))) {
      pathsToAdd.push(relativePath);
    }
  }
  await runCommand("git", ["add", "--", ...pathsToAdd], args);

  const staged = await runCommand("git", ["diff", "--cached", "--name-only"], { capture: true, dryRun: args.dryRun });
  if (!args.dryRun && !staged.trim()) {
    process.stdout.write("commit: no staged release metadata changes\n");
    return;
  }
  await runCommand("git", ["commit", "-m", message], args);
  await runCommand("node", ["scripts/ci/release-hygiene.mjs", "--commit", "HEAD", "--no-fixtures"], args);
  await runCommand("node", ["scripts/npm/changelog.mjs", "--check"], args);
}

async function pushRelease(args) {
  await runCommand("git", ["push", args.remote, `HEAD:${args.branch}`], args);
}

async function findWorkflowRun(repo, workflow, branch, headSha, args) {
  const query = new URLSearchParams({
    branch,
    per_page: "20",
  });
  const data = await ghApi(`/repos/${repo}/actions/workflows/${workflow}/runs?${query.toString()}`, {
    retries: args.ghRetries,
    dryRun: args.dryRun,
  });
  if (!data) {
    return null;
  }
  return data.workflow_runs?.find((run) => run.head_sha === headSha) ?? null;
}

async function waitForWorkflow({ repo, workflow, branch, headSha, timeoutMinutes, args }) {
  if (args.dryRun) {
    process.stdout.write(`dry-run: watch ${workflow} for ${headSha} on ${branch}\n`);
    return;
  }

  const deadline = Date.now() + timeoutMinutes * 60_000;
  let lastStatus = "";
  while (Date.now() < deadline) {
    const run = await findWorkflowRun(repo, workflow, branch, headSha, args);
    if (!run) {
      lastStatus = "not found yet";
    } else {
      lastStatus = `${run.status}/${run.conclusion ?? "pending"} ${run.html_url}`;
      if (run.status === "completed") {
        if (run.conclusion === "success") {
          process.stdout.write(`${workflow}: success ${run.html_url}\n`);
          return;
        }
        throw new Error(`${workflow}: ${run.conclusion} ${run.html_url}`);
      }
    }
    process.stdout.write(`${workflow}: ${lastStatus}; polling again in ${args.pollSeconds}s\n`);
    await sleep(args.pollSeconds * 1_000);
  }
  throw new Error(`${workflow}: timed out after ${timeoutMinutes} minute(s); last status: ${lastStatus}`);
}

async function triggerPublish(repo, version, args) {
  const headSha = await currentHead();
  const existing = await findWorkflowRun(repo, args.publishWorkflow, args.branch, headSha, args);
  if (existing) {
    process.stdout.write(`trigger-publish: existing run for ${headSha}: ${existing.html_url}\n`);
    return;
  }
  await ghApi(`/repos/${repo}/actions/workflows/${args.publishWorkflow}/dispatches`, {
    method: "POST",
    fields: { ref: args.branch },
    retries: args.ghRetries,
    dryRun: args.dryRun,
  });
  const action = args.dryRun ? "would dispatch" : "dispatched";
  process.stdout.write(`trigger-publish: ${action} ${args.publishWorkflow} for ${args.branch} (${version})\n`);
}

async function verifyNpm(version, args) {
  if (args.skipVerifyNpm) {
    process.stdout.write("verify: npm registry skipped\n");
    return;
  }
  const packages = ["@christiandoxa/prodex", ...platformPackages.map((spec) => spec.packageName)];
  for (const packageName of packages) {
    const found = (await runCommand("npm", ["view", `${packageName}@${version}`, "version"], {
      capture: true,
      dryRun: args.dryRun,
    })).trim();
    if (!args.dryRun && found !== version) {
      throw new Error(`npm verify failed for ${packageName}@${version}: found ${found || "<missing>"}`);
    }
  }
}

async function verifyGithubRelease(repo, version, args) {
  if (args.skipVerifyGithub) {
    process.stdout.write("verify: GitHub release skipped\n");
    return;
  }
  const release = await ghApi(`/repos/${repo}/releases/tags/${version}`, {
    retries: args.ghRetries,
    dryRun: args.dryRun,
  });
  if (args.dryRun) {
    return;
  }
  if (release?.tag_name !== version) {
    throw new Error(`GitHub release verify failed for ${version}`);
  }
}

async function runSelectedStep(step, version, repo, args) {
  switch (step) {
    case "bump":
      await bumpVersion(args.version, args.dryRun);
      return;
    case "sync":
      await syncReleaseMetadata(version, args);
      return;
    case "test":
      await runReleaseTests(version, args);
      return;
    case "commit":
      await commitRelease(version, args);
      return;
    case "push":
      await pushRelease(args);
      return;
    case "watch-ci":
      requireGithubRepo(repo, step);
      await waitForWorkflow({
        repo,
        workflow: args.ciWorkflow,
        branch: args.branch,
        headSha: await currentHead(),
        timeoutMinutes: args.ciTimeoutMinutes,
        args,
      });
      return;
    case "trigger-publish":
      requireGithubRepo(repo, step);
      await triggerPublish(repo, version, args);
      return;
    case "watch-publish":
      requireGithubRepo(repo, step);
      await waitForWorkflow({
        repo,
        workflow: args.publishWorkflow,
        branch: args.branch,
        headSha: await currentHead(),
        timeoutMinutes: args.publishTimeoutMinutes,
        args,
      });
      return;
    case "verify":
      await verifyNpm(version, args);
      if (!args.skipVerifyGithub) {
        requireGithubRepo(repo, step);
      }
      await verifyGithubRelease(repo, version, args);
      return;
    default:
      throw new Error(`unhandled step: ${step}`);
  }
}

function requireGithubRepo(repo, step) {
  if (!repo) {
    throw new Error(`${step} requires a GitHub remote`);
  }
}

export async function main(argv = process.argv) {
  const args = parseArgs(argv);
  if (args.help) {
    process.stdout.write(`${usage()}\n`);
    return;
  }

  const state = args.resume ? await readState(args.stateFile) : { completed: {}, version: null };
  if (shouldRequireCleanWorktree({
    steps: args.steps,
    resume: args.resume,
    completed: state.completed,
    dryRun: args.dryRun,
  })) {
    await assertCleanWorktree();
  }
  const repo = shouldResolveGithubRepo({
    steps: args.steps,
    resume: args.resume,
    completed: state.completed,
    skipVerifyGithub: args.skipVerifyGithub,
  })
    ? await githubRepo(args.remote)
    : null;

  for (const step of args.steps) {
    if (args.resume && state.completed?.[step]) {
      process.stdout.write(`${step}: already complete; skipping\n`);
      continue;
    }

    const version = args.version ?? state.version ?? await readCargoVersion();
    process.stdout.write(`release-run: ${step} (${version})\n`);
    await runSelectedStep(step, version, repo, args);
    if (!args.dryRun) {
      state.version = version;
      state.completed[step] = new Date().toISOString();
      await writeState(args.stateFile, state);
    }
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    await main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`release-run: ${message}\n`);
    process.exitCode = 1;
  }
}
