#!/usr/bin/env node
import { git } from "./guard-common.mjs";
import { runStepsSerial } from "./main-internal-test-runner.mjs";
import { repoRoot } from "../npm/common.mjs";

function parseArgs(argv) {
  const args = {
    dryRun: false,
    head: "HEAD",
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--base") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.base = argv[index];
      continue;
    }
    if (value === "--head") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.head = argv[index];
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

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/prepush.mjs [--base <rev>] [--head <rev>] [--dry-run]",
      "",
      "Runs cheap local checks that mirror CI's push-facing guard.",
      "",
      "Checks:",
      "  - changelog freshness",
      "  - churn hygiene over upstream merge-base..HEAD when available",
      "  - Rust source file size guard",
      "  - cargo fmt --check",
      "  - npm/docs version sync idempotence",
      "",
      "Options:",
      "  --base <rev>  explicit churn-hygiene base",
      "  --head <rev>  explicit churn-hygiene head; default HEAD",
      "  --dry-run     print the command plan without running it",
    ].join("\n") + "\n",
  );
}

async function upstreamMergeBase(head) {
  try {
    const { stdout: upstreamStdout } = await git(
      ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
      { cwd: repoRoot },
    );
    const upstream = upstreamStdout.trim();
    if (!upstream) {
      return null;
    }
    const { stdout: baseStdout } = await git(["merge-base", upstream, head], { cwd: repoRoot });
    return baseStdout.trim() || null;
  } catch {
    return null;
  }
}

async function churnStep(args) {
  const explicitBase = args.base;
  const base = explicitBase ?? (await upstreamMergeBase(args.head));
  const churnArgs = ["scripts/ci/churn-hygiene.mjs", "--check"];
  if (base) {
    churnArgs.push("--base", base, "--head", args.head);
  }
  return {
    label: base ? "churn-hygiene-push-range" : "churn-hygiene-default-range",
    command: "node",
    args: churnArgs,
  };
}

async function prepushSteps(args) {
  return [
    {
      label: "changelog-check",
      command: "node",
      args: ["scripts/npm/changelog.mjs", "--check"],
    },
    await churnStep(args),
    {
      label: "size-guard",
      command: "node",
      args: ["scripts/ci/size-guard.mjs"],
    },
    {
      label: "fmt-check",
      command: "cargo",
      args: ["fmt", "--check"],
    },
    {
      label: "npm-sync-version",
      command: "node",
      args: ["scripts/npm/sync-version.mjs", "--root", "npm"],
    },
    {
      label: "npm-sync-docs-version",
      command: "node",
      args: ["scripts/npm/sync-docs-version.mjs"],
    },
    {
      label: "version-sync-idempotence",
      command: "git",
      args: [
        "diff",
        "--exit-code",
        "--",
        "Cargo.toml",
        "npm",
        "README.md",
        "QUICKSTART.md",
        "scripts/npm",
      ],
    },
  ];
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  await runStepsSerial(await prepushSteps(args), { dryRun: args.dryRun });
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`prepush: ${message}\n`);
  process.exitCode = 1;
}
