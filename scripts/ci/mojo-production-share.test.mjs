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
    release_target: "0.421.0",
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
    release_target: "0.421.0",
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

test("0.421.0 requires the normal 10 percent requirement", () => {
  const manifest = {
    schema_version: 1,
    release_target: "0.421.0",
    baseline_sha: "2531c7a345f1607a18aa926e204b4d02cc322167",
    counting_rules_version: 1,
    minimum_percent: 10,
    production_build_feature: "mojo-core",
  };
  assert.equal(validateManifestMetadata(manifest), manifest);
  assert.equal(productionShareMeetsMinimum({
    final: { broad_mojo_production_loc: 1_000, broad_total_production_loc: 10_000 },
    minimum_percent: 10,
  }), true);
  assert.equal(productionShareMeetsReleaseRequirement({
    final: { broad_mojo_production_loc: 999, broad_total_production_loc: 10_000 },
    minimum_percent: 10,
  }), false);
});

const validWaiver = {
  release_target: "0.421.0",
  baseline_sha: "2531c7a345f1607a18aa926e204b4d02cc322167",
  temporary_release_floor_percent: 7,
  scope: "0.421.0 only",
  expiration: "immediately after 0.421.0",
  reason: "quantity-only release waiver",
};

test("0.421.0 accepts only its explicit quantity waiver", () => {
  const manifest = {
    schema_version: 1,
    release_target: "0.421.0",
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
    current_prodex_version: "0.421.0",
    temporary_release_waiver_applicable: true,
    temporary_release_floor_percent: 7,
    temporary_release_waiver_scope: "0.421.0 only",
  }), true);
  assert.equal(productionShareMeetsReleaseRequirement({
    final: { broad_mojo_production_loc: 699, broad_total_production_loc: 10_000 },
    minimum_percent: 10,
    current_prodex_version: "0.421.0",
    temporary_release_waiver_applicable: true,
    temporary_release_floor_percent: 7,
    temporary_release_waiver_scope: "0.421.0 only",
  }), false);
  assert.throws(
    () => validateManifestMetadata({ ...manifest, temporary_release_waiver: { ...validWaiver, release_target: "0.422.0" } }),
    /release target must be 0.421.0/u,
  );
  assert.throws(
    () => validateManifestMetadata({ ...manifest, temporary_release_waiver: { ...validWaiver, temporary_release_floor_percent: 6 } }),
    /exactly 7/u,
  );
  assert.throws(
    () => validateManifestMetadata({ ...manifest, temporary_release_waiver: undefined, counting_rules_version: 2 }),
    /counting rules must be 1/u,
  );
  assert.throws(
    () => validateManifestMetadata({ ...manifest, temporary_release_waiver: undefined, baseline_sha: "wrong" }),
    /broad production-share baseline must be/u,
  );
});

for (const version of ["0.421.0", "0.422.0", "0.423.1", "0.424.0"]) {
  test(`${version} release-scoped waiver fixture`, () => {
    assert.equal(productionShareMeetsReleaseRequirement({
      final: { broad_mojo_production_loc: 700, broad_total_production_loc: 10_000 },
      minimum_percent: 10,
      current_prodex_version: version,
      temporary_release_waiver_applicable: true,
      temporary_release_floor_percent: 7,
      temporary_release_waiver_scope: "0.421.0 only",
    }), version === "0.421.0");
  });
}

test("the canonical current version expires the inherited waiver", () => {
  const manifest = {
    schema_version: 1,
    release_target: "0.421.0",
    baseline_sha: "2531c7a345f1607a18aa926e204b4d02cc322167",
    baseline_snapshot: "migration/mojo-production-share-baseline-0.419.2.json",
    counting_rules_version: 1,
    minimum_percent: 10,
    production_build_feature: "mojo-core",
    temporary_release_waiver: validWaiver,
  };
  const result = calculateProductionShare(manifest, manifest.baseline_sha, manifest.baseline_sha);
  assert.equal(result.current_prodex_version, "0.423.1");
  assert.equal(result.temporary_release_waiver_applicable, false);
  assert.equal(result.temporary_release_floor_percent, null);
  assert.equal(result.release_requirement_met, false);
});

test("no waiver keeps the normal 10 percent requirement", () => {
  assert.equal(productionShareMeetsReleaseRequirement({
    final: { broad_mojo_production_loc: 700, broad_total_production_loc: 10_000 },
    minimum_percent: 10,
  }), false);
});
