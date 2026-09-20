import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  DEFAULT_DUPLICATE_BUDGET,
  budgetFailed,
  evaluateDuplicateBudget,
  parseCargoTreeDuplicates,
} from "./dependency-duplicate-guard.mjs";

const SCRIPT_PATH = new URL("./dependency-duplicate-guard.mjs", import.meta.url).pathname;

const CURRENT_TREE_OUTPUT = `aes v0.8.4
    secret-service v5.1.0

aes v0.9.2
    aes-gcm-siv v0.12.0

base64 v0.22.1
    tiktoken-rs v0.12.0

base64 v0.23.1
    prodex-profile-export v0.402.0

block-buffer v0.10.4
    digest v0.10.7

block-buffer v0.12.0
    digest v0.11.3

cpufeatures v0.2.17
    sha1 v0.10.6

cpufeatures v0.3.0
    sha2 v0.11.0

crypto-common v0.1.7
    digest v0.10.7

crypto-common v0.2.1
    digest v0.11.3

cipher v0.4.4
    secret-service v5.1.0

cipher v0.5.2
    aes-gcm-siv v0.12.0

digest v0.10.7 (*)

digest v0.11.3 (*)

getrandom v0.2.17
    legacy-crypto v0.1.0

getrandom v0.4.3
    tempfile v3.27.0

hmac v0.12.1
    secret-service v5.1.0

hmac v0.13.0
    pbkdf2 v0.13.0

inout v0.1.4
    cipher v0.4.4

inout v0.2.2
    cipher v0.5.2

sha2 v0.10.9
    secret-service v5.1.0

sha2 v0.11.0
    prodex-secret-store v0.307.0

syn v2.0.119
    tokio-macros v2.6.0

syn v3.0.0
    serde_derive v1.0.229
`;

test("default budget accepts current duplicate families", () => {
  const duplicateFamilies = parseCargoTreeDuplicates(CURRENT_TREE_OUTPUT);
  const summary = evaluateDuplicateBudget(duplicateFamilies);

  assert.equal(summary.status, "ok");
  assert.equal(budgetFailed(summary), false);
  assert.deepEqual(
    duplicateFamilies.map((entry) => [entry.name, entry.versions.length]),
    [
      ["aes", 2],
      ["base64", 2],
      ["block-buffer", 2],
      ["cipher", 2],
      ["cpufeatures", 2],
      ["crypto-common", 2],
      ["digest", 2],
      ["getrandom", 2],
      ["hmac", 2],
      ["inout", 2],
      ["sha2", 2],
      ["syn", 2],
    ],
  );
  assert.equal(summary.duplicateFamilyBudget, DEFAULT_DUPLICATE_BUDGET.length);
});

test("unallowlisted duplicate family fails", () => {
  const duplicateFamilies = parseCargoTreeDuplicates(`${CURRENT_TREE_OUTPUT}\nregex v1.11.1\n\nregex v1.12.0\n`);
  const summary = evaluateDuplicateBudget(duplicateFamilies);

  assert.equal(summary.status, "failed");
  assert.deepEqual(
    summary.unallowlistedFamilies.map((entry) => entry.name),
    ["regex"],
  );
});

test("allowed family over version budget fails", () => {
  const duplicateFamilies = parseCargoTreeDuplicates("rand_core v0.6.4\n\nrand_core v0.10.1\n\nrand_core v0.11.0\n");
  const summary = evaluateDuplicateBudget(duplicateFamilies, [{
    name: "rand_core",
    maxVersions: 2,
    reason: "fixture budget for the over-budget test",
  }]);

  assert.equal(summary.status, "failed");
  assert.deepEqual(summary.overBudgetFamilies, [
    {
      name: "rand_core",
      versions: ["0.6.4", "0.10.1", "0.11.0"],
      maxVersions: 2,
      reason: "fixture budget for the over-budget test",
    },
  ]);
});

test("stale budget entry fails so duplicate reductions are ratcheted", () => {
  const duplicateFamilies = parseCargoTreeDuplicates(CURRENT_TREE_OUTPUT).filter((entry) => entry.name !== "digest");
  const summary = evaluateDuplicateBudget(duplicateFamilies);

  assert.equal(summary.status, "failed");
  assert.deepEqual(
    summary.staleBudgetEntries.map((entry) => [entry.name, entry.versions]),
    [["digest", []]],
  );
});

test("CLI accepts fixture input and exits nonzero on new duplicate family", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-dep-duplicate-guard-"));
  try {
    const inputPath = path.join(root, "cargo-tree-d.txt");
    await fs.writeFile(inputPath, `${CURRENT_TREE_OUTPUT}\nregex v1.11.1\n\nregex v1.12.0\n`, "utf8");

    const result = spawnSync(process.execPath, [SCRIPT_PATH, "--input", inputPath, "--json"], {
      encoding: "utf8",
    });

    assert.equal(result.status, 1);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.status, "failed");
    assert.deepEqual(
      payload.unallowlistedFamilies.map((entry) => entry.name),
      ["regex"],
    );
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
