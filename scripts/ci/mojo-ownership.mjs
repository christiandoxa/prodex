#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { repoRoot } from "../npm/common.mjs";

const manifestPath = path.join(repoRoot, "migration", "mojo-ownership.json");

function parseArgs(argv) {
  const args = {
    baseline: null,
    check: false,
    json: false,
    releaseSha: "WORKTREE",
    selfTest: false,
  };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--baseline") {
      args.baseline = argv[++index];
      if (!args.baseline) throw new Error("--baseline requires a SHA");
      continue;
    }
    if (value === "--release-sha") {
      args.releaseSha = argv[++index];
      if (!args.releaseSha) throw new Error("--release-sha requires a SHA");
      continue;
    }
    if (value === "--check") {
      args.check = true;
      continue;
    }
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--self-test") {
      args.selfTest = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function sourceText(manifest, revision, relativePath) {
  if (revision === "WORKTREE") {
    return readFileSync(path.join(repoRoot, relativePath), "utf8");
  }
  try {
    return execFileSync("git", ["show", `${revision}:${relativePath}`], {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
  } catch (error) {
    if (manifest.baseline_optional_sources?.includes(relativePath)) return "";
    throw error;
  }
}

function isCommentOrDirective(line, language) {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith("//") || trimmed.startsWith("/*") || trimmed === "*") {
    return true;
  }
  if (language === "rust") {
    return trimmed.startsWith("*") || trimmed.startsWith("*/") || trimmed.startsWith("use ") ||
      trimmed.startsWith("pub use ") || trimmed.startsWith("#[") || trimmed.startsWith("#![");
  }
  return trimmed.startsWith("#") || trimmed.startsWith("from ") || trimmed.startsWith("import ") ||
    trimmed.startsWith("@export") || trimmed.startsWith("abi(\"");
}

function rustProductionLines(text) {
  const lines = text.split(/\r?\n/);
  const output = [];
  let testDepth = 0;
  let pendingTestModule = false;
  for (const line of lines) {
    const trimmed = line.trim();
    if (testDepth > 0) {
      testDepth += (line.match(/{/g) ?? []).length;
      testDepth -= (line.match(/}/g) ?? []).length;
      continue;
    }
    if (trimmed.includes("#[cfg(test)]") || trimmed.includes("#[cfg(any(test")) {
      pendingTestModule = true;
      continue;
    }
    if (pendingTestModule) {
      if (trimmed.includes("{")) {
        testDepth = (line.match(/{/g) ?? []).length - (line.match(/}/g) ?? []).length;
        pendingTestModule = false;
      }
      continue;
    }
    if (!isCommentOrDirective(line, "rust")) output.push(line);
  }
  return output;
}

export function countSemanticLines(text, language) {
  if (language === "rust") return rustProductionLines(text).length;
  return text
    .split(/\r?\n/)
    .filter((line) => !isCommentOrDirective(line, "mojo")).length;
}

function percent(part, total) {
  return total === 0 ? 0 : (part * 100) / total;
}

function readManifest() {
  return JSON.parse(readFileSync(manifestPath, "utf8"));
}

function inventory(manifest, revision) {
  const rust = manifest.rust_deterministic_sources.reduce(
    (total, relativePath) => total + countSemanticLines(sourceText(manifest, revision, relativePath), "rust"),
    0,
  );
  const mojo = manifest.mojo_deterministic_sources.reduce(
    (total, relativePath) => total + countSemanticLines(sourceText(manifest, revision, relativePath), "mojo"),
    0,
  );
  const total = rust + mojo;
  return {
    mojo_loc: mojo,
    mojo_percent: percent(mojo, total),
    rust_loc: rust,
    total_loc: total,
  };
}

function validateManifest(manifest) {
  const sourceContents = manifest.mojo_deterministic_sources.map((relativePath) => [
    relativePath,
    sourceText(manifest, "WORKTREE", relativePath),
  ]);
  for (const operation of manifest.authoritative_operations) {
    assert(
      sourceContents.some(([, contents]) => contents.includes(operation.mojo_entry)),
      `${operation.name} is missing its Mojo entry point ${operation.mojo_entry}`,
    );
  }
  assert(manifest.authoritative_operations.length >= 6, "at least six Mojo operations are required");
  assert(
    new Set(manifest.authoritative_operations.map((operation) => operation.domain.split("/")[0])).size >= 4,
    "Mojo operations must span at least four deterministic domains",
  );
}

export function calculateOwnership(manifest, baselineRevision, releaseRevision) {
  const baseline = inventory(manifest, baselineRevision);
  const final = inventory(manifest, releaseRevision);
  return {
    baseline,
    final,
    percentage_point_increase: final.mojo_percent - baseline.mojo_percent,
    authoritative_operation_count: manifest.authoritative_operations.length,
    authoritative_operations: manifest.authoritative_operations,
  };
}

function selfTest() {
  assert.equal(countSemanticLines("// comment\nuse std::x;\nfn main() {\n}\n", "rust"), 2);
  assert.equal(countSemanticLines("# comment\nfrom std import x\ndef f():\n    return 1\n", "mojo"), 2);
  const result = calculateOwnership(
    {
      rust_deterministic_sources: [],
      mojo_deterministic_sources: [],
      authoritative_operations: [],
    },
    "HEAD",
    "HEAD",
  );
  assert.equal(result.final.mojo_percent, 0);
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.selfTest) selfTest();
  const manifest = readManifest();
  validateManifest(manifest);
  const result = calculateOwnership(
    manifest,
    args.baseline ?? manifest.baseline_sha,
    args.releaseSha,
  );
  if (args.check && result.final.mojo_percent < manifest.minimum_percent) {
    throw new Error(
      `Mojo deterministic ownership ${result.final.mojo_percent.toFixed(2)}% is below ${manifest.minimum_percent}%`,
    );
  }
  if (args.json) {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    return;
  }
  process.stdout.write(
    [
      `baseline: ${result.baseline.mojo_percent.toFixed(2)}% Mojo (${result.baseline.mojo_loc}/${result.baseline.total_loc})`,
      `final: ${result.final.mojo_percent.toFixed(2)}% Mojo (${result.final.mojo_loc}/${result.final.total_loc})`,
      `increase: ${result.percentage_point_increase.toFixed(2)} percentage points`,
      `authoritative operations: ${result.authoritative_operation_count}`,
    ].join("\n") + "\n",
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`mojo-ownership: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
