#!/usr/bin/env node
import fs from "node:fs/promises";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const LIGHT_EXACT_PATHS = Object.freeze([
  "README.md",
  "QUICKSTART.md",
  "package.json",
  "package-lock.json",
  "scripts/ci/ci-impact.mjs",
  "scripts/ci/ci-impact.test.mjs",
  "scripts/ci/release-duplicate-version-guard.mjs",
  "scripts/ci/release-empty-commit-guard.mjs",
  "scripts/ci/release-metadata-only-guard.mjs",
  "scripts/ci/release-tag-changelog-guard.mjs",
  "scripts/ci/test-impact-manifest.mjs",
  "scripts/ci/version-metadata-release-guard.mjs",
]);

const LIGHT_PREFIXES = Object.freeze(["docs/", "npm/", "scripts/npm/"]);

const HEAVY_EXACT_PATHS = Object.freeze([
  "Cargo.lock",
  "Cargo.toml",
  "scripts/ci/runtime-env-parallel.mjs",
  "scripts/ci/runtime-hotpath-guard.mjs",
  "scripts/ci/runtime-proxy-bench-thresholds.json",
  "scripts/ci/runtime-proxy-ci-matrix.mjs",
  "scripts/ci/runtime-proxy-shard.mjs",
  "scripts/ci/runtime-stress.mjs",
  "scripts/ci/runtime-test-manifest-guard.mjs",
  "scripts/ci/runtime-test-manifest.mjs",
]);

const HEAVY_PREFIXES = Object.freeze([
  ".cargo/",
  ".github/workflows/",
  "benches/",
  "crates/",
  "src/",
  "tests/",
]);

const HEAVY_PREFIX_PATTERNS = Object.freeze(["rust-toolchain"]);

export function normalizeChangedPath(filePath) {
  return String(filePath ?? "")
    .trim()
    .replaceAll("\\", "/")
    .replace(/^\.\//, "");
}

function pathCategory(filePath) {
  if (HEAVY_EXACT_PATHS.includes(filePath)) {
    return "heavy";
  }
  if (HEAVY_PREFIXES.some((prefix) => filePath.startsWith(prefix))) {
    return "heavy";
  }
  if (HEAVY_PREFIX_PATTERNS.some((prefix) => filePath.startsWith(prefix))) {
    return "heavy";
  }
  if (LIGHT_EXACT_PATHS.includes(filePath)) {
    return "light";
  }
  if (LIGHT_PREFIXES.some((prefix) => filePath.startsWith(prefix))) {
    return "light";
  }
  return "unknown";
}

export function classifyChangedPaths(changedPaths) {
  if (!changedPaths || typeof changedPaths[Symbol.iterator] !== "function") {
    return {
      heavy: true,
      reason: "no changed paths provided",
      paths: [],
      heavyPaths: [],
      lightPaths: [],
      unknownPaths: [],
    };
  }

  const paths = [...new Set([...changedPaths].map(normalizeChangedPath).filter(Boolean))].sort();
  if (paths.length === 0) {
    return {
      heavy: true,
      reason: "empty changed path set",
      paths,
      heavyPaths: [],
      lightPaths: [],
      unknownPaths: [],
    };
  }

  const heavyPaths = [];
  const lightPaths = [];
  const unknownPaths = [];

  for (const filePath of paths) {
    const category = pathCategory(filePath);
    if (category === "heavy") {
      heavyPaths.push(filePath);
    } else if (category === "light") {
      lightPaths.push(filePath);
    } else {
      unknownPaths.push(filePath);
    }
  }

  if (heavyPaths.length > 0) {
    return {
      heavy: true,
      reason: `heavy path matched: ${heavyPaths[0]}`,
      paths,
      heavyPaths,
      lightPaths,
      unknownPaths,
    };
  }

  if (unknownPaths.length > 0) {
    return {
      heavy: true,
      reason: `unknown path matched conservative default: ${unknownPaths[0]}`,
      paths,
      heavyPaths,
      lightPaths,
      unknownPaths,
    };
  }

  return {
    heavy: false,
    reason: "only lightweight docs/npm/release metadata paths changed",
    paths,
    heavyPaths,
    lightPaths,
    unknownPaths,
  };
}

function parseArgs(argv) {
  const args = {
    head: "HEAD",
    json: false,
    githubOutput: false,
    paths: [],
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
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--github-output") {
      args.githubOutput = true;
      continue;
    }
    if (value === "--path") {
      index += 1;
      args.paths.push(requiredValue(argv[index], value));
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
      "Usage: node scripts/ci/ci-impact.mjs [--base <rev>] [--head <rev>] [--json] [--github-output]",
      "",
      "Classifies changed paths as requiring heavy Rust/runtime CI or lightweight checks only.",
      "",
      "Inputs:",
      "  --base <rev>       Git diff base revision. Required unless --path is used.",
      "  --head <rev>       Git diff head revision. Defaults to HEAD.",
      "  --path <path>      Add an explicit changed path. Repeatable, mainly for tests.",
      "  --json             Print a JSON result.",
      "  --github-output    Append heavy and reason outputs to $GITHUB_OUTPUT.",
    ].join("\n") + "\n",
  );
}

async function gitDiffNameOnly(base, head) {
  const result = await runGit(["diff", "--name-only", `${base}..${head}`]);
  return result.split(/\r?\n/).filter(Boolean);
}

async function runGit(args) {
  return await new Promise((resolve, reject) => {
    const child = spawn("git", args, {
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolve(Buffer.concat(stdout).toString("utf8"));
        return;
      }
      reject(new Error(`git ${args.join(" ")} failed: ${Buffer.concat(stderr).toString("utf8").trim()}`));
    });
  });
}

async function appendGithubOutput(result) {
  const outputPath = process.env.GITHUB_OUTPUT;
  if (!outputPath) {
    throw new Error("--github-output requires GITHUB_OUTPUT");
  }
  await fs.appendFile(
    outputPath,
    [`heavy=${result.heavy ? "true" : "false"}`, `reason=${result.reason}`, ""].join("\n"),
    "utf8",
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const paths = args.paths.length > 0 ? args.paths : args.base ? await gitDiffNameOnly(args.base, args.head) : [];
  const result = classifyChangedPaths(paths);

  if (args.githubOutput) {
    await appendGithubOutput(result);
  }

  if (args.json) {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    return;
  }

  process.stdout.write(`heavy=${result.heavy ? "true" : "false"}\nreason=${result.reason}\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    process.stderr.write(`ci-impact: ${error.message}\n`);
    process.exitCode = 1;
  });
}
