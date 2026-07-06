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

const CURRENT_TREE_OUTPUT = `block-buffer v0.10.4
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

cfg_aliases v0.1.1
    nix v0.28.0

cfg_aliases v0.2.1
    nix v0.31.3

digest v0.10.7 (*)

digest v0.11.3 (*)

fallible-iterator v0.2.0
    rusqlite v0.40.0

fallible-iterator v0.3.0
    der v0.8.0

getrandom v0.2.17
    rand_core v0.6.4

getrandom v0.3.4
    rand_core v0.9.5

getrandom v0.4.2
    tempfile v3.27.0

hashbrown v0.16.1
    kasuari v0.4.12

hashbrown v0.17.1
    hashlink v0.12.0

itertools v0.13.0
    criterion v0.8.2

itertools v0.14.0
    ratatui-core v0.1.2

nix v0.28.0
    portable-pty v0.9.0

nix v0.31.3
    os_info v3.15.0

rand v0.9.4
    provider crypto v0.1.0

rand v0.10.1
    jsonwebtoken v10.4.0

rand_core v0.6.4 (*)

rand_core v0.9.5 (*)

rand_core v0.10.1
    rand v0.10.1

thiserror v1.0.69
    filedescriptor v0.8.3

thiserror v2.0.18
    tungstenite v0.29.0

thiserror-impl v1.0.69
    thiserror v1.0.69

thiserror-impl v2.0.18
    thiserror v2.0.18

untrusted v0.7.1
    webpki v0.22.4

untrusted v0.9.0
    aws-lc-rs v1.15.0
`;

test("default budget accepts current duplicate families", () => {
  const duplicateFamilies = parseCargoTreeDuplicates(CURRENT_TREE_OUTPUT);
  const summary = evaluateDuplicateBudget(duplicateFamilies);

  assert.equal(summary.status, "ok");
  assert.equal(budgetFailed(summary), false);
  assert.deepEqual(
    duplicateFamilies.map((entry) => [entry.name, entry.versions.length]),
    [
      ["block-buffer", 2],
      ["cfg_aliases", 2],
      ["cpufeatures", 2],
      ["crypto-common", 2],
      ["digest", 2],
      ["fallible-iterator", 2],
      ["getrandom", 3],
      ["hashbrown", 2],
      ["itertools", 2],
      ["nix", 2],
      ["rand", 2],
      ["rand_core", 3],
      ["thiserror", 2],
      ["thiserror-impl", 2],
      ["untrusted", 2],
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
  const duplicateFamilies = parseCargoTreeDuplicates(`${CURRENT_TREE_OUTPUT}\nrand_core v0.11.0\n`);
  const summary = evaluateDuplicateBudget(duplicateFamilies);

  assert.equal(summary.status, "failed");
  assert.deepEqual(summary.overBudgetFamilies, [
    {
      name: "rand_core",
      versions: ["0.6.4", "0.9.5", "0.10.1", "0.11.0"],
      maxVersions: 3,
      reason: "crypto-common, rand/proptest, and JWT AWS-LC dependencies currently span rand_core 0.6, 0.9, and 0.10.",
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
