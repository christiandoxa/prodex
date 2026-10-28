import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const source=readFileSync(".github/workflows/standalone-release.yml","utf8");
const job=source.match(/\n  verify-ci:\n([\s\S]*?)\n  antivirus-health:/)?.[1];
const block=job.slice(job.indexOf("          set -euo pipefail"));
const script=block.split("\n").map(line=>line.replace(/^          /,"")).join("\n");
const sha="a".repeat(40);
for(const scenario of ["success","pending","failure","foreign"]) {
  test(`exact-source release polling ${scenario}`,{skip:process.platform==="win32"},(t)=>{
    const dir=mkdtempSync(path.join(os.tmpdir(),"prodex-ci-poll-"));t.after(()=>rmSync(dir,{recursive:true,force:true}));
    writeFileSync(path.join(dir,"gh"),`#!${process.execPath}
import fs from 'node:fs';
const args=process.argv.slice(2);
if(args[args.indexOf('--commit')+1]!==${JSON.stringify(sha)})process.exit(9);
const callsFile=${JSON.stringify(path.join(dir,"calls"))};
const index=fs.existsSync(callsFile)?Number(fs.readFileSync(callsFile,'utf8')):0;
fs.writeFileSync(callsFile,String(index+1));
const scenario=${JSON.stringify(scenario)};
console.log(JSON.stringify([{headSha:scenario==='foreign'&&index===0?'b'.repeat(40):${JSON.stringify(sha)},status:scenario==='pending'&&index===0?'in_progress':'completed',conclusion:scenario==='failure'?'failure':'success',url:'https://example.com/run'}]));
`,{mode:0o700});
    writeFileSync(path.join(dir,"sleep"),`#!/bin/sh\nprintf '%s\\n' "$1" >> '${dir}/sleeps'\n`,{mode:0o700});
    const r=spawnSync("bash",["-c",script],{env:{...process.env,PATH:dir+path.delimiter+process.env.PATH,TARGET_SHA:sha},encoding:"utf8",timeout:10000});
    assert.equal(r.status,scenario==="failure"?1:0,r.stdout+r.stderr);
    if(["pending","foreign"].includes(scenario))assert.equal(readFileSync(path.join(dir,"sleeps"),"utf8"),"10\n");
    if(scenario==="failure")assert.match(r.stderr,/CI did not pass/);
    else assert.match(r.stdout,/CI success for/);
  });
}
