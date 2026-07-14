import assert from "node:assert/strict";
import test from "node:test";
import { isVersionMetadataChangePath } from "./release-guard-common.mjs";
import { RELEASE_HYGIENE_POLICY, releaseHygieneSteps } from "./release-hygiene.mjs";

test("fuzz lock workspace versions count as release metadata", () => {
  const filePath = "fuzz/Cargo.lock";
  const change = {
    changedLinesByFile: new Map([
      [filePath, [{ hunkContext: 'name = "prodex-core"', text: 'version = "0.278.0"' }]],
    ]),
  };

  assert.equal(isVersionMetadataChangePath(change, filePath), true);
});

test("release hygiene policy keeps ordered mandatory guards and fixtures", () => {
  assert.deepEqual(
    RELEASE_HYGIENE_POLICY.map((entry) => entry.label),
    [
      "changelog-noise-guard",
      "release-metadata-only-guard",
      "version-metadata-release-guard",
      "release-changelog-coupling-guard",
      "release-empty-commit-guard",
      "release-duplicate-version-guard",
      "release-tag-changelog-guard",
      "release-hygiene-tests",
      "release-run-tests",
      "changelog-tests",
      "release-guard-fixtures",
      "release-cut-fixtures",
    ],
  );
});

test("release hygiene selector args are mapped by guard type", () => {
  const steps = releaseHygieneSteps({
    base: "base-sha",
    head: "head-sha",
    fixtures: false,
  });

  assert.deepEqual(steps, [
    {
      label: "changelog-noise-guard",
      command: "node",
      args: ["scripts/ci/changelog-noise-guard.mjs", "--base", "base-sha", "--head", "head-sha"],
    },
    {
      label: "release-metadata-only-guard",
      command: "node",
      args: ["scripts/ci/release-metadata-only-guard.mjs", "--base", "base-sha", "--head", "head-sha"],
    },
    {
      label: "version-metadata-release-guard",
      command: "node",
      args: ["scripts/ci/version-metadata-release-guard.mjs", "--base", "base-sha", "--head", "head-sha"],
    },
    {
      label: "release-changelog-coupling-guard",
      command: "node",
      args: [
        "scripts/ci/release-changelog-coupling-guard.mjs",
        "--base",
        "base-sha",
        "--head",
        "head-sha",
      ],
    },
    {
      label: "release-empty-commit-guard",
      command: "node",
      args: ["scripts/ci/release-empty-commit-guard.mjs", "--base", "base-sha", "--head", "head-sha"],
    },
    {
      label: "release-duplicate-version-guard",
      command: "node",
      args: ["scripts/ci/release-duplicate-version-guard.mjs", "--base", "base-sha", "--head", "head-sha"],
    },
    {
      label: "release-tag-changelog-guard",
      command: "node",
      args: ["scripts/ci/release-tag-changelog-guard.mjs", "--base", "base-sha", "--head", "head-sha"],
    },
  ]);
});
