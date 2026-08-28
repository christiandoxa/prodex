import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";
import {
  calculateOwnership,
  countSemanticLines,
  ownershipMeetsMinimum,
  validateManifest,
} from "./mojo-ownership.mjs";

const BASE_SHA = "6f0f632a178492647da764e3522ff2092db40fb3";

function releaseManifest() {
  return JSON.parse(fs.readFileSync("migration/mojo-ownership.json", "utf8"));
}

test("Mojo ownership counter excludes comments, imports, and Rust test modules", () => {
  assert.equal(
    countSemanticLines(
      "// comment\nuse std::fmt;\n#[cfg(test)]\nmod tests {\nfn ignored() {}\n}\nfn production() {}\n",
      "rust",
    ),
    1,
  );
  assert.equal(
    countSemanticLines("# comment\nfrom std import Pointer\n@export(\"x\")\ndef production():\n    return 1\n", "mojo"),
    2,
  );
});

test("ownership result is deterministic for an explicit inventory", () => {
  const manifest = {
    rust_deterministic_sources: [],
    mojo_deterministic_sources: [],
    authoritative_operations: [],
  };
  const first = calculateOwnership(manifest, "HEAD", "HEAD");
  const second = calculateOwnership(manifest, "HEAD", "HEAD");
  assert.deepEqual(first, second);
});

test("removing a baseline Rust source requires a reduction record", () => {
  const manifest = releaseManifest();
  manifest.release_inventory.removed = [
    manifest.baseline_inventory.find((entry) => entry.path.endsWith("provider-core/src/constraints.rs")).path,
  ];
  assert.throws(
    () => validateManifest(manifest, BASE_SHA, "WORKTREE"),
    /removed from the release manifest without a Rust reduction record/,
  );
});

test("dead Mojo source is not migration evidence", () => {
  const manifest = releaseManifest();
  manifest.release_inventory.additions.push({
    path: "migration/mojo-ownership.json",
    language: "mojo",
    classification: "DETERMINISTIC_DOMAIN",
    production_reachable: true,
    operation: "provider_catalog_model_identity",
  });
  assert.throws(
    () => validateManifest(manifest, BASE_SHA, "WORKTREE"),
    /counted but not reachable from selected Mojo production sources/,
  );
});

test("test-only Mojo source contributes no ownership", () => {
  const manifest = releaseManifest();
  const before = calculateOwnership(manifest, BASE_SHA, "WORKTREE");
  manifest.release_inventory.additions.push({
    path: "migration/mojo-ownership.json",
    language: "mojo",
    classification: "DETERMINISTIC_DOMAIN",
    production_reachable: false,
    operation: "provider_catalog_model_identity",
  });
  const after = calculateOwnership(manifest, BASE_SHA, "WORKTREE");
  assert.deepEqual(after.final, before.final);
});

test("baseline remains frozen while the final inventory evolves", () => {
  const manifest = releaseManifest();
  const before = calculateOwnership(manifest, BASE_SHA, "WORKTREE").baseline;
  manifest.release_inventory.overrides["mojo/prodex_core/quota_pressure.mojo"].semantic_loc = 1;
  const after = calculateOwnership(manifest, BASE_SHA, "WORKTREE").baseline;
  assert.deepEqual(after, before);
});

test("the frozen baseline records the Rust denominator and migration floor", () => {
  const result = calculateOwnership(releaseManifest(), BASE_SHA, BASE_SHA);
  assert.equal(result.baseline.rust_loc, 4727);
  assert.equal(result.required_migration_volume_loc, 473);
  assert.equal(result.baseline_remaining_rust_semantic_loc, 4727);
  assert.equal(result.rust_semantic_loc_migrated, 0);
  assert.equal(result.rust_semantic_migration_percent, 0);
  assert.equal(result.baseline_mojo_percent, result.baseline.mojo_percent);
  assert.equal(result.final_mojo_percent, result.final.mojo_percent);
  assert.equal(result.baseline_authoritative_operation_count, 14);
});

test("release inventory cannot hide baseline Rust semantic lines", () => {
  const manifest = releaseManifest();
  const entry = manifest.baseline_inventory.find((candidate) =>
    candidate.path.endsWith("provider-core/src/constraints.rs"));
  manifest.release_inventory.overrides[entry.path] = {
    classification: "SYSTEM_BOUNDARY",
  };
  assert.throws(
    () => validateManifest(manifest, BASE_SHA, "WORKTREE"),
    /without a traceable reduction/,
  );
});

test("baseline Mojo ownership cannot regress", () => {
  const manifest = releaseManifest();
  for (const entry of manifest.baseline_inventory.filter((candidate) => candidate.language === "mojo")) {
    manifest.release_inventory.overrides[entry.path] = {
      ...(manifest.release_inventory.overrides[entry.path] ?? {}),
      classification: "SYSTEM_BOUNDARY",
    };
  }
  assert.throws(
    () => calculateOwnership(manifest, BASE_SHA, "WORKTREE"),
    /Mojo semantic ownership regressed/,
  );
});

test("baseline authoritative operations remain continuous", () => {
  const manifest = releaseManifest();
  manifest.authoritative_operations.shift();
  assert.throws(
    () => validateManifest(manifest, BASE_SHA, "WORKTREE"),
    /baseline authoritative operation .* is missing/,
  );
});

test("Rust reductions are traceable in both baseline and release source", () => {
  const manifest = releaseManifest();
  manifest.rust_semantic_reductions[0].symbol = "missing_reduction_symbol";
  assert.throws(
    () => validateManifest(manifest, BASE_SHA, "WORKTREE"),
    /not traceable in the frozen baseline source/,
  );
});

test("authoritative operation metadata must point at its real exported entry", () => {
  const manifest = releaseManifest();
  manifest.authoritative_operations[0].mojo_entry = "not-shipped";
  assert.throws(
    () => validateManifest(manifest, BASE_SHA, "WORKTREE"),
    /baseline authoritative operation contract changed|Mojo entry is not exported by its source/,
  );
});

test("the ownership threshold is exact rather than rounded", () => {
  assert.equal(ownershipMeetsMinimum({ final: { mojo_percent: 19.99 } }, 20), false);
  assert.equal(ownershipMeetsMinimum({ final: { mojo_percent: 20 } }, 20), true);
});
