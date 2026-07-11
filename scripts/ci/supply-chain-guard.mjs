#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const ACTION = /^\s*uses:\s*([^\s#]+)(?:\s+#\s*(\S+))?\s*$/gmu;
const CONTAINER = /\b((?:ghcr\.io|quay\.io|docker\.io)\/[a-z0-9._/-]+|anchore\/[a-z0-9._/-]+):([a-z0-9._-]+)(?:@sha256:([a-f0-9]{64}))?/giu;

export function validateWorkflow(filePath, contents) {
  const violations = [];
  let rustActions = 0;
  for (const match of contents.matchAll(ACTION)) {
    const [target, comment] = match.slice(1);
    if (target.startsWith("./")) continue;
    const ref = target.slice(target.lastIndexOf("@") + 1);
    if (!/^[0-9a-f]{40}$/u.test(ref)) {
      violations.push(`${filePath}: third-party action is not pinned to a full commit SHA: ${target}`);
    } else if (!comment || /^[0-9a-f]{40}$/u.test(comment)) {
      violations.push(`${filePath}: pinned action must retain its readable tag comment: ${target}`);
    }
    if (target.startsWith("dtolnay/rust-toolchain@")) rustActions += 1;
  }
  const exactToolchains = contents.match(/^\s*toolchain:\s*1\.97\.0\s*$/gmu)?.length ?? 0;
  if (rustActions !== exactToolchains) {
    violations.push(`${filePath}: every rust-toolchain action must install exact toolchain 1.97.0`);
  }
  for (const match of contents.matchAll(CONTAINER)) {
    if (!match[3]) {
      violations.push(`${filePath}: CI container is not digest-pinned: ${match[0]}`);
    }
  }
  for (const line of contents.split(/\r?\n/u)) {
    if (/\b(?:cargo|cross)\s+(?:bench|build|check|clippy|run|test)\b/u.test(line) && !/--locked\b/u.test(line)) {
      violations.push(`${filePath}: Cargo graph command must use --locked: ${line.trim()}`);
    }
  }
  return violations;
}

export function validateDockerfile(contents) {
  const fromLines = contents.split(/\r?\n/u).filter((line) => /^FROM\s+/iu.test(line));
  return fromLines
    .filter((line) => !/^FROM\s+(?:--platform=\S+\s+)?\S+:[^@\s]+@sha256:[0-9a-f]{64}(?:\s+AS\s+\S+)?$/iu.test(line))
    .map((line) => `Dockerfile: base image is not tag-and-digest pinned: ${line}`);
}

export function validateCompose(contents) {
  return contents
    .split(/\r?\n/u)
    .filter((line) => /^\s*image:\s*/u.test(line) && !/:local\s*$/u.test(line))
    .filter((line) => !/^\s*image:\s*\S+:[^@\s]+@sha256:[0-9a-f]{64}\s*$/iu.test(line))
    .map((line) => `compose.yaml: service image is not tag-and-digest pinned: ${line.trim()}`);
}

function selfTest() {
  assert.deepEqual(
    validateWorkflow("safe.yml", "uses: owner/action@0123456789abcdef0123456789abcdef01234567 # v1\n"),
    [],
  );
  assert.equal(validateWorkflow("bad.yml", "uses: owner/action@v1\n").length, 1);
  assert.equal(
    validateWorkflow("bad.yml", "run: docker run ghcr.io/example/tool:v1 scan\n").length,
    1,
  );
  assert.equal(validateWorkflow("bad.yml", "run: cargo test --workspace\n").length, 1);
  assert.deepEqual(
    validateDockerfile("FROM rust:1.97.0@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef AS builder\n"),
    [],
  );
  assert.equal(validateDockerfile("FROM rust:latest\n").length, 1);
  assert.deepEqual(
    validateCompose("services:\n  db:\n    image: postgres:16@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n"),
    [],
  );
  assert.equal(validateCompose("services:\n  db:\n    image: postgres:16\n").length, 1);
}

async function main() {
  if (process.argv.includes("--self-test")) selfTest();
  const workflowDir = path.join(repoRoot, ".github", "workflows");
  const violations = [];
  for (const fileName of (await fs.readdir(workflowDir)).filter((name) => /\.ya?ml$/u.test(name)).sort()) {
    const filePath = `.github/workflows/${fileName}`;
    violations.push(...validateWorkflow(filePath, await fs.readFile(path.join(workflowDir, fileName), "utf8")));
  }
  violations.push(...validateDockerfile(await fs.readFile(path.join(repoRoot, "Dockerfile"), "utf8")));
  violations.push(...validateCompose(await fs.readFile(path.join(repoRoot, "compose.yaml"), "utf8")));

  const toolchain = await fs.readFile(path.join(repoRoot, "rust-toolchain.toml"), "utf8");
  for (const marker of ['channel = "1.97.0"', 'components = ["clippy", "rustfmt"]']) {
    if (!toolchain.includes(marker)) violations.push(`rust-toolchain.toml: missing ${marker}`);
  }
  await fs.access(path.join(repoRoot, "Cargo.lock"));

  if (violations.length === 0) {
    process.stdout.write("supply-chain guard: ok\n");
    return;
  }
  process.stderr.write(`supply-chain guard failed:\n  - ${violations.join("\n  - ")}\n`);
  process.exitCode = 1;
}

main().catch((error) => {
  process.stderr.write(`supply-chain-guard: ${error.message}\n`);
  process.exitCode = 1;
});
