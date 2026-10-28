#!/usr/bin/env node
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Preserve the deployment floors of the qualified 0.165.0 public binaries.
const TARGETS = {
  "x86_64-apple-darwin": { cpu: 0x01000007, maximum: 0x000a0c00 },
  "aarch64-apple-darwin": { cpu: 0x0100000c, maximum: 0x000b0000 },
};
export function verifyMacosDeployment(bytes, target) {
  const expected = TARGETS[target];
  assert.ok(expected, "unsupported macOS target");
  assert.ok(bytes.length >= 32, "truncated Mach-O header");
  assert.equal(bytes.readUInt32LE(0), 0xfeedfacf, "expected thin little-endian Mach-O64");
  assert.equal(bytes.readUInt32LE(4), expected.cpu, "Mach-O target CPU mismatch");
  const commands = bytes.readUInt32LE(16);
  const end = 32 + bytes.readUInt32LE(20);
  assert.ok(end <= bytes.length && commands <= (end - 32) / 8, "invalid load-command bounds");
  let offset = 32;
  const versions = [];
  for (let index = 0; index < commands; index++) {
    assert.ok(offset + 8 <= end, "truncated load command");
    const command = bytes.readUInt32LE(offset);
    const size = bytes.readUInt32LE(offset + 4);
    assert.ok(size >= 8 && offset + size <= end, "invalid load-command size");
    if (command === 0x32) {
      assert.ok(size >= 24, "truncated build version");
      assert.equal(bytes.readUInt32LE(offset + 8), 1, "non-macOS build platform");
      versions.push(bytes.readUInt32LE(offset + 12));
    } else if (command === 0x24) {
      assert.ok(size >= 16, "truncated minimum version");
      versions.push(bytes.readUInt32LE(offset + 8));
    }
    offset += size;
  }
  assert.equal(offset, end, "load-command size mismatch");
  assert.equal(versions.length, 1, "exactly one macOS deployment version required");
  const minimum = versions[0];
  assert.ok(minimum > 0 && minimum <= expected.maximum, "macOS minimum deployment target increased");
  return [minimum >>> 16, (minimum >>> 8) & 255, minimum & 255].join(".");
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    assert.equal(process.argv.length, 4, "usage: macos-deployment-guard.mjs TARGET BINARY");
    const minimum = verifyMacosDeployment(readFileSync(process.argv[3]), process.argv[2]);
    console.log(`${process.argv[2]}: verified macOS minimum ${minimum}`);
  } catch (error) {
    console.error(`macos-deployment-guard: ${error.message}`);
    process.exitCode = 1;
  }
}
