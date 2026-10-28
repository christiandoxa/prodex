import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { partitionTestPlan, runBundle, sha256, testInventory, testPlan } from "./native-test-artifact.mjs";
import { windowsGithubMatrix } from "./prodex-app-test-shards.mjs";

const names = ["a::case", "a::case_extended", "b::case", "b::skip_case", "c::case", "login::blocked"];
const listing = names.map((name) => `${name}: test`).join("\n");
const base = testPlan("", "skip_case", true);

test("tail partitions retain all eligible tests exactly once, including prefix-colliding names", () => {
  const plans = [0, 1].map((index) => partitionTestPlan(base, listing, {index: String(index), count: "2"}));
  const selected = plans.flatMap((plan) => plan.flatMap((entry) => entry.args.slice(2)));
  assert.deepEqual(selected.sort(), ["a::case", "a::case_extended", "b::case", "c::case"]);
  assert.equal(new Set(selected).size, selected.length);
  for (const plan of plans) {
    assert.deepEqual(plan[0].args.slice(0, 2), ["--test-threads=1", "--exact"]);
    assert.equal(plan[0].expectedCount, 2);
    assert.ok(!plan[0].args.includes("--skip"), "substring exclusions must be applied before exact mode");
  }
  assert.equal(partitionTestPlan(base, listing), base);
  assert.equal(partitionTestPlan(base, listing, {index: "", count: ""}), base);
});

test("invalid or empty subshards fail instead of silently running the entire corpus", () => {
  for (const options of [{index: "0"}, {count: "2"}, {index: "-1", count: "2"},
    {index: "2", count: "2"}, {index: "0", count: "0"}, {index: "1.5", count: "2"}]) {
    assert.throws(() => partitionTestPlan(base, listing, options));
  }
  assert.throws(() => partitionTestPlan(testPlan("missing", ""), listing, {index: "0", count: "2"}), /matched no tests/);
  assert.throws(() => partitionTestPlan(base, listing + "\na::case: test", {index: "0", count: "2"}), /duplicate/);
});

test("Windows emits both remainder owners with identical exclusions and independent runner identities", () => {
  const entries = windowsGithubMatrix().include;
  const tail = entries.filter((entry) => entry.subshard_count);
  assert.equal(entries.length, 16);
  assert.deepEqual(tail.map((entry) => entry.suite), ["remainder-1", "remainder-2"]);
  assert.deepEqual(tail.map((entry) => [entry.subshard_index, entry.subshard_count]), [["0", "2"], ["1", "2"]]);
  assert.equal(tail[0].filters, "");
  assert.equal(tail[0].skip_filters, tail[1].skip_filters);
  assert.ok(entries.slice(0, 2).every((entry) => entry.suite.startsWith("remainder-")));
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  assert.match(workflow, /PRODEX_APP_SHARD_INDEX: \$\{\{ matrix.subshard_index \}\}/);
  assert.match(workflow, /PRODEX_APP_SHARD_COUNT: \$\{\{ matrix.subshard_count \}\}/);
});

function fixture(t, wrongCount = false) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "prodex-tail-tests-"));
  t.after(() => rmSync(dir, {recursive: true, force: true}));
  mkdirSync(path.join(dir, "crates/prodex-app"), {recursive: true});
  const calls = path.join(dir, "calls.jsonl");
  const payload = `#!${process.execPath}
import fs from 'node:fs';
const names=${JSON.stringify(names)};
const args=process.argv.slice(2);
if(args.includes('--list')) {console.log(names.map(name=>name+': test').join('\\n'));process.exit(0);}
const selected=names.filter(name=>args.includes(name));
fs.appendFileSync(${JSON.stringify(calls)},JSON.stringify({args,selected,cwd:process.cwd()})+'\\n');
console.log('running '+selected.length+' tests');
console.log('test result: ok. '+(${wrongCount} ? 1 : selected.length)+' passed; 0 failed; 0 ignored; 0 measured');
`;
  const expected = {source_sha: "a".repeat(40), run_id: "42", attempt: 1,
    platform: "linux", arch: "x64", checkout_path_sha256: "b".repeat(64)};
  const manifest = JSON.stringify({schema: 1, package: "prodex-app", ...expected,
    binary: "prodex-app-tests", binary_sha256: sha256(payload), inventory: testInventory(listing)});
  writeFileSync(path.join(dir, "prodex-app-tests"), payload, {mode: 0o700});
  writeFileSync(path.join(dir, "manifest.json"), manifest);
  return {dir, calls, expected, digest: sha256(manifest)};
}

test("verified bundle executes exact-name tail shards with serial isolation and count validation", {skip: process.platform === "win32"}, (t) => {
  const f = fixture(t);
  for (const index of [0, 1]) runBundle(f.dir, f.expected, f.digest, {
    repository: f.dir, skips: "skip_case\nlogin::", subshard: {index: String(index), count: "2"},
  });
  const calls = readFileSync(f.calls, "utf8").trim().split("\n").map(JSON.parse);
  assert.equal(calls.length, 2);
  assert.deepEqual(calls.flatMap((call) => call.selected).sort(), ["a::case", "a::case_extended", "b::case", "c::case"]);
  assert.ok(calls.every((call) => call.cwd === path.join(f.dir, "crates/prodex-app")));
});

test("positive but incomplete execution cannot satisfy a tail shard", {skip: process.platform === "win32"}, (t) => {
  const f = fixture(t, true);
  assert.throws(() => runBundle(f.dir, f.expected, f.digest, {
    repository: f.dir, skips: "skip_case\nlogin::", subshard: {index: "0", count: "2"},
  }), /partitions failed/);
});
