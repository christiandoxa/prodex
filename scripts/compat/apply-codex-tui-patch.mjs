#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const baseSha = "3d2ee51ca2d5db578f328aa75e20aa22c0197c9a";
const patchPath = path.join(repoRoot, "migration/codex-rust-v0.153.4-tui-queued-input.patch");
const expectedPatchSha256 = "6c2dd2dae167c687bc2870082815a62f1c191e34f5b323f98ea442abfd11859b";
const expectedUpstreamLockSha256 = "3494b8a78d0f643556a83a9cc184e912bcab9f4c5640288952f4223452ba5dc8";
const expectedPreparedLockSha256 = "a2cb91dfb2e8112bc81d05158fa00b9698e2df8cc1ae0547b5dc5606a44904d3";

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

function command(args) {
  const result = spawnSync("git", args, { encoding: "utf8" });
  if (result.status !== 0) {
    fail(result.stderr.trim() || `git ${args.join(" ")} failed`);
  }
  return result.stdout.trim();
}

function parseArgs(argv) {
  const sourceIndex = argv.indexOf("--source");
  if (sourceIndex < 0 || !argv[sourceIndex + 1]) {
    fail("usage: apply-codex-tui-patch.mjs --source <codex-source> [--apply]");
  }
  return {
    source: path.resolve(argv[sourceIndex + 1]),
    apply: argv.includes("--apply"),
  };
}

const args = parseArgs(process.argv.slice(2));
if (!fs.existsSync(patchPath)) fail(`missing patch: ${patchPath}`);
const patch = fs.readFileSync(patchPath);
const patchSha256 = crypto.createHash("sha256").update(patch).digest("hex");
if (patchSha256 !== expectedPatchSha256) {
  fail(`patch checksum mismatch: expected ${expectedPatchSha256}, found ${patchSha256}`);
}
const patchedPaths = patch
  .toString("utf8")
  .split(/\r?\n/u)
  .filter((line) => line.startsWith("diff --git a/"))
  .map((line) => line.split(" ", 3)[2]?.slice(2));
if (patchedPaths.some((file) => file?.endsWith("Cargo.toml") || file?.endsWith("Cargo.lock"))) {
  fail("Codex TUI patch must not change Cargo manifests or lockfiles");
}
const sourceSha = command(["-C", args.source, "rev-parse", "HEAD"]);
if (sourceSha !== baseSha) {
  fail(`Codex source must be ${baseSha}, found ${sourceSha}`);
}
const lockPath = path.join(args.source, "codex-rs", "Cargo.lock");
if (!fs.existsSync(lockPath)) fail(`missing Codex lockfile: ${lockPath}`);
const upstreamLock = fs.readFileSync(lockPath, "utf8");
const upstreamLockSha256 = crypto.createHash("sha256").update(upstreamLock).digest("hex");
if (upstreamLockSha256 !== expectedUpstreamLockSha256) {
  fail(
    `Codex upstream lock checksum mismatch: expected ${expectedUpstreamLockSha256}, found ${upstreamLockSha256}`,
  );
}
const workspaceVersionEntries = upstreamLock.match(/^version = "0\.0\.0"$/gmu) ?? [];
if (workspaceVersionEntries.length !== 149) {
  fail(`expected 149 Codex workspace lock entries, found ${workspaceVersionEntries.length}`);
}
// The release tag bumps workspace manifests but leaves local lock entries at 0.0.0.
// Normalize only those entries; the pinned external dependency graph stays unchanged.
const preparedLock = upstreamLock.replace(/^version = "0\.0\.0"$/gmu, 'version = "0.153.4"');
const preparedLockSha256 = crypto.createHash("sha256").update(preparedLock).digest("hex");
if (preparedLockSha256 !== expectedPreparedLockSha256) {
  fail(
    `Codex prepared lock checksum mismatch: expected ${expectedPreparedLockSha256}, found ${preparedLockSha256}`,
  );
}
const ramaVersions = [...preparedLock.matchAll(/name = "rama-(?:core|error|macros|utils)"\nversion = "([^"]+)"/gu)].map(
  (match) => match[1],
);
if (ramaVersions.length !== 4 || new Set(ramaVersions).size !== 1) {
  fail("Codex prepared lock has an inconsistent Rama dependency family");
}
const checkArgs = ["-C", args.source, "apply", "--check", "-p1", patchPath];
command(checkArgs);
if (args.apply) {
  command(["-C", args.source, "apply", "-p1", patchPath]);
  const manifestChanges = command(["-C", args.source, "diff", "--name-only"])
    .split(/\r?\n/u)
    .filter((file) => file.endsWith("Cargo.toml") || file.endsWith("Cargo.lock"));
  if (manifestChanges.length > 0) fail("applied Codex patch changed Cargo metadata");
  fs.writeFileSync(lockPath, preparedLock);
  process.stdout.write(`applied Codex TUI patch at ${baseSha}\n`);
} else {
  process.stdout.write(`Codex TUI patch applies cleanly at ${baseSha}\n`);
}
process.stdout.write(`Codex upstream lock SHA-256 ${upstreamLockSha256}\n`);
process.stdout.write(`Codex prepared lock SHA-256 ${preparedLockSha256}\n`);
process.stdout.write(`Codex Rama lock family ${ramaVersions[0]}\n`);
