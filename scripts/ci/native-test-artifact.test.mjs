import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
function job(name) {
  const start = workflow.indexOf(`\n  ${name}:\n`);
  assert.ok(start >= 0, `missing ${name}`);
  return workflow.slice(start).split(/\n  [a-z][a-z0-9-]*:\n/)[1];
}

test("Windows application shards share one exact-run compiled executable", () => {
  const producer = job("windows-app-test-build");
  const consumer = job("windows-prodex-app");
  assert.match(producer, /native-test-artifact\.mjs pack/);
  assert.match(producer, /manifest_sha256:/);
  assert.match(consumer, /needs: \[changes, windows-app-test-build\]/);
  assert.match(consumer, /TEST_MANIFEST_SHA256: \$\{\{ needs\.windows-app-test-build\.outputs\.manifest_sha256 \}\}/);
  assert.match(consumer, /native-test-artifact\.mjs run/);
  assert.doesNotMatch(consumer, /cargo test|rust-cache@|sccache-action@/);
  assert.match(consumer, /PRODEX_APP_FILTERS:/);
  assert.match(consumer, /PRODEX_APP_SKIP_FILTERS:/);
  assert.match(producer, /if-no-files-found: error/);
  assert.match(producer, /overwrite: true/);
});


import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync as readFile, symlinkSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { WINDOWS_SKIPS, selectTestExecutable, testInventory, testPlan, validateManifest, runBundle } from "./native-test-artifact.mjs";

const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const expected = { source_sha: "a".repeat(40), run_id: "42", attempt: 2, platform: "linux", arch: "x64", checkout_path_sha256: "c".repeat(64) };
const binary = Buffer.from("synthetic executable");
const inventory = testInventory("b::case: test\na::case: test\n\n2 tests, 0 benchmarks\n");
const manifest = {
  schema: 1, package: "prodex-app", ...expected, attempt: 1, binary: "prodex-app-tests",
  binary_sha256: hash(binary), inventory,
};

test("artifact identity is source/run/platform bound and supports rerun-failed in the same run", () => {
  validateManifest(manifest, expected, binary, inventory);
  for (const [key, value] of Object.entries({source_sha: "b".repeat(40), run_id: "43", platform: "win32", arch: "arm64", attempt: 3, checkout_path_sha256: "d".repeat(64), schema: 2, package: "other", binary: "../foreign", binary_sha256: "0".repeat(64)})) {
    assert.throws(() => validateManifest({...manifest, [key]: value}, expected, binary, inventory), key);
  }
  assert.throws(() => validateManifest(manifest, expected, Buffer.from("changed"), inventory));
  assert.throws(() => validateManifest(manifest, expected, binary, {...inventory, count: 1}));
  assert.throws(() => validateManifest({...manifest, attempt: 0}, expected, binary));
});

test("Cargo selection ignores production artifacts and refuses ambiguous test executables", () => {
  const entry = { reason: "compiler-artifact", target: {name: "prodex_app", kind: ["lib"]}, profile: {test: true}, executable: "/synthetic/test-harness" };
  assert.equal(selectTestExecutable([JSON.stringify({...entry, profile:{test:false}}), JSON.stringify(entry)].join("\n")), entry.executable);
  assert.throws(() => selectTestExecutable(JSON.stringify({...entry, executable:null})));
  assert.throws(() => selectTestExecutable([entry, entry].map(JSON.stringify).join("\n")));
  assert.throws(() => testInventory("0 tests, 0 benchmarks"));
  assert.throws(() => testInventory("a: test\na: test"));
});

test("all original Windows skips, shard exclusions and serial scheduling are preserved", () => {
  assert.deepEqual(WINDOWS_SKIPS, ["login::", "quota_doctor::", "run::", "shared_state::", "super_mode::", "continuity_failure_reason_metrics_", "rtk::tests::"]);
  const exclusions = ["already_owned", ...WINDOWS_SKIPS].flatMap((value) => ["--skip", value]);
  assert.deepEqual(testPlan("one\r\ntwo\n", "already_owned\n", true), [
    {filter:"one", args:["one", "--test-threads=1", ...exclusions]},
    {filter:"two", args:["two", "--test-threads=1", ...exclusions]},
  ]);
  assert.deepEqual(testPlan("", "one"), [{filter:null, args:["--test-threads=1", "--skip", "one"]}]);
});

function fixture(t) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "prodex-test-bundle-"));
  t.after(() => rmSync(dir, {recursive:true, force:true}));
  mkdirSync(path.join(dir,"crates/prodex-app"), {recursive:true});
  const source = `#!/usr/bin/env node
import fs from 'node:fs';
const args=process.argv.slice(2);
if(args.includes('--list')) {console.log('a::case: test\\nb::case: test');process.exit(0);}
fs.appendFileSync(${JSON.stringify(path.join(dir, "calls"))}, JSON.stringify({args,cwd:process.cwd()})+'\\n');
if(args.includes('fail')) {console.error('synthetic failure');process.exit(2);}
if(args.includes('zero')) {console.log('running 0 tests');process.exit(0);}
console.log('running 1 test\\ntest result: ok. 1 passed; 0 failed; 0 ignored');
`;
  const payload=Buffer.from(source);
  writeFileSync(path.join(dir,"prodex-app-tests"), payload, {mode:0o700});
  const record=JSON.stringify({...manifest,binary_sha256:hash(payload)});
  writeFileSync(path.join(dir,"manifest.json"),record);
  return {dir, record, digest:hash(record)};
}

test("verified bundle runs every filter from Cargo package cwd and fails closed on zero or failed tests", {skip:process.platform==='win32'}, async (t) => {
  const {dir,digest}=fixture(t);
  const plan={repository:dir,filters:"a\nb",skips:"owned"};
  assert.deepEqual(await runBundle(dir,expected,digest,plan),inventory);
  const calls=readFile(path.join(dir,"calls"),'utf8').trim().split('\n').map(JSON.parse);
  assert.equal(calls.length,2);
  assert.ok(calls.every(call=>call.cwd===path.join(dir,'crates/prodex-app')));
  assert.deepEqual(calls[0].args,['a','--test-threads=1','--skip','owned']);
  await assert.rejects(()=>runBundle(dir,expected,digest,{repository:dir,filters:"zero"}));
  await assert.rejects(()=>runBundle(dir,expected,digest,{repository:dir,filters:"fail\na"}));
  const all=readFile(path.join(dir,"calls"),'utf8').trim().split('\n').map(JSON.parse);
  assert.deepEqual(all.slice(-2).map(call=>call.args[0]),['fail','a'],"later filters must still run after failure");
});

test("tampered and symlinked bundle members cannot run", {skip:process.platform==='win32'}, async (t) => {
  const {dir,digest}=fixture(t);
  await assert.rejects(()=>runBundle(dir,expected,'0'.repeat(64),{repository:dir}));
  writeFileSync(path.join(dir,"prodex-app-tests"),'corrupt');
  await assert.rejects(()=>runBundle(dir,expected,digest,{repository:dir}));
  rmSync(path.join(dir,"prodex-app-tests"));
  symlinkSync(process.execPath,path.join(dir,"prodex-app-tests"));
  await assert.rejects(()=>runBundle(dir,expected,digest,{repository:dir}));
});


import { validateWindowsSecurityJob } from "./supply-chain-guard.mjs";
test("Windows security gate rejects unbound bundles and missing producer compilation", () => {
  assert.deepEqual(validateWindowsSecurityJob(workflow), []);
  for (const marker of [
    "needs: [changes, windows-app-test-build]",
    "TEST_MANIFEST_SHA256: ${{ needs.windows-app-test-build.outputs.manifest_sha256 }}",
    "native-test-artifact.mjs pack", "native-test-artifact.mjs run", "if-no-files-found: error",
  ]) {
    const targeted = [job("windows-app-test-build"), job("windows-prodex-app")]
      .find((body) => body.includes(marker));
    assert.ok(targeted, `Windows fixture marker missing: ${marker}`);
    const damaged = workflow.replace(targeted, targeted.replace(marker, "missing-binding"));
    assert.ok(validateWindowsSecurityJob(damaged).length > 0, marker);
  }
});


test("native Windows preparation and execution retain the original Bash environment", () => {
  assert.match(job("windows-app-test-build"), /id: pack\n\s+shell: bash\n\s+run: node scripts\/ci\/native-test-artifact\.mjs pack/);
  assert.match(job("windows-prodex-app"), /Run Windows prodex-app partition.*\n\s+shell: bash\n/);
});


test("native progress is visible before the executable finishes", {skip: process.platform === "win32"}, async (t) => {
  const {dir} = fixture(t);
  const ack = path.join(dir, "progress-ack");
  const payload = `#!${process.execPath}
import fs from 'node:fs';
if(process.argv.includes('--list')) {console.log('a::case: test\\nb::case: test');process.exit(0);}
console.log('running 1 test\\nNATIVE_PROGRESS_READY');
const started=Date.now();
const timer=setInterval(()=>{
 if(fs.existsSync(${JSON.stringify(ack)})) {clearInterval(timer);console.log('test result: ok. 1 passed; 0 failed; 0 ignored');}
 else if(Date.now()-started>3000) {clearInterval(timer);process.exitCode=7;}
},10);
`;
  writeFileSync(path.join(dir,"prodex-app-tests"),payload,{mode:0o700});
  const record=JSON.stringify({...manifest,binary_sha256:hash(payload)});
  writeFileSync(path.join(dir,"manifest.json"),record);
  const {spawn} = await import("node:child_process");
  const source = `import {runBundle} from ${JSON.stringify(new URL("./native-test-artifact.mjs",import.meta.url).href)};
await runBundle(${JSON.stringify(dir)},${JSON.stringify(expected)},${JSON.stringify(hash(record))},{repository:${JSON.stringify(dir)},filters:'a'});`;
  const child=spawn(process.execPath,["--input-type=module","-e",source],{stdio:["ignore","pipe","pipe"]});
  let output="",errors="",acknowledged=false;
  const timer=setTimeout(()=>child.kill("SIGKILL"),10000);
  child.stdout.on("data",chunk=>{
    output+=chunk;
    if(output.includes("NATIVE_PROGRESS_READY")&&!acknowledged){acknowledged=true;writeFileSync(ack,"continue");}
  });
  child.stderr.on("data",chunk=>{errors+=chunk;});
  const code=await new Promise((resolve,reject)=>{child.on("error",reject);child.on("close",resolve);}).finally(()=>clearTimeout(timer));
  assert.equal(code,0,output+errors);
  assert.ok(acknowledged,"test progress was held until exit");
  assert.match(output,/test result: ok/);
});
