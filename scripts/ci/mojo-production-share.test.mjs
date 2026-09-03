import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import test from "node:test";
import { readCargoVersion } from "../npm/common.mjs";
import {
  assessMojoProductionNonRegression,
  calculateProductionShare,
  countMojoProductionLines,
  countRustProductionLines,
  isProductionRustPath,
  productionInventoryAtRevision,
  productionShareMeetsMinimum,
  productionShareMeetsProjectTarget,
  productionShareMeetsReleaseFloor,
  productionShareMeetsReleaseRequirement,
  validateManifestMetadata,
} from "./mojo-production-share.mjs";

const BASE_SHA = "2531c7a345f1607a18aa926e204b4d02cc322167";
const NON_REGRESSION_BASELINE_SHA = "43768659073cc1ab5c5686d3d58f2af68eebdef2";

function manifest(overrides = {}) {
  return {
    schema_version: 1,
    release_target: "0.421.0",
    baseline_sha: BASE_SHA,
    baseline_snapshot: "migration/mojo-production-share-baseline-0.419.2.json",
    counting_rules_version: 1,
    release_floor_percent: 7,
    project_target_percent: 10,
    mojo_non_regression: {
      baseline_sha: NON_REGRESSION_BASELINE_SHA,
      baseline_source_inventory_sha256: "48cddc617b019666bd72efa5c2fb91c2e1b40400ded1dfeee73877240662c27e",
      baseline_mojo_production_loc: 26_420,
      approved_reductions: [],
    },
    production_build_feature: "mojo-core",
    ...overrides,
  };
}

function share(mojoLoc, overrides = {}) {
  return {
    final: {
      broad_mojo_production_loc: mojoLoc,
      broad_total_production_loc: 10_000,
    },
    release_floor_percent: 7,
    project_target_percent: 10,
    mojo_non_regression_met: true,
    ...overrides,
  };
}

function runShare(...args) {
  return execFileSync(
    process.execPath,
    ["scripts/ci/mojo-production-share.mjs", ...args],
    { encoding: "utf8" },
  );
}

test("broad production metric self-test passes", () => {
  assert.equal(runShare("--self-test"), "");
});

test("production path filter and counting exclusions remain unchanged", () => {
  assert.equal(isProductionRustPath("crates/prodex-app/src/lib.rs"), true);
  assert.equal(isProductionRustPath("crates/prodex-app/src/runtime_tests.rs"), false);
  assert.equal(isProductionRustPath("crates/prodex-app/tests/src/lib.rs"), false);
  assert.equal(isProductionRustPath("crates/prodex-app/src/bench_support/stream_cases.rs"), false);
  assert.equal(isProductionRustPath("migration/abi_probe.rs"), false);
  assert.equal(
    countRustProductionLines(
      "// comment\nuse std::fmt;\n#[cfg(test)]\nmod tests {\nfn ignored() {}\n}\nfn production() {}\n",
    ),
    1,
  );
  assert.equal(
    countMojoProductionLines("# comment\nfrom std import Pointer\n@export(\"x\")\ndef production():\n    return 1\n"),
    2,
  );
  const inventory = productionInventoryAtRevision("WORKTREE");
  assert.equal(inventory.reachable_mojo_sources.some((path) => path.startsWith("migration/")), false);
  assert.equal(inventory.entries.some((entry) => entry.path.startsWith("migration/")), false);
});

test("manifest separates general release floor, project target, and frozen policy", () => {
  const current = manifest();
  assert.equal(validateManifestMetadata(current), current);
  assert.equal(current.release_floor_percent, 7);
  assert.equal(current.project_target_percent, 10);
  assert.throws(
    () => validateManifestMetadata({ ...current, release_floor_percent: 6.99 }),
    /release floor must remain 7%/u,
  );
  assert.throws(
    () => validateManifestMetadata({ ...current, project_target_percent: 9.99 }),
    /project target must remain 10%/u,
  );
  assert.throws(
    () => validateManifestMetadata({
      ...current,
      mojo_non_regression: { ...current.mojo_non_regression, baseline_sha: "wrong" },
    }),
    /non-regression baseline must be/u,
  );
  assert.throws(
    () => validateManifestMetadata({
      ...current,
      mojo_non_regression: { ...current.mojo_non_regression, baseline_sha: BASE_SHA },
    }),
    /non-regression baseline must be 43768659073cc1ab5c5686d3d58f2af68eebdef2/u,
  );
});

test("release floor and project target thresholds are exact", async (t) => {
  const cases = [
    ["6.99%", 699, false, false, false],
    ["7.00%", 700, true, false, true],
    ["7.200617%", 720, true, false, true],
    ["9.99%", 999, true, false, true],
    ["10.00%", 1_000, true, true, true],
  ];
  for (const [label, mojoLoc, floorMet, targetMet, releaseMet] of cases) {
    await t.test(`threshold ${label}`, () => {
      const result = share(mojoLoc);
      assert.equal(productionShareMeetsReleaseFloor(result), floorMet);
      assert.equal(productionShareMeetsProjectTarget(result), targetMet);
      assert.equal(productionShareMeetsMinimum(result), targetMet);
      assert.equal(productionShareMeetsReleaseRequirement(result), releaseMet);
    });
  }
});

test("non-regression checks reachable source identity and absolute Mojo ownership", () => {
  const baseline = {
    entries: [
      { language: "mojo", path: "mojo/prodex_core/first.mojo", loc: 100 },
      { language: "mojo", path: "mojo/prodex_core/second.mojo", loc: 50 },
    ],
    mojo_production_loc: 150,
    reachable_mojo_sources: [
      "mojo/prodex_core/first.mojo",
      "mojo/prodex_core/second.mojo",
    ],
  };
  const replacement = assessMojoProductionNonRegression(baseline, {
    mojo_production_loc: 150,
    reachable_mojo_sources: ["mojo/prodex_core/replacement.mojo"],
  });
  assert.equal(replacement.mojo_production_loc_regressed, false);
  assert.deepEqual(replacement.missing_reachable_mojo_sources, baseline.reachable_mojo_sources);
  assert.equal(replacement.met, false);

  const approved = assessMojoProductionNonRegression(baseline, {
    mojo_production_loc: 50,
    reachable_mojo_sources: ["mojo/prodex_core/second.mojo"],
  }, [{ path: "mojo/prodex_core/first.mojo", reason: "feature removed" }]);
  assert.equal(approved.approved_reduction_loc, 100);
  assert.equal(approved.required_mojo_production_loc, 50);
  assert.equal(approved.met, true);
  assert.throws(
    () => assessMojoProductionNonRegression(baseline, baseline, [
      { path: "mojo/prodex_core/unknown.mojo", reason: "not a baseline source" },
    ]),
    /not a reachable Mojo source/u,
  );
});

const historicalWaiver = {
  release_target: "0.421.0",
  baseline_sha: BASE_SHA,
  temporary_release_floor_percent: 7,
  scope: "0.421.0 only",
  expiration: "immediately after 0.421.0",
  status: "expired",
  reason: "historical quantity-only release waiver",
};

test("historical waiver remains expired, scoped to 0.421.0, and cannot lower the floor", () => {
  const current = manifest({ temporary_release_waiver: historicalWaiver });
  assert.equal(validateManifestMetadata(current), current);
  assert.throws(
    () => validateManifestMetadata({
      ...current,
      temporary_release_waiver: { ...historicalWaiver, release_target: "0.424.0" },
    }),
    /release target must be 0.421.0/u,
  );
  assert.throws(
    () => validateManifestMetadata({
      ...current,
      temporary_release_waiver: { ...historicalWaiver, status: "active" },
    }),
    /scope or expiration is invalid/u,
  );
  for (const version of ["0.421.0", "0.422.0", "0.423.1", "0.424.0"]) {
    const belowFloor = share(699, {
      current_prodex_version: version,
      temporary_release_waiver_applicable: true,
      temporary_release_floor_percent: 7,
      temporary_release_waiver_scope: "0.421.0 only",
    });
    assert.equal(productionShareMeetsReleaseRequirement(belowFloor), false, version);
    assert.equal(
      productionShareMeetsReleaseRequirement({ ...belowFloor, final: { ...belowFloor.final, broad_mojo_production_loc: 700 } }),
      true,
      version,
    );
  }
});

test("canonical report exposes separate statuses and --check enforces only floor plus non-regression", async () => {
  const report = JSON.parse(runShare("--json"));
  assert.equal(report.current_prodex_version, await readCargoVersion());
  assert.equal(report.final.broad_mojo_production_loc, 26_420);
  assert.equal(report.final.broad_total_production_loc, 367_618);
  assert.equal(report.final.broad_mojo_percent, 7.1868080453079015);
  assert.equal(report.release_floor_percent, 7);
  assert.equal(report.release_floor_met, true);
  assert.equal(report.release_floor_status, "PASS");
  assert.equal(report.project_target_percent, 10);
  assert.equal(report.project_target_met, false);
  assert.equal(report.project_target_status, "NOT_YET_MET");
  assert.equal(report.mojo_non_regression_met, true);
  assert.equal(report.mojo_non_regression_status, "PASS");
  assert.equal(report.mojo_non_regression_baseline_mojo_production_loc, 26_420);
  assert.equal(report.release_requirement_met, true);
  assert.equal(report.release_status, "PASS");
  assert.equal(report.temporary_release_waiver_applicable, false);
  assert.equal(report.historical_temporary_release_waiver.release_target, "0.421.0");
  assert.equal(report.historical_temporary_release_waiver.status, "EXPIRED");

  const checked = JSON.parse(runShare("--check", "--json"));
  assert.deepEqual(checked, report);
  const human = runShare();
  assert.match(human, /Release floor status: PASS/u);
  assert.match(human, /Project target status: NOT YET MET/u);
  assert.match(human, /Historical 0\.421\.0 waiver: EXPIRED/u);
});

test("baseline counting remains frozen while non-regression protects current reachable ownership", () => {
  const result = calculateProductionShare(manifest(), BASE_SHA, BASE_SHA);
  assert.equal(result.final.broad_mojo_percent, result.baseline.broad_mojo_percent);
  assert.equal(result.release_floor_met, false);
  assert.equal(result.project_target_met, false);
  assert.equal(result.mojo_non_regression_met, false);
});
