import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { testInventory } from "./native-test-artifact.mjs";
import { RUNTIME_CI_BROAD_SHARD_FILTERS, RUNTIME_STRESS_SERIALIZED_TESTS, RUNTIME_STRESS_CONTINUATION_TESTS } from "./runtime-test-manifest.mjs";
import { validateLinuxNativeTests } from "./supply-chain-guard.mjs";

const root = process.cwd();
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const runtimeTest = RUNTIME_CI_BROAD_SHARD_FILTERS[0].filter + "fixture";
const names = ["main_internal_tests::fixture::alpha", "main_internal_tests::fixture::beta", runtimeTest].sort();
function fixture(t, testNames = names) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "prodex-internal-bundle-"));
  t.after(() => rmSync(dir, {recursive:true, force:true}));
  const calls = path.join(dir, "calls.jsonl");
  const payload = `#!${process.execPath}
import fs from 'node:fs';
const names=${JSON.stringify(testNames)};
const args=process.argv.slice(2);
if(args.includes('--list')) {console.log(names.map(n=>n+': test').join('\\n'));process.exit(0);}
const skip=args.flatMap((v,i)=>v==='--skip'?[args[i+1]]:[]);
const selected=names.filter(n=>n.includes(args[0])&&!skip.some(s=>n.includes(s)));
fs.appendFileSync(${JSON.stringify(calls)}, JSON.stringify({cwd:process.cwd(),args,selected})+'\\n');
if(process.env.NATIVE_FIXTURE_MODE==='timeout') {setInterval(()=>{},1000);} else {
console.log('running '+selected.length+' tests');
if(process.env.NATIVE_FIXTURE_MODE==='fail') process.exit(7);
console.log('test result: ok. '+selected.length+' passed; 0 failed; 0 ignored');
}
`;
  const sourceSha = spawnSync("git", ["rev-parse", "HEAD"], {encoding:"utf8"}).stdout.trim();
  const expected = {source_sha:sourceSha,run_id:"42",attempt:1,platform:process.platform,arch:process.arch,
    checkout_path_sha256:hash(path.resolve(root).replaceAll("\\", "/"))};
  writeFileSync(path.join(dir, "prodex-app-tests"), payload, {mode:0o700});
  const manifest = JSON.stringify({schema:1,package:"prodex-app",...expected,binary:"prodex-app-tests",
    binary_sha256:hash(payload),inventory:testInventory(testNames.map(n=>n+": test").join("\n"))});
  writeFileSync(path.join(dir, "manifest.json"),manifest);
  const bin = path.join(dir, "bin");mkdirSync(bin);
  writeFileSync(path.join(bin, "cargo"), "#!/bin/sh\necho unexpected-cargo >&2\nexit 91\n",{mode:0o700});
  return {dir,calls,env:{...process.env,PATH:bin+path.delimiter+process.env.PATH,
    GITHUB_SHA:sourceSha,GITHUB_RUN_ID:"42",GITHUB_RUN_ATTEMPT:"1",
    TEST_MANIFEST_SHA256:hash(manifest),PRODEX_TEST_BUNDLE:dir}};
}
function run(file,args,env) {
  return spawnSync(process.execPath,["scripts/ci/"+file,...args],{cwd:root,env,encoding:"utf8",timeout:15000});
}
const unixOnly={skip:process.platform==="win32"};
test("weighted core uses the verified census, preserves each owner and Cargo package cwd",unixOnly,(t)=>{
  const f=fixture(t);
  for (let index=0;index<2;index++) {
    const r=run("main-internal-core-shard.mjs",["--shard-count","2","--shard-index",String(index)],f.env);
    assert.equal(r.status,0,r.stdout+r.stderr);assert.doesNotMatch(r.stderr,/unexpected-cargo/);
  }
  const calls=readFileSync(f.calls,"utf8").trim().split("\n").map(JSON.parse);
  assert.deepEqual(calls.flatMap(c=>c.selected).sort(),names.filter(n=>n!==runtimeTest));
  assert.ok(calls.every(c=>c.cwd===path.join(root,"crates/prodex-app")&&c.args.includes("--test-threads=1")));
});
for (const [mode,filter,expected] of [["ok",runtimeTest,0],["fail",runtimeTest,7],["ok","absent-fixture",1],["timeout",runtimeTest,124]]) {
  test(`native runtime diagnostics retain ${mode} / ${expected} without rebuilding`,unixOnly,(t)=>{
    const f=fixture(t);const artifacts=path.join(f.dir,"diagnostics");
    const r=run("runtime-proxy-shard.mjs",["--filter",filter,"--artifact-dir",artifacts,
      "--timeout-minutes",mode==="timeout"?"0.005":"1"],{...f.env,NATIVE_FIXTURE_MODE:mode});
    assert.equal(r.status,expected,r.stdout+r.stderr);
    assert.doesNotMatch(r.stdout+r.stderr,/unexpected-cargo/);
    if(expected) assert.ok(existsSync(path.join(artifacts,"diagnostics.txt")),r.stdout+r.stderr);
    assert.equal(JSON.parse(readFileSync(f.calls,"utf8").trim()).cwd,path.join(root,"crates/prodex-app"));
  });
}
test("bad native bundle cannot execute or silently fall back to Cargo",unixOnly,(t)=>{
  const f=fixture(t);
  for(const file of ["runtime-proxy-shard.mjs","main-internal-core-shard.mjs"]) {
    const args=file.startsWith("runtime")?["--artifact-dir",path.join(f.dir,"bad")]:[];
    const r=run(file,args,{...f.env,TEST_MANIFEST_SHA256:"0".repeat(64)});
    assert.notEqual(r.status,0);assert.match(r.stderr,/manifest differs/);
    assert.doesNotMatch(r.stderr,/unexpected-cargo/);assert.ok(!existsSync(f.calls));
  }
});
test("every internal Linux consumer is bound to its producer and cannot recompile",()=>{
  const source=readFileSync(".github/workflows/ci.yml","utf8");
  assert.deepEqual(validateLinuxNativeTests(source),[]);
  for(const name of ["profile-commands-internal","main-internal-core","main-internal-runtime-proxy","runtime-stress"]) {
    const start=source.indexOf(`\n  ${name}:`);
    const next=source.slice(start+1).search(/\n  [a-z][a-z0-9_-]*:/u);
    const body=source.slice(start,next<0?source.length:start+1+next);
    for (const marker of ["needs: [changes, linux-app-test-build]","name: linux-app-tests",
      "TEST_MANIFEST_SHA256: ${{ needs.linux-app-test-build.outputs.manifest_sha256 }}"]) {
      assert.ok(validateLinuxNativeTests(source.replace(body,body.replace(marker,"removed"))).length>0);
    }
    assert.doesNotMatch(body,/cargo test|rust-cache@|sccache-action@/);
  }
});

for (const [suite, manifest, repetitions] of [
  ["serialized", RUNTIME_STRESS_SERIALIZED_TESTS, 1],
  ["continuation", RUNTIME_STRESS_CONTINUATION_TESTS, 2],
]) {
  test(`native ${suite} stress preserves all filters, repetitions and process isolation`, unixOnly, (t) => {
    const f = fixture(t, manifest);
    for (let shard = 0; shard < 2; shard++) {
      const r = run("runtime-stress.mjs", ["--suite", suite, "--shard-count", "2", "--shard-index", String(shard)], f.env);
      assert.equal(r.status, 0, r.stdout + r.stderr);
      assert.doesNotMatch(r.stdout + r.stderr, /unexpected-cargo/);
    }
    const calls = readFileSync(f.calls, "utf8").trim().split("\n").map(JSON.parse);
    assert.equal(calls.length, manifest.length * repetitions);
    for (const filter of manifest) assert.equal(calls.filter(c => c.args[0] === filter).length, repetitions);
    assert.ok(calls.every(c => c.cwd === path.join(root, "crates/prodex-app") && c.args.includes("--test-threads=1")));
  });
}
test("native broad stress partitions never select unrelated core tests", unixOnly, (t) => {
  const runtime = [0, 1, 2, 3].map(i => `main_internal_tests::runtime_proxy_synthetic::case_${i}`);
  const f = fixture(t, [...runtime, "main_internal_tests::unrelated::case"]);
  for (const strategy of ["weighted", "modulo"]) {
    for (let shard = 0; shard < 2; shard++) {
      const r = run("runtime-stress.mjs", ["--suite", "stress", "--shard-count", "2", "--shard-index", String(shard),
        "--shard-strategy", strategy], f.env);
      assert.equal(r.status, 0, r.stdout + r.stderr);
    }
  }
  const calls = readFileSync(f.calls, "utf8").trim().split("\n").map(JSON.parse);
  for (let start = 0; start < 4; start += 2) assert.deepEqual(calls.slice(start, start + 2).flatMap(c => c.selected).sort(), runtime);
});
test("stress propagates one failure without replaying the suite or compiling a fallback", unixOnly, (t) => {
  const f = fixture(t, RUNTIME_STRESS_SERIALIZED_TESTS);
  const r = run("runtime-stress.mjs", ["--suite", "serialized"], {...f.env, NATIVE_FIXTURE_MODE:"fail"});
  assert.notEqual(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stderr, /exited with code 7/);
  assert.equal(readFileSync(f.calls, "utf8").trim().split("\n").length, 1);
  assert.doesNotMatch(r.stdout + r.stderr, /retrying|unexpected-cargo/);
});
test("stress refuses a damaged bundle before test execution", unixOnly, (t) => {
  const f = fixture(t);
  const r = run("runtime-stress.mjs", ["--suite", "all"], {...f.env, TEST_MANIFEST_SHA256:"0".repeat(64)});
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /manifest differs/);
  assert.ok(!existsSync(f.calls));
});
test("scheduled stress has an eligible producer and absolute diagnostics", () => {
  const ci = readFileSync(".github/workflows/ci.yml", "utf8");
  const producer = ci.match(/\n  linux-app-test-build:\n([\s\S]*?)\n  prodex-app-lib:/u)[1];
  const stress = ci.match(/\n  runtime-stress:\n([\s\S]*?)\n  real-mojo:/u)[1];
  assert.doesNotMatch(producer, /github.event_name != 'schedule'/);
  assert.match(stress, /PRODEX_RUNTIME_LOG_DIR: \$\{\{ github.workspace \}\}\/target\//u);
  assert.match(stress, /Upload runtime stress diagnostics/);
});
