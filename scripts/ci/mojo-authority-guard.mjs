#!/usr/bin/env node

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { repoRoot } from "../npm/common.mjs";

const MANIFEST_PATH = path.join(repoRoot, "migration", "mojo-ownership.json");
const BASELINE_SHA = "2531c7a345f1607a18aa926e204b4d02cc322167";
const RELEASE_TARGET = "0.420.0";
const VALID_RUST_STATES = new Set(["deleted", "adapter-only", "test-only", "test-oracle-only"]);
const FORBIDDEN_FALLBACK_MARKERS = [
  /\bfallback_to_rust\b/iu,
  /\brust_fallback\b/iu,
  /\buse_rust\b/iu,
  /\bdisable_mojo\b/iu,
  /\bshadow_compare\b/iu,
  /\bpre_mojo\b/iu,
];

function readManifest() {
  return JSON.parse(fs.readFileSync(MANIFEST_PATH, "utf8"));
}

function releaseOperations(manifest) {
  const operations = manifest.authoritative_operations ?? [];
  const overrides = manifest.release_operation_overrides ?? {};
  for (const name of Object.keys(overrides)) {
    if (!operations.some((operation) => operation.name === name)) {
      throw new Error(`release operation override names unknown operation ${name}`);
    }
  }
  return operations.map((operation) => ({
    ...operation,
    ...(overrides[operation.name] ?? {}),
  }));
}

function productionRustFiles() {
  return execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard"], {
    cwd: repoRoot,
    encoding: "utf8",
  }).split(/\r?\n/u).filter((file) =>
    file.endsWith(".rs") &&
    (file.startsWith("src/") || file.startsWith("crates/")) &&
    !/(?:^|\/)(?:tests?|benches|examples|fixtures|snapshots|generated|vendor|target)(?:\/|$)/iu.test(file),
  ).sort();
}

function operationMetadataViolations(manifest) {
  const violations = [];
  if (manifest.release_target !== RELEASE_TARGET) {
    violations.push(`migration release target is ${manifest.release_target}, expected ${RELEASE_TARGET}`);
  }
  if (manifest.baseline_sha !== BASELINE_SHA) {
    violations.push(`migration baseline is ${manifest.baseline_sha}, expected ${BASELINE_SHA}`);
  }
  const operations = releaseOperations(manifest);
  const names = new Set();
  const exportOwners = new Map();
  const reductionKeys = new Set();
  for (const reduction of manifest.rust_semantic_reductions ?? []) {
    const key = [reduction.operation, reduction.file, reduction.symbol].join("\u0000");
    if (reductionKeys.has(key)) {
      violations.push(`duplicate Rust cleanup record ${reduction.operation}:${reduction.file}:${reduction.symbol}`);
    }
    reductionKeys.add(key);
    if (reduction.migrated_semantic_loc !== undefined && reduction.cleanup_loc !== undefined) {
      violations.push(`${reduction.operation}: cleanup record cannot mix migrated_semantic_loc and cleanup_loc`);
    }
    if (reduction.migrated_semantic_loc !== undefined &&
        (!Number.isInteger(reduction.migrated_semantic_loc) || reduction.migrated_semantic_loc <= 0)) {
      violations.push(`${reduction.operation}: migrated_semantic_loc must be positive`);
    }
    if (reduction.cleanup_loc !== undefined &&
        (!Number.isInteger(reduction.cleanup_loc) || reduction.cleanup_loc <= 0)) {
      violations.push(`${reduction.operation}: cleanup_loc must be positive`);
    }
  }
  for (const operation of operations) {
    if (names.has(operation.name)) violations.push(`duplicate authoritative operation ${operation.name}`);
    names.add(operation.name);
    if (operation.final_state !== "authoritative") {
      violations.push(`${operation.name}: final state is not authoritative`);
    }
    if (operation.final_state !== "authoritative") continue;
    if (operation.production_fallback !== false) {
      violations.push(`${operation.name}: production_fallback must be false`);
    }
    if (operation.duplicate_production_owner !== false) {
      violations.push(`${operation.name}: duplicate_production_owner must be false`);
    }
    if (operation.platform_fallback !== false) {
      violations.push(`${operation.name}: platform_fallback must be false`);
    }
    if (operation.final_state === "authoritative") {
      const previousOwner = exportOwners.get(operation.mojo_entry);
      if (previousOwner) {
        violations.push(
          `${operation.name}: Mojo entry ${operation.mojo_entry} is already claimed by ${previousOwner}`,
        );
      } else {
        exportOwners.set(operation.mojo_entry, operation.name);
      }
    }
    if (!VALID_RUST_STATES.has(operation.rust_state_after)) {
      violations.push(`${operation.name}: rust_state_after must be deleted, adapter-only, or test-only`);
    }
    const isNew = operation.introduced_in === RELEASE_TARGET || operation.expanded_in === RELEASE_TARGET;
    if (!isNew) continue;
    const reductions = (manifest.rust_semantic_reductions ?? [])
      .filter((reduction) => reduction.operation === operation.name);
    if (reductions.length === 0) {
      violations.push(`${operation.name}: no traceable Rust cleanup record`);
    } else if (!reductions.some((reduction) =>
      (Number.isInteger(reduction.migrated_semantic_loc) && reduction.migrated_semantic_loc > 0) ||
      (Number.isInteger(reduction.cleanup_loc) && reduction.cleanup_loc > 0))) {
      violations.push(`${operation.name}: Rust cleanup record must claim positive migrated semantic LOC or cleanup_loc`);
    }
  }
  return violations;
}

export function findViolations(manifest, files = []) {
  const violations = operationMetadataViolations(manifest);
  for (const [filePath, contents] of files) {
    for (const marker of FORBIDDEN_FALLBACK_MARKERS) {
      if (marker.test(contents)) violations.push(`${filePath}: forbidden production Mojo fallback marker`);
    }
  }
  return violations;
}

export function validateManifest(manifest, files = []) {
  const violations = findViolations(manifest, files);
  if (violations.length > 0) throw new Error(violations.join("\n"));
  return true;
}

function selfTest() {
  const operation = {
    name: "new_operation",
    introduced_in: RELEASE_TARGET,
    final_state: "authoritative",
    production_fallback: false,
    duplicate_production_owner: false,
    platform_fallback: false,
    rust_state_after: "adapter-only",
  };
  const manifest = {
    baseline_sha: BASELINE_SHA,
    release_target: RELEASE_TARGET,
    authoritative_operations: [operation],
    rust_semantic_reductions: [{ operation: operation.name, migrated_semantic_loc: 1 }],
  };
  assert.equal(validateManifest(manifest, [["x.rs", "fn adapter() {}"]]), true);
  assert.throws(
    () => validateManifest({ ...manifest, authoritative_operations: [{ ...operation, production_fallback: true }] }),
    /production_fallback must be false/u,
  );
  assert.throws(
    () => validateManifest(manifest, [["x.rs", "fn rust_fallback() {}"]]),
    /forbidden production Mojo fallback marker/u,
  );
  assert.throws(
    () => validateManifest({
      ...manifest,
      authoritative_operations: [
        operation,
        { ...operation, name: "other_operation", introduced_in: "0.419.1" },
      ],
    }),
    /Mojo entry .* is already claimed by/u,
  );
  const cleanupManifest = {
    baseline_sha: BASELINE_SHA,
    release_target: RELEASE_TARGET,
    authoritative_operations: [operation],
    rust_semantic_reductions: [{
      operation: operation.name,
      file: "crates/example.rs",
      symbol: "cleanup_symbol",
      cleanup_loc: 1,
    }],
  };
  assert.equal(validateManifest(cleanupManifest, []), true);
  assert.throws(
    () => validateManifest({
      ...cleanupManifest,
      rust_semantic_reductions: [{ ...cleanupManifest.rust_semantic_reductions[0], cleanup_loc: 0 }],
    }, []),
    /cleanup_loc must be positive/u,
  );
  assert.throws(
    () => validateManifest({
      ...cleanupManifest,
      rust_semantic_reductions: [
        ...cleanupManifest.rust_semantic_reductions,
        { ...cleanupManifest.rust_semantic_reductions[0] },
      ],
    }, []),
    /duplicate Rust cleanup record/u,
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    selfTest();
    if (process.argv.length === 3) {
      process.stdout.write("mojo authority guard: self-test ok\n");
      return;
    }
  }
  const manifest = readManifest();
  const files = productionRustFiles().map((file) => [file, fs.readFileSync(path.join(repoRoot, file), "utf8")]);
  validateManifest(manifest, files);
  process.stdout.write("mojo authority guard: ok\n");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`mojo-authority-guard: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
