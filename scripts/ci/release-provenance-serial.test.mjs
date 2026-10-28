import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
const block = workflow.split("      - name: Verify release provenance\n")[1]
  .split("\n      - name:")[0];
const shell = block.split("        run: |\n")[1]
  .split("\n").map(line => line.startsWith("          ") ? line.slice(10) : line).join("\n");
const source = "a".repeat(40);

function fixture(t, names, failure = "") {
  const dir = mkdtempSync(path.join(os.tmpdir(), "prodex-provenance-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  mkdirSync(path.join(dir, "release-assets"));
  mkdirSync(path.join(dir, "bin"));
  for (const name of names) writeFileSync(path.join(dir, "release-assets", name), "fixture");
  const gh = `#!${process.execPath}
const fs = require('node:fs');
const path = require('node:path');
const args = process.argv.slice(2);
const asset = path.basename(args[2]);
if (args[0] !== 'attestation' || args[1] !== 'verify') process.exit(80);
for (const [flag, value] of [['--repo','fixture/repo'],['--source-digest','${source}'],
  ['--source-ref','refs/heads/main'],['--signer-workflow','fixture/repo/.github/workflows/standalone-release.yml']]) {
  if (args[args.indexOf(flag)+1] !== value) process.exit(81);
}
if (!args.includes('--deny-self-hosted-runners')) process.exit(82);
const lock = path.join(process.cwd(), 'trust-cache.lock');
try { fs.mkdirSync(lock); } catch { console.error('concurrent trust-cache initialization'); process.exit(83); }
fs.appendFileSync('verified.jsonl',JSON.stringify(asset)+'\\n');
setTimeout(() => { fs.rmdirSync(lock); process.exit(asset === process.env.FAILED_ASSET ? 7 : 0); }, 100);
`;
  writeFileSync(path.join(dir, "bin", "gh"), gh, { mode: 0o700 });
  const result = spawnSync("bash", ["-c", shell], {
    cwd: dir, encoding: "utf8", timeout: 10000,
    env: { ...process.env, PATH: path.join(dir,"bin") + path.delimiter + process.env.PATH,
      RUNNER_TEMP: dir, GITHUB_REPOSITORY: "fixture/repo", TARGET_SHA: source, FAILED_ASSET: failure },
  });
  let calls = [];
  try { calls = readFileSync(path.join(dir,"verified.jsonl"),"utf8").trim().split("\n").map(JSON.parse); }
  catch (error) { if (error.code !== "ENOENT") throw error; }
  return { result, calls };
}
const unix = { skip: process.platform === "win32" };
test("provenance checks cannot race one shared trust cache and verify every asset", unix, t => {
  const names = ["first.bin","a manifest.json","last.bin"];
  const { result, calls } = fixture(t, names);
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.deepEqual(calls.sort(), names.sort());
  assert.doesNotMatch(result.stdout + result.stderr, /concurrent trust-cache/);
});
test("one invalid signature fails publication while retaining every asset verdict", unix, t => {
  const names = ["valid.bin","invalid.bin","also-valid.bin"];
  const { result, calls } = fixture(t, names, "invalid.bin");
  assert.notEqual(result.status, 0);
  assert.deepEqual(calls.sort(), names.sort());
  assert.match(result.stdout + result.stderr, /Release provenance verification failed: release-assets\/invalid.bin/);
});
test("empty artifact directory cannot produce a successful provenance verdict", unix, t => {
  const { result, calls } = fixture(t, []);
  assert.notEqual(result.status, 0);
  assert.deepEqual(calls, []);
});
