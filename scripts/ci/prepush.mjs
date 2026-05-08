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
      "  - release hygiene guards over upstream merge-base..HEAD by default",
      "  - release guard historical fixtures",
      "  - release changelog freshness",
      "  - upstream compatibility baseline guard",
      "  - churn hygiene over upstream merge-base..HEAD when available",
      "  - Rust source file size guard",
      "  - cargo fmt --check",
      "  - npm/docs version sync idempotence",
      "",
      "Options:",
      "  --base <rev>  explicit release/churn hygiene base",
      "  --head <rev>  explicit release/churn hygiene head; default HEAD",
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

async function prepushRange(args) {
  const explicitBase = args.base;
  const base = explicitBase ?? (await upstreamMergeBase(args.head));
  return base ? { base, head: args.head } : null;
}

function withOptionalRange(script, range) {
  const guardArgs = [script];
  if (range) {
    guardArgs.push("--base", range.base, "--head", range.head);
  }
  return guardArgs;
}

function releaseHygieneSteps(range) {
  return [
    {
      label: range ? "release-hygiene-push-range" : "release-hygiene-default-range",
      command: "node",
      args: withOptionalRange("scripts/ci/release-hygiene.mjs", range),
    },
  ];
}

function churnStep(range) {
  const churnArgs = ["scripts/ci/churn-hygiene.mjs", "--check"];
  if (range) {
    churnArgs.push("--base", range.base, "--head", range.head);
  }
  return {
    label: range ? "churn-hygiene-push-range" : "churn-hygiene-default-range",
    command: "node",
    args: churnArgs,
  };
}

async function prepushSteps(args) {
  const range = await prepushRange(args);
  return [
    {
      label: "changelog-ci-check",
      command: "node",
      args: ["scripts/npm/changelog.mjs", "--ci-check"],
    },
    {
      label: "upstream-baseline-guard",
      command: "node",
      args: ["scripts/compat/check-upstream-baseline.mjs"],
    },
    ...releaseHygieneSteps(range),
    churnStep(range),
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
