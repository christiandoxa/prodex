import assert from "node:assert/strict";
import test from "node:test";

import { scanRustFileEntries } from "./allow-attribute-guard.mjs";

function scanFixture(contentsByPath, policy) {
  return scanRustFileEntries(Object.entries(contentsByPath), {
    allowAttributeCaps: {},
    allowAttributeLocationKeys: [],
    testOnlyDeadCodeAllowCap: 0,
    testOnlyDeadCodeAllowLocationKeys: [],
    ...policy,
  });
}

test("allows listed direct and test-only dead_code allowances at exact caps", () => {
  const report = scanFixture(
    {
      "src/lib.rs": [
        "#[allow(dead_code)]",
        "fn retained_test_helper() {}",
        "",
        "#[cfg_attr(not(test), allow(dead_code))]",
        "pub fn retained_test_only_api() {}",
      ].join("\n"),
    },
    {
      allowAttributeCaps: { dead_code: 1 },
      allowAttributeLocationKeys: ["dead_code|src/lib.rs|fn retained_test_helper() {}"],
      testOnlyDeadCodeAllowCap: 1,
      testOnlyDeadCodeAllowLocationKeys: ["src/lib.rs|pub fn retained_test_only_api() {}"],
    },
  );

  assert.deepEqual(report.violations, []);
});

test("fails on new direct allow location even when cap is unchanged", () => {
  const report = scanFixture(
    {
      "src/lib.rs": [
        "#[allow(dead_code)]",
        "fn new_test_helper() {}",
      ].join("\n"),
    },
    {
      allowAttributeCaps: { dead_code: 1 },
    },
  );

  assert.equal(report.violations.length, 1);
  assert.equal(report.violations[0].type, "unlisted-allow-location");
  assert.equal(report.violations[0].key, "dead_code|src/lib.rs|fn new_test_helper() {}");
});

test("fails on new cfg_attr not-test dead_code location even when cap is unchanged", () => {
  const report = scanFixture(
    {
      "src/lib.rs": [
        "#[cfg_attr(not(test), allow(dead_code))]",
        "pub fn new_test_only_api() {}",
      ].join("\n"),
    },
    {
      testOnlyDeadCodeAllowCap: 1,
    },
  );

  assert.equal(report.violations.length, 1);
  assert.equal(report.violations[0].type, "unlisted-test-only-dead-code-allow");
  assert.equal(report.violations[0].key, "src/lib.rs|pub fn new_test_only_api() {}");
});

test("fails when direct allow cap is not lowered after removal", () => {
  const report = scanFixture(
    {
      "src/lib.rs": [
        "#[allow(dead_code)]",
        "fn retained_test_helper() {}",
      ].join("\n"),
    },
    {
      allowAttributeCaps: { dead_code: 2 },
      allowAttributeLocationKeys: ["dead_code|src/lib.rs|fn retained_test_helper() {}"],
    },
  );

  assert.equal(report.violations.length, 1);
  assert.equal(report.violations[0].type, "cap-not-ratcheted");
  assert.equal(report.violations[0].name, "dead_code");
});

test("fails when test-only dead_code cap is not lowered after removal", () => {
  const report = scanFixture(
    {
      "src/lib.rs": [
        "#[cfg_attr(not(test), allow(dead_code))]",
        "pub fn retained_test_only_api() {}",
      ].join("\n"),
    },
    {
      testOnlyDeadCodeAllowCap: 2,
      testOnlyDeadCodeAllowLocationKeys: ["src/lib.rs|pub fn retained_test_only_api() {}"],
    },
  );

  assert.equal(report.violations.length, 1);
  assert.equal(report.violations[0].type, "test-only-dead-code-cap-not-ratcheted");
});

test("fails when allowlist entry becomes stale", () => {
  const report = scanFixture(
    {
      "src/lib.rs": "",
    },
    {
      allowAttributeLocationKeys: ["dead_code|src/lib.rs|fn removed_test_helper() {}"],
    },
  );

  assert.equal(report.violations.length, 1);
  assert.equal(report.violations[0].type, "stale-allow-location");
});

test("fails when test-only dead_code allowlist entry becomes stale", () => {
  const report = scanFixture(
    {
      "src/lib.rs": "",
    },
    {
      testOnlyDeadCodeAllowLocationKeys: ["src/lib.rs|pub fn removed_test_only_api() {}"],
    },
  );

  assert.equal(report.violations.length, 1);
  assert.equal(report.violations[0].type, "stale-test-only-dead-code-allow");
});
