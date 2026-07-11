#!/usr/bin/env node
import fs from "node:fs/promises";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { ciImpactCategory } from "./test-impact-manifest.mjs";

export function normalizeChangedPath(filePath) {
  return String(filePath ?? "")
    .trim()
    .replaceAll("\\", "/")
    .replace(/^\.\//, "");
}

function pathCategory(filePath) {
  return ciImpactCategory(filePath);
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

export function forceHeavyForCiEvent({ eventName, ref } = {}) {
  const normalizedEventName = String(eventName ?? "").trim();
  if (normalizedEventName === "schedule" || normalizedEventName === "workflow_dispatch") {
    return {
      heavy: true,
      reason: `${normalizedEventName} requires full CI`,
      paths: [],
      heavyPaths: [],
      lightPaths: [],
      unknownPaths: [],
    };
  }
  void ref;
  return null;
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
    if (value === "--event-name") {
      index += 1;
      args.eventName = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--ref") {
      index += 1;
      args.ref = requiredValue(argv[index], value);
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
      "  --event-name <name> GitHub event name. Scheduled and manual events require full CI.",
      "  --ref <ref>         GitHub ref or ref name for event-specific classification.",
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

  const forcedResult = forceHeavyForCiEvent({ eventName: args.eventName, ref: args.ref });
  const paths = forcedResult
    ? []
    : args.paths.length > 0
      ? args.paths
      : args.base
        ? await gitDiffNameOnly(args.base, args.head)
        : [];
  const result = forcedResult ?? classifyChangedPaths(paths);

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
