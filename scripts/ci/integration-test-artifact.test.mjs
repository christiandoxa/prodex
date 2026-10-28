import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { identity, sha256, testInventory } from "./native-test-artifact.mjs";
import { restoreIntegrationBundle, selectIntegrationArtifacts } from "./integration-test-artifact.mjs";
import { validateWindowsIntegrationTests } from "./supply-chain-guard.mjs";

const sourceRoot=process.cwd();
const unix={skip:process.platform==="win32"};
function fixture(t) {
  const directory=mkdtempSync(path.join(os.tmpdir(),"prodex-integration-bundle-"));
  t.after(()=>rmSync(directory,{recursive:true,force:true}));
  const repository=path.join(directory,"repo");mkdirSync(repository);
  const git=(args)=>{const r=spawnSync("git",args,{cwd:repository,encoding:"utf8"});assert.equal(r.status,0,r.stderr);return r.stdout.trim();};
  git(["init","-q"]);writeFileSync(path.join(repository,"fixture.txt"),"synthetic\n");git(["add","fixture.txt"]);
  git(["-c","user.name=Fixture","-c","user.email=fixture@example.com","-c","commit.gpgsign=false","commit","-qm","fixture"]);
  const scripts=path.join(repository,"scripts/ci");mkdirSync(scripts,{recursive:true});
  for(const name of ["native-test-artifact.mjs","integration-test-artifact.mjs","auto-rotate-shards.mjs","main-internal-test-runner.mjs","guard-common.mjs"])
    copyFileSync(path.join(sourceRoot,"scripts/ci",name),path.join(scripts,name));
  const env={...process.env,GITHUB_SHA:git(["rev-parse","HEAD"]),GITHUB_RUN_ID:"42",GITHUB_RUN_ATTEMPT:"2"};
  delete env.PRODEX_INTEGRATION_BUNDLE;delete env.INTEGRATION_MANIFEST_SHA256;
  const expected=identity(env,repository);
  const bundle=path.join(directory,"bundle");mkdirSync(bundle);
  const calls=path.join(directory,"calls.jsonl");
  const product=Buffer.from("synthetic CLI bytes\n");
  const payload=Buffer.from(`#!${process.execPath}
import fs from 'node:fs';
import path from 'node:path';
const args=process.argv.slice(2);
if(args.includes('--list')){console.log('a::case: test\\nb::case: test');process.exit(0);}
if(fs.readFileSync(path.join(process.cwd(),'target/debug/prodex'),'utf8')!==${JSON.stringify(product.toString())})process.exit(19);
fs.appendFileSync(${JSON.stringify(calls)},JSON.stringify({cwd:process.cwd(),args})+'\\n');
if(process.env.INTEGRATION_FIXTURE_FAIL==='1'){console.error('synthetic assertion failure');process.exit(7);}
console.log('running 1 test\\ntest result: ok. 1 passed; 0 failed; 0 ignored');
`.replaceAll("\\\\n","\\n"));
  writeFileSync(path.join(bundle,"auto-rotate-tests"),payload,{mode:0o700});writeFileSync(path.join(bundle,"prodex"),product);
  const manifest={schema:1,package:"prodex",target:"auto_rotate",features:[],...expected,attempt:1,
    inventory:testInventory("a::case: test\nb::case: test"),files:[{name:"auto-rotate-tests",sha256:sha256(payload)},{name:"prodex",sha256:sha256(product)}]};
  const save=()=>{const text=JSON.stringify(manifest);writeFileSync(path.join(bundle,"manifest.json"),text);return sha256(text);};
  const digest=save();
  return {repository,bundle,calls,expected,manifest,save,digest,product,
    env:{...env,PRODEX_INTEGRATION_BUNDLE:bundle,INTEGRATION_MANIFEST_SHA256:digest}};
}

test("Cargo selection binds the production CLI to its embedded checkout path",()=>{
  const repository=path.resolve("synthetic");
  const entries=[{reason:"compiler-artifact",target:{name:"auto_rotate",kind:["test"]},profile:{test:true},executable:"fixture-harness"},
    {reason:"compiler-artifact",target:{name:"prodex",kind:["bin"]},profile:{test:false},executable:path.join(repository,"target/debug/prodex")}];
  const output=(items)=>items.map(JSON.stringify).join("\n");
  assert.equal(selectIntegrationArtifacts(output(entries),repository,"linux").harness,"fixture-harness");
  assert.throws(()=>selectIntegrationArtifacts(output([...entries,entries[0]]),repository,"linux"));
  assert.throws(()=>selectIntegrationArtifacts(output([entries[0]]),repository,"linux"));
  assert.throws(()=>selectIntegrationArtifacts(output([entries[0],{...entries[1],executable:"/foreign/prodex"}]),repository,"linux"));
});

test("integration restoration verifies both executable hashes and same-run identity",unix,(t)=>{
  const f=fixture(t);
  for(const [key,value] of Object.entries({source_sha:"b".repeat(40),run_id:"43",platform:"other",arch:"other",checkout_path_sha256:"0".repeat(64),attempt:0}))
    assert.throws(()=>restoreIntegrationBundle(f.bundle,{...f.expected,[key]:value},f.digest,f.repository),key);
  assert.throws(()=>restoreIntegrationBundle(f.bundle,f.expected,"0".repeat(64),f.repository));
  assert.ok(!existsSync(path.join(f.repository,"target")),"invalid identity must not materialize a binary");
  const got=restoreIntegrationBundle(f.bundle,f.expected,f.digest,f.repository);
  assert.deepEqual(readFileSync(got.product),f.product);
  assert.deepEqual(got.inventory,f.manifest.inventory);
  assert.equal(got.cwd,f.repository);
  assert.deepEqual(restoreIntegrationBundle(f.bundle,f.expected,f.digest,f.repository),got,"same-source restore is idempotent");
});

test("corrupt product bytes, untrusted names and different features fail closed",unix,(t)=>{
  const f=fixture(t);writeFileSync(path.join(f.bundle,"prodex"),"corrupt");
  assert.throws(()=>restoreIntegrationBundle(f.bundle,f.expected,f.digest,f.repository),/checksum/);
  writeFileSync(path.join(f.bundle,"prodex"),f.product);
  f.manifest.files[1].name="../outside";
  assert.throws(()=>restoreIntegrationBundle(f.bundle,f.expected,f.save(),f.repository),/payload paths/);
  f.manifest.files[1].name="prodex";f.manifest.features=["all"];
  assert.throws(()=>restoreIntegrationBundle(f.bundle,f.expected,f.save(),f.repository),/default-feature/);
});

test("bundle and destination symlinks cannot redirect product restoration",unix,(t)=>{
  const f=fixture(t);const outside=path.join(f.repository,"outside");mkdirSync(outside);
  symlinkSync(outside,path.join(f.repository,"target"));
  assert.throws(()=>restoreIntegrationBundle(f.bundle,f.expected,f.digest,f.repository),/real directory/);
  assert.deepEqual(readFileSync(path.join(f.bundle,"prodex")),f.product);
  rmSync(path.join(f.repository,"target"));
  rmSync(path.join(f.bundle,"prodex"));symlinkSync(path.join(f.bundle,"absent"),path.join(f.bundle,"prodex"));
  assert.throws(()=>restoreIntegrationBundle(f.bundle,f.expected,f.digest,f.repository),/regular file/);
});

test("auto-rotate executes the verified pair without Cargo and preserves every selected filter",unix,(t)=>{
  const f=fixture(t);const fake=path.join(f.repository,"fake-bin");mkdirSync(fake);
  writeFileSync(path.join(fake,"cargo"),"#!/bin/sh\necho unexpected-cargo >&2\nexit 91\n",{mode:0o700});
  const env={...f.env,PATH:fake+path.delimiter+process.env.PATH};
  const args=["scripts/ci/auto-rotate-shards.mjs","--jobs","1","--shard-index","0","--shard-count","2"];
  const baselineEnv={...env};delete baselineEnv.PRODEX_INTEGRATION_BUNDLE;
  const planned=spawnSync(process.execPath,[...args,"--dry-run"],{cwd:f.repository,env:baselineEnv,encoding:"utf8",timeout:10000});
  assert.equal(planned.status,0,planned.stderr);
  const filters=[...planned.stdout.matchAll(/--test auto_rotate (\S+) -- --test-threads=1/g)].map(m=>m[1]);
  const actual=spawnSync(process.execPath,args,{cwd:f.repository,env,encoding:"utf8",timeout:15000});
  assert.equal(actual.status,0,actual.stdout+actual.stderr);assert.doesNotMatch(actual.stdout+actual.stderr,/unexpected-cargo/);
  const calls=readFileSync(f.calls,"utf8").trim().split("\n").map(JSON.parse);
  assert.deepEqual(calls.map(c=>c.args),filters.map(filter=>[filter,"--test-threads=1"]));
  assert.ok(calls.every(c=>c.cwd===f.repository));
  const failure=spawnSync(process.execPath,args,{cwd:f.repository,env:{...env,INTEGRATION_FIXTURE_FAIL:"1"},encoding:"utf8",timeout:15000});
  assert.notEqual(failure.status,0);assert.match(failure.stderr,/exited with code 7/);
  const damaged=spawnSync(process.execPath,args,{cwd:f.repository,env:{...env,INTEGRATION_MANIFEST_SHA256:"0".repeat(64)},encoding:"utf8",timeout:15000});
  assert.notEqual(damaged.status,0);assert.match(damaged.stderr,/manifest differs/);
});

test("Windows workflow refuses missing source bindings, unchecked installer bytes and recompilation",()=>{
  const workflow=readFileSync(".github/workflows/ci.yml","utf8");assert.deepEqual(validateWindowsIntegrationTests(workflow),[]);
  for(const marker of ["integration-test-artifact.mjs pack","INTEGRATION_MANIFEST_SHA256: ${{ needs.windows-integration-build.outputs.manifest_sha256 }}",
    "integration-test-artifact.mjs restore","needs: [changes, windows-integration-build]","node --test scripts/install.test.mjs"])
    assert.ok(validateWindowsIntegrationTests(workflow.replace(marker,"removed")).length>0,marker);
  assert.ok(validateWindowsIntegrationTests(workflow.replace("  windows-integration:\n","  windows-integration:\n    run: cargo build\n")).length>0);
});
