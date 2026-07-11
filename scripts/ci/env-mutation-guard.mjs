#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { git } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const ENV_MUTATION_PATTERN = /\b(?:std::env|env)::(?:set_var|remove_var)\s*\(/;

function parseArgs(argv) {
  const args = { json: false };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--json") {
      args.json = true;
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
      "Usage: node scripts/ci/env-mutation-guard.mjs [--json]",
      "",
      "Fails when Rust tests mutate process environment without an explicit restore guard and lock.",
      "This prevents parallel test flakes from shared PRODEX_* or upstream runtime variables.",
    ].join("\n") + "\n",
  );
}

async function rustFiles() {
  const result = await git(["ls-files", "--", "*.rs"], { cwd: repoRoot });
  return result.stdout.split(/\r?\n/).filter(Boolean).sort();
}

function lineNumber(contents, index) {
  return contents.slice(0, index).split(/\r\n|\n|\r/).length;
}

function fileHasSerializedEnvGuard(contents) {
  const hasGuardType = /\bstruct\s+(?:TestEnvVarGuard|EnvGuard)\b/.test(contents);
  const hasRestoreDrop = /\bimpl\s+Drop\s+for\s+(?:TestEnvVarGuard|EnvGuard)\b/.test(contents);
  const hasLock =
    /\bTEST_ENV_LOCK\b/.test(contents) ||
    /\benv_lock\s*\(\s*\)/.test(contents) ||
    /\bacquire_test_env_lock\b/.test(contents);
  return hasGuardType && hasRestoreDrop && hasLock;
}

function envMutationHits(filePath, contents) {
  const hits = [];
  for (const match of contents.matchAll(new RegExp(ENV_MUTATION_PATTERN, "g"))) {
    hits.push({
      filePath,
      line: lineNumber(contents, match.index),
      snippet: contents
        .slice(match.index, contents.indexOf("\n", match.index) === -1 ? undefined : contents.indexOf("\n", match.index))
        .trim(),
    });
  }
  return hits;
}

async function scan() {
  const files = [];
  const violations = [];

  for (const filePath of await rustFiles()) {
    const contents = await fs.readFile(path.join(repoRoot, filePath), "utf8");
    const hits = envMutationHits(filePath, contents);
    if (hits.length === 0) {
      continue;
    }
    const hasSerializedGuard = fileHasSerializedEnvGuard(contents);
    files.push({ filePath, hits: hits.length, hasSerializedGuard });
    if (!hasSerializedGuard) {
      violations.push(...hits);
    }
  }

  return { files, violations };
}

function printHuman(report) {
  if (report.violations.length === 0) {
    process.stdout.write(`env mutation guard: ok (${report.files.length} guarded file(s))\n`);
    return;
  }

  process.stderr.write("env mutation guard failed:\n");
  for (const violation of report.violations) {
    process.stderr.write(
      `  - ${violation.filePath}:${violation.line}: ${violation.snippet}\n`,
    );
  }
  process.stderr.write("\nWrap environment mutation in a restore guard that holds a shared env lock.\n");
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  const report = await scan();
  if (args.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    printHuman(report);
  }
  if (report.violations.length > 0) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`env-mutation-guard: ${error.message}\n`);
  process.exitCode = 1;
});
