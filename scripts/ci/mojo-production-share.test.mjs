import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import test from "node:test";
import {
  calculateProductionShare,
  isProductionRustPath,
  productionShareMeetsReleaseRequirement,
  productionShareMeetsMinimum,
  validateManifestMetadata,
} from "./mojo-production-share.mjs";

test("broad production metric self-test passes", () => {
  const output = execFileSync(
    process.execPath,
    ["scripts/ci/mojo-production-share.mjs", "--self-test"],
    { encoding: "utf8" },
  );
  assert.equal(output, "");
});

test("production path filter excludes test-only source and keeps shipped Rust", () => {
  assert.equal(isProductionRustPath("crates/prodex-app/src/lib.rs"), true);
  assert.equal(isProductionRustPath("crates/prodex-app/src/runtime_tests.rs"), false);
  assert.equal(isProductionRustPath("crates/prodex-app/tests/src/lib.rs"), false);
});

test("production-share metadata keeps the active target, build feature, and floor fixed", () => {
  const manifest = {
    schema_version: 1,
    release_target: "0.420.0",
    baseline_sha: "2531c7a345f1607a18aa926e204b4d02cc322167",
    counting_rules_version: 1,
    minimum_percent: 10,
    production_build_feature: "mojo-core",
  };
  assert.equal(validateManifestMetadata(manifest), manifest);
  assert.throws(
    () => validateManifestMetadata({ ...manifest, production_build_feature: "mojo-runtime" }),
    /build feature must be mojo-core/u,
  );
});

test("baseline broad share is independently frozen and exact", () => {
  const manifest = {
    schema_version: 1,
    release_target: "0.420.0",
    baseline_sha: "2531c7a345f1607a18aa926e204b4d02cc322167",
    baseline_snapshot: "migration/mojo-production-share-baseline-0.419.2.json",
    counting_rules_version: 1,
    minimum_percent: 10,
    production_build_feature: "mojo-core",
  };
  const result = calculateProductionShare(manifest, manifest.baseline_sha, manifest.baseline_sha);
  assert.equal(result.final.broad_mojo_percent, result.baseline.broad_mojo_percent);
  assert.equal(productionShareMeetsMinimum(result), false);
  assert.ok(result.additional_mojo_loc_needed_at_final_rust_volume > 0);
});

const validWaiver = {
  release_target: "0.420.0",
  baseline_sha: "2531c7a345f1607a18aa926e204b4d02cc322167",
  temporary_release_floor_percent: 7,
  scope: "0.420.0 only",
  expiration: "immediately after 0.420.0",
  reason: "broad migration continues next release",
};

test("0.420.0 waiver passes only at or above its temporary floor", () => {
  const manifest = {
    schema_version: 1,
    release_target: "0.420.0",
    baseline_sha: "2531c7a345f1607a18aa926e204b4d02cc322167",
    counting_rules_version: 1,
    minimum_percent: 10,
    production_build_feature: "mojo-core",
    temporary_release_waiver: validWaiver,
  };
  assert.equal(validateManifestMetadata(manifest), manifest);
  assert.equal(productionShareMeetsReleaseRequirement({
    final: { broad_mojo_production_loc: 700, broad_total_production_loc: 10_000 },
    minimum_percent: 10,
    temporary_release_waiver_applicable: true,
    temporary_release_floor_percent: 7,
    temporary_release_waiver_scope: "0.420.0 only",
  }), true);
  assert.equal(productionShareMeetsReleaseRequirement({
    final: { broad_mojo_production_loc: 699, broad_total_production_loc: 10_000 },
    minimum_percent: 10,
    temporary_release_waiver_applicable: true,
    temporary_release_floor_percent: 7,
    temporary_release_waiver_scope: "0.420.0 only",
  }), false);
  assert.equal(productionShareMeetsMinimum({
    final: { broad_mojo_production_loc: 700, broad_total_production_loc: 10_000 },
    minimum_percent: 10,
  }), false);
});

test("waiver validation fails closed and is not inherited", () => {
  const manifest = {
    schema_version: 1,
    release_target: "0.420.0",
    baseline_sha: "2531c7a345f1607a18aa926e204b4d02cc322167",
    counting_rules_version: 1,
    minimum_percent: 10,
    production_build_feature: "mojo-core",
    temporary_release_waiver: validWaiver,
  };
  for (const [field, value, message] of [
    ["baseline_sha", "wrong", /baseline SHA/u],
    ["release_target", "0.421.0", /release target/u],
    ["temporary_release_floor_percent", 6, /exactly 7/u],
    ["scope", "all releases", /scope or expiration/u],
    ["reason", "", /reason/u],
  ]) {
    const next = { ...manifest, temporary_release_waiver: { ...validWaiver, [field]: value } };
    assert.throws(() => validateManifestMetadata(next), message);
  }
  assert.throws(
    () => validateManifestMetadata({ ...manifest, release_target: "0.421.0" }),
    /release target must be 0.420.0/u,
  );
  assert.throws(
    () => validateManifestMetadata({ ...manifest, counting_rules_version: 2 }),
    /counting rules must be 1/u,
  );
  assert.throws(
    () => validateManifestMetadata({ ...manifest, baseline_sha: "wrong" }),
    /broad production-share baseline must be/u,
  );
});

test("no waiver keeps the normal 10 percent requirement", () => {
  assert.equal(productionShareMeetsReleaseRequirement({
    final: { broad_mojo_production_loc: 700, broad_total_production_loc: 10_000 },
    minimum_percent: 10,
  }), false);
});
