#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const baseSha = "3d2ee51ca2d5db578f328aa75e20aa22c0197c9a";
const patchPath = path.join(repoRoot, "migration/codex-rust-v0.153.4-tui-queued-input.patch");
const expectedSha256 = "6c2dd2dae167c687bc2870082815a62f1c191e34f5b323f98ea442abfd11859b";

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
if (patchSha256 !== expectedSha256) {
  fail(`patch checksum mismatch: expected ${expectedSha256}, found ${patchSha256}`);
}
const sourceSha = command(["-C", args.source, "rev-parse", "HEAD"]);
if (sourceSha !== baseSha) {
  fail(`Codex source must be ${baseSha}, found ${sourceSha}`);
}
const checkArgs = ["-C", args.source, "apply", "--check", "-p1", patchPath];
command(checkArgs);
if (args.apply) {
  command(["-C", args.source, "apply", "-p1", patchPath]);
  process.stdout.write(`applied Codex TUI patch at ${baseSha}\n`);
} else {
  process.stdout.write(`Codex TUI patch applies cleanly at ${baseSha}\n`);
}
