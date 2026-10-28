import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync, existsSync } from "node:fs";
import path from "node:path";
import os from "node:os";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { validateReleaseContainerPublication } from "./supply-chain-guard.mjs";

const target = readFileSync(".github/workflows/release-target.yml", "utf8");
const parent = readFileSync(".github/workflows/standalone-release.yml", "utf8");
const step = target.split("      - name: Install pinned native clients\n")[1].split("\n      - name:")[0];
const shell = step.split("        run: |\n")[1].replace(/^          /gmu, "");
const pins = {
  kiro: "d8dd659c11c156002942f09443b6cb7904959004d4e7f1f73777fdbb2d909b54",
  copilot: "00efcc2e196f0793d097935d6230062fc66e8247dc01334d2ac3eae4e26a1c7f",
  claude: "3a68d3406cf674e17bed1733a4dcf37805e2e47d87417700007d7e1aa766a944",
};

test("release client setup is bounded, pinned, credential-free, and fail-closed", () => {
  assert.match(step, /timeout-minutes: 10/);
  assert.match(shell, /clients=\(gemini kiro copilot claude\)/);
  assert.match(shell, /if ! wait "\$\{pids\[index\]\}"/);
  assert.match(shell, /test "\$\{status\}" -eq 0/);
  assert.match(shell, /trap cleanup EXIT/);
  assert.ok(shell.indexOf("unset GITHUB_TOKEN") < shell.indexOf('pids=()'));
  assert.ok(shell.indexOf('test "${status}" -eq 0') < shell.indexOf('>> "${GITHUB_PATH}"'));
  assert.deepEqual(validateReleaseContainerPublication(parent, target), []);
  for (const digest of Object.values(pins)) {
    assert.ok(validateReleaseContainerPublication(parent, target.replace(digest, "0".repeat(64))).length > 0);
  }
});

for (const scenario of ["parallel", "download-failure", "checksum-failure", "installer-failure", "npm-failure"]) {
  test(`real release shell preserves validation and all-worker completion: ${scenario}`, {skip:process.platform === "win32"}, t => {
    const dir = mkdtempSync(path.join(os.tmpdir(), "prodex-client-setup-"));
    t.after(() => rmSync(dir, {recursive:true, force:true}));
    for (const name of ["bin", "tmp", "home"]) mkdirSync(path.join(dir, name));
    // Peers wait for each other only in the concurrency proof. A sequential
    // implementation cannot pass the barrier; failures have no timing oracle.
    const worker = `#!${process.execPath}
import fs from 'node:fs';import path from 'node:path';
const dir=${JSON.stringify(dir)},scenario=${JSON.stringify(scenario)};
const id=process.argv[2],args=process.argv.slice(3);
for (const key of ['GITHUB_TOKEN','GH_TOKEN','ACTIONS_ID_TOKEN_REQUEST_TOKEN','ACTIONS_RUNTIME_TOKEN']) if(process.env[key])process.exit(92);
if(id!=='gemini'&&!fs.existsSync(path.join(dir,'verified-'+id)))process.exit(93);
fs.writeFileSync(path.join(dir,'started-'+id),JSON.stringify({args,version:process.env.VERSION??null,prefix:process.env.PREFIX??null}));
if(scenario==='parallel') {const end=Date.now()+4000;while(!['gemini','kiro','copilot','claude'].every(n=>fs.existsSync(path.join(dir,'started-'+n)))) {if(Date.now()>end)process.exit(94);await new Promise(r=>setTimeout(r,10));}}
fs.writeFileSync(path.join(dir,'finished-'+id),'done');
process.exit((scenario==='installer-failure'&&id==='kiro')||(scenario==='npm-failure'&&id==='gemini')?17:0);
`;
    writeFileSync(path.join(dir, "worker.mjs"), worker);
    const workerPath = JSON.stringify(path.join(dir, "worker.mjs"));
    writeFileSync(path.join(dir, "bin/npm"), `#!/bin/sh\nexec '${process.execPath}' ${workerPath} gemini "$@"\n`, {mode:0o700});
    writeFileSync(path.join(dir, "bin/curl"), `#!${process.execPath}
import fs from 'node:fs';import path from 'node:path';
const args=process.argv.slice(2),out=args[args.indexOf('-fsSLo')+1];
const id=path.basename(out).split('-')[0];
if(!args.includes('--proto')||!args.includes('=https')||!args.includes('--tlsv1.2'))process.exit(95);
if(${JSON.stringify(scenario)}==='download-failure'&&id==='kiro')process.exit(18);
fs.writeFileSync(out,'#!/bin/sh\\nexec '+${JSON.stringify("'"+process.execPath+"'")}+' '+${JSON.stringify(workerPath)}+' '+id+' "$@"\\n');
`, {mode:0o700});
    writeFileSync(path.join(dir, "bin/sha256sum"), `#!${process.execPath}
import fs from 'node:fs';import path from 'node:path';
const input=fs.readFileSync(0,'utf8').trim(),parts=input.split(/\\s+/),id=path.basename(parts[1]).split('-')[0];
if(!process.argv.includes('--check')||!process.argv.includes('--strict')||parts[0]!==${JSON.stringify(pins)}[id])process.exit(96);
if(${JSON.stringify(scenario)}==='checksum-failure'&&id==='kiro')process.exit(19);
fs.writeFileSync(path.join(${JSON.stringify(dir)},'verified-'+id),'checked');
`, {mode:0o700});
    const env = {...process.env, PATH:path.join(dir,"bin")+path.delimiter+process.env.PATH,
      HOME:path.join(dir,"home"),RUNNER_TEMP:path.join(dir,"tmp"),GITHUB_PATH:path.join(dir,"github-path"),
      GITHUB_TOKEN:"synthetic-ci-value",GH_TOKEN:"synthetic-ci-value",ACTIONS_ID_TOKEN_REQUEST_TOKEN:"synthetic-ci-value",ACTIONS_RUNTIME_TOKEN:"synthetic-ci-value"};
    const result = spawnSync("bash", ["-c", shell], {env, encoding:"utf8", timeout:10000});
    assert.equal(result.status, scenario === "parallel" ? 0 : 1, result.stdout + result.stderr);
    for (const id of ["gemini", "copilot", "claude"]) assert.ok(existsSync(path.join(dir, "finished-"+id)), id);
    if (["download-failure", "checksum-failure"].includes(scenario)) assert.ok(!existsSync(path.join(dir,"started-kiro")));
    else assert.ok(existsSync(path.join(dir,"finished-kiro")));
    if (scenario !== "parallel") assert.ok(!existsSync(path.join(dir,"github-path")));
    else {
      const record = id => JSON.parse(readFileSync(path.join(dir,"started-"+id),"utf8"));
      assert.deepEqual(record("gemini").args,["install","--global","--ignore-scripts","@google/gemini-cli@0.53.0"]);
      assert.deepEqual(record("kiro").args,["--force","--channel","stable"]);
      assert.equal(record("copilot").version,"v1.0.77");
      assert.deepEqual(record("claude").args,["2.1.220"]);
      assert.equal(readFileSync(path.join(dir,"github-path"),"utf8").trim(),path.join(dir,"home/.local/bin"));
    }
  });
}
