import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { verifyMacosDeployment } from "./macos-deployment-guard.mjs";

function binary(cpu, version, legacy = false) {
  const size = legacy ? 16 : 24;
  const bytes = Buffer.alloc(32 + size);
  bytes.writeUInt32LE(0xfeedfacf, 0); bytes.writeUInt32LE(cpu, 4);
  bytes.writeUInt32LE(1, 16); bytes.writeUInt32LE(size, 20);
  bytes.writeUInt32LE(legacy ? 0x24 : 0x32, 32); bytes.writeUInt32LE(size, 36);
  if (legacy) bytes.writeUInt32LE(version, 40);
  else { bytes.writeUInt32LE(1, 40); bytes.writeUInt32LE(version, 44); }
  return bytes;
}
test("supported-host migration retains qualified Apple Silicon and Intel deployment floors", () => {
  assert.equal(verifyMacosDeployment(binary(0x0100000c, 0x000b0000), "aarch64-apple-darwin"), "11.0.0");
  assert.equal(verifyMacosDeployment(binary(0x01000007, 0x000a0c00, true), "x86_64-apple-darwin"), "10.12.0");
});
test("a newer build host cannot silently raise either minimum OS requirement", () => {
  for (const [cpu, target] of [[0x0100000c,"aarch64-apple-darwin"], [0x01000007,"x86_64-apple-darwin"]]) {
    assert.throws(() => verifyMacosDeployment(binary(cpu, 0x000f0000), target), /minimum deployment target increased/);
  }
});
test("wrong architecture, malformed commands, missing versions and non-macOS platforms fail closed", () => {
  const valid = binary(0x0100000c, 0x000b0000);
  const mutations = [Buffer.alloc(0), valid.subarray(0,35)];
  for (const [offset,value] of [[0,0],[4,0x01000007],[16,0],[20,999],[32,0],[36,0],[40,2],[44,0]]) {
    const copy = Buffer.from(valid); copy.writeUInt32LE(value,offset); mutations.push(copy);
  }
  for (const bytes of mutations) assert.throws(() => verifyMacosDeployment(bytes,"aarch64-apple-darwin"));
});
test("both actual scanned macOS assets pass the deployment gate before malware scanning", () => {
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const start = workflow.indexOf("- name: Verify unchanged macOS minimum deployment targets");
  const scan = workflow.indexOf("- name: Scan release assets for malware");
  assert.ok(start > 0 && scan > start);
  for (const target of ["x86_64-apple-darwin","aarch64-apple-darwin"]) {
    assert.ok(workflow.slice(start,scan).includes(`node scripts/ci/macos-deployment-guard.mjs ${target} release-assets/prodex-${target}`));
  }
});


test("production macOS compilation explicitly retains the previously published minimum versions", () => {
  const workflow = readFileSync(".github/workflows/release-target.yml", "utf8");
  const build = workflow.split("- name: Build prodex\n")[1].split("- name:")[0];
  assert.ok(build.includes("x86_64-apple-darwin) export MACOSX_DEPLOYMENT_TARGET=10.12"));
  assert.ok(build.includes("aarch64-apple-darwin) export MACOSX_DEPLOYMENT_TARGET=11.0"));
});
