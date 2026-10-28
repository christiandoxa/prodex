#!/usr/bin/env node
// Integration tests embed CARGO_BIN_EXE_prodex: preserve that exact checkout path.
import assert from "node:assert/strict";
import { constants, appendFileSync, chmodSync, copyFileSync, lstatSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { command, identity, sha256, testInventory, validateRunIdentity } from "./native-test-artifact.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const suffix = (platform) => platform === "win32" ? ".exe" : "";
const names = (platform) => [`auto-rotate-tests${suffix(platform)}`, `prodex${suffix(platform)}`];

export function selectIntegrationArtifacts(output, repository, platform = process.platform) {
  const entries = output.split(/\r?\n/u).filter(Boolean).map((line) => JSON.parse(line));
  const select = (name, kind, test) => {
    const matches = entries.filter((entry) => entry.reason === "compiler-artifact" &&
      entry.target?.name === name && entry.target?.kind?.includes(kind) &&
      entry.profile?.test === test && typeof entry.executable === "string");
    assert.equal(matches.length, 1, `one ${name} ${kind} executable required`);
    return matches[0].executable;
  };
  const harness = select("auto_rotate", "test", true);
  const product = select("prodex", "bin", false);
  assert.equal(path.resolve(product), path.resolve(repository, "target/debug", `prodex${suffix(platform)}`),
    "integration bundle requires the default checkout-local Cargo target directory");
  return { harness, product };
}

function regularBytes(file) {
  const stat = lstatSync(file);
  assert.ok(stat.isFile() && !stat.isSymbolicLink(), "bundle member must be a regular file");
  const bytes = readFileSync(file);
  assert.ok(bytes.length > 0, "bundle executable must not be empty");
  return bytes;
}

export function packIntegrationBundle(directory, env = process.env, repository = root) {
  const expected = identity(env, repository);
  assert.ok(!env.CARGO_BUILD_TARGET, "host-native integration compilation required");
  if (env.CARGO_TARGET_DIR) assert.equal(path.resolve(repository, env.CARGO_TARGET_DIR),
    path.resolve(repository, "target"), "integration compilation must use checkout-local target");
  const output = command("cargo", ["test", "--locked", "-p", "prodex", "--test", "auto_rotate", "--no-run",
    "--message-format=json-render-diagnostics"], repository, {stdio:["ignore","pipe","inherit"],env});
  const { harness, product } = selectIntegrationArtifacts(output, repository);
  const files = names(expected.platform).map((name, index) => {
    const source = index === 0 ? harness : product;
    return {name, source, bytes:regularBytes(source)};
  });
  const inventory = testInventory(command(harness, ["--list", "--format", "terse"], repository));
  const manifest = {schema:1,package:"prodex",target:"auto_rotate",features:[],...expected,inventory,
    files:files.map(({name,bytes}) => ({name,sha256:sha256(bytes)}))};
  mkdirSync(directory,{recursive:true});
  for(const {name,source} of files) copyFileSync(source,path.join(directory,name));
  const manifestBytes = JSON.stringify(manifest,null,2)+"\n";
  writeFileSync(path.join(directory,"manifest.json"),manifestBytes);
  assert.ok(env.GITHUB_OUTPUT,"producer output file required");
  appendFileSync(env.GITHUB_OUTPUT,`manifest_sha256=${sha256(manifestBytes)}\n`);
  process.stdout.write(`Packed ${inventory.count} integration tests and their exact CLI binary\n`);
}

export function restoreIntegrationBundle(directory, expected, manifestDigest, repository = root) {
  assert.match(manifestDigest ?? "",/^[a-f0-9]{64}$/u,"independent integration manifest digest required");
  const bytes = regularBytes(path.join(directory,"manifest.json"));
  assert.equal(sha256(bytes),manifestDigest,"integration manifest differs from producer output");
  const manifest = JSON.parse(bytes);
  assert.equal(manifest.schema,1);
  assert.equal(manifest.package,"prodex");
  assert.equal(manifest.target,"auto_rotate");
  assert.deepEqual(manifest.features,[],"integration bundle is a default-feature build");
  validateRunIdentity(manifest,expected);
  assert.deepEqual(manifest.files.map((file)=>file.name),names(expected.platform),"unexpected integration payload paths");
  const payloads = manifest.files.map((file)=>{
    const location = path.join(directory,file.name);
    const payload = regularBytes(location);
    assert.equal(sha256(payload),file.sha256,`integration checksum mismatch: ${file.name}`);
    return {location,payload};
  });
  // Never write through a pre-existing symlink/junction or replace a different binary.
  let destination = path.resolve(repository);
  for(const part of ["target","debug"]) {
    destination=path.join(destination,part);
    if(!lstatSync(destination,{throwIfNoEntry:false})) mkdirSync(destination);
    const stat=lstatSync(destination);
    assert.ok(stat.isDirectory()&&!stat.isSymbolicLink(),"integration target must be a real directory");
  }
  destination=path.join(destination,names(expected.platform)[1]);
  if(lstatSync(destination,{throwIfNoEntry:false})) assert.equal(sha256(regularBytes(destination)),manifest.files[1].sha256,
    "refusing to replace a different integration product binary");
  else copyFileSync(payloads[1].location,destination,constants.COPYFILE_EXCL);
  const executable=path.resolve(payloads[0].location);
  if(process.platform!=="win32") {chmodSync(executable,0o700);chmodSync(destination,0o700);}
  const inventory=testInventory(command(executable,["--list","--format","terse"],repository));
  assert.deepEqual(inventory,manifest.inventory,"integration test inventory changed");
  return {executable,cwd:repository,inventory,product:destination};
}

export function environmentIntegrationBundle(env = process.env) {
  if(env.PRODEX_INTEGRATION_BUNDLE===undefined) return null;
  assert.ok(env.PRODEX_INTEGRATION_BUNDLE.trim(),"integration bundle directory required");
  return restoreIntegrationBundle(path.resolve(root,env.PRODEX_INTEGRATION_BUNDLE),identity(env,root),
    env.INTEGRATION_MANIFEST_SHA256);
}

if(import.meta.url===pathToFileURL(process.argv[1]??"").href) {
  try {
    const [mode,bundle,...extra]=process.argv.slice(2);
    assert.ok(["pack","restore"].includes(mode)&&bundle&&extra.length===0,
      "usage: node scripts/ci/integration-test-artifact.mjs pack|restore DIRECTORY");
    const directory=path.resolve(root,bundle);
    if(mode==="pack") packIntegrationBundle(directory);
    else restoreIntegrationBundle(directory,identity(process.env,root),process.env.INTEGRATION_MANIFEST_SHA256);
  } catch(error) {
    process.stderr.write(`integration-test-artifact: ${error.message}\n`);process.exitCode=1;
  }
}
