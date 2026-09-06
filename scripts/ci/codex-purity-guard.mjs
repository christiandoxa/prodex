#!/usr/bin/env node

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const MAX_FILES = 200_000;
const MAX_JSON_BYTES = 16 * 1024 * 1024;
const MAX_ARCHIVE_LIST_BYTES = 32 * 1024 * 1024;

function forbiddenPackageName(value) {
  const name = String(value ?? "").trim().toLowerCase();
  return name === "codex" || name.startsWith("codex-") ||
    name === "@openai/codex" || name.startsWith("@openai/codex-");
}

function forbiddenArtifactPath(value) {
  const parts = String(value).replaceAll("\\", "/").split("/").filter(Boolean);
  if (parts.some((part) => /^codex(?:$|[._-])/iu.test(part))) return true;
  return parts.some(
    (part, index) => part.toLowerCase() === "@openai" &&
      /^codex(?:$|[._-])/iu.test(parts[index + 1] ?? ""),
  );
}

function inspectPackageJson(value, label, violations) {
  for (const section of [
    "dependencies",
    "optionalDependencies",
    "peerDependencies",
    "bundledDependencies",
    "bundleDependencies",
  ]) {
    const dependencies = value?.[section];
    const entries = Array.isArray(dependencies)
      ? dependencies.map((name) => [name, ""])
      : Object.entries(dependencies ?? {});
    for (const [name, specifier] of entries) {
      if (forbiddenPackageName(name) || /(?:npm:)?@openai\/codex(?:$|[-@])/iu.test(String(specifier))) {
        violations.push(`${label}: forbidden Codex runtime package in ${section}`);
      }
    }
  }
}

function inspectPackageLock(value, label, violations) {
  inspectPackageJson(value, label, violations);
  for (const [packagePath, entry] of Object.entries(value?.packages ?? {})) {
    if (forbiddenArtifactPath(packagePath) || forbiddenPackageName(entry?.name)) {
      violations.push(`${label}: forbidden Codex runtime package lock entry`);
    }
    inspectPackageJson(entry, `${label}:${packagePath || "root"}`, violations);
  }
}

export function validateCodexPackageLock(value, label = "package-lock.json") {
  const violations = [];
  inspectPackageLock(value, label, violations);
  return violations;
}

function inspectSbom(value, label, violations) {
  for (const entry of value?.packages ?? []) {
    if (forbiddenPackageName(entry?.name)) {
      violations.push(`${label}: forbidden Codex runtime package in SBOM`);
    }
    for (const reference of entry?.externalRefs ?? []) {
      const locator = String(reference?.referenceLocator ?? "");
      if (/pkg:npm\/(?:%40|@)openai(?:%2f|\/)codex(?:$|[-@])/iu.test(locator)) {
        violations.push(`${label}: forbidden Codex npm package reference in SBOM`);
      }
    }
  }
  for (const entry of value?.files ?? []) {
    if (forbiddenArtifactPath(entry?.fileName ?? "")) {
      violations.push(`${label}: forbidden Codex artifact path in SBOM`);
    }
  }
}

async function readJson(filePath) {
  const metadata = await fs.stat(filePath);
  if (metadata.size > MAX_JSON_BYTES) throw new Error(`${filePath}: JSON exceeds purity bound`);
  return JSON.parse(await fs.readFile(filePath, "utf8"));
}

function tar(args, label) {
  const result = spawnSync("tar", args, {
    encoding: "utf8",
    maxBuffer: MAX_ARCHIVE_LIST_BYTES,
  });
  if (result.error || result.status !== 0) {
    throw new Error(`${label}: archive inspection failed`);
  }
  return result.stdout;
}

async function inspectArchive(filePath, state) {
  const entries = tar(["-tf", filePath], filePath).split(/\r?\n/u).filter(Boolean);
  state.archiveEntries += entries.length;
  if (state.files + state.archiveEntries > MAX_FILES) {
    throw new Error(`${filePath}: archive entry count exceeds purity bound`);
  }
  for (const entry of entries) {
    if (forbiddenArtifactPath(entry)) {
      state.violations.push(`${filePath}: forbidden Codex artifact in archive (${entry})`);
    }
  }
  const jsonEntries = entries.filter((entry) =>
    /(?:^|\/)(?:package-lock|package)\.json$/u.test(entry) || /\.spdx\.json$/u.test(entry));
  if (jsonEntries.length > 100) throw new Error(`${filePath}: too many JSON manifests in archive`);
  for (const entry of jsonEntries) {
    const contents = tar(["-xOf", filePath, "--", entry], `${filePath}:${entry}`);
    if (Buffer.byteLength(contents) > MAX_JSON_BYTES) {
      throw new Error(`${filePath}:${entry}: JSON exceeds purity bound`);
    }
    const value = JSON.parse(contents);
    if (entry.endsWith("package-lock.json")) {
      inspectPackageLock(value, `${filePath}:${entry}`, state.violations);
    } else if (entry.endsWith("package.json")) {
      inspectPackageJson(value, `${filePath}:${entry}`, state.violations);
    } else {
      inspectSbom(value, `${filePath}:${entry}`, state.violations);
    }
  }
}

async function inspectFile(filePath, state) {
  state.files += 1;
  if (state.files + state.archiveEntries > MAX_FILES) {
    throw new Error(`${filePath}: file count exceeds purity bound`);
  }
  if (forbiddenArtifactPath(filePath)) {
    state.violations.push(`${filePath}: forbidden Codex artifact path`);
  }
  if (/\.(?:tar|tgz|tar\.gz)$/iu.test(filePath)) {
    await inspectArchive(filePath, state);
    return;
  }
  const name = path.basename(filePath);
  if (name === "package-lock.json") {
    inspectPackageLock(await readJson(filePath), filePath, state.violations);
  } else if (name === "package.json") {
    inspectPackageJson(await readJson(filePath), filePath, state.violations);
  } else if (name.endsWith(".spdx.json")) {
    inspectSbom(await readJson(filePath), filePath, state.violations);
  }
}

async function inspectPath(inputPath, state) {
  const resolved = path.resolve(inputPath);
  const metadata = await fs.lstat(resolved);
  if (metadata.isSymbolicLink()) {
    const target = await fs.readlink(resolved);
    if (forbiddenArtifactPath(target)) {
      state.violations.push(`${resolved}: symlink targets a forbidden Codex artifact`);
    }
    return;
  }
  if (metadata.isFile()) {
    await inspectFile(resolved, state);
    return;
  }
  if (!metadata.isDirectory()) throw new Error(`${resolved}: unsupported artifact type`);
  const entries = await fs.readdir(resolved, { withFileTypes: true });
  for (const entry of entries) {
    await inspectPath(path.join(resolved, entry.name), state);
  }
}

export async function inspectCodexPurity(paths) {
  const state = { archiveEntries: 0, files: 0, violations: [] };
  for (const inputPath of paths) await inspectPath(inputPath, state);
  return state;
}

async function selfTest() {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-codex-purity-"));
  try {
    const safe = path.join(root, "safe");
    await fs.mkdir(safe);
    await fs.writeFile(path.join(safe, "package-lock.json"), JSON.stringify({ packages: {} }));
    assert.deepEqual((await inspectCodexPurity([safe])).violations, []);

    const badManifest = path.join(root, "bad", "package.json");
    await fs.mkdir(path.dirname(badManifest));
    await fs.writeFile(badManifest, JSON.stringify({ dependencies: { "@openai/codex": "0.153.4" } }));
    assert.equal((await inspectCodexPurity([badManifest])).violations.length, 1);

    const badArtifact = path.join(root, "codex-linux-x64");
    await fs.writeFile(badArtifact, "fixture");
    assert.equal((await inspectCodexPurity([badArtifact])).violations.length, 1);

    const badSbom = path.join(root, "bad.spdx.json");
    await fs.writeFile(badSbom, JSON.stringify({ packages: [{ name: "@openai/codex" }] }));
    assert.equal((await inspectCodexPurity([badSbom])).violations.length, 1);
    const safeSbom = path.join(root, "safe.spdx.json");
    await fs.writeFile(safeSbom, JSON.stringify({ packages: [{ name: "prodex-codex-config" }] }));
    assert.deepEqual((await inspectCodexPurity([safeSbom])).violations, []);

    const archive = path.join(root, "rootfs.tar");
    tar(["-cf", archive, "-C", safe, "."], archive);
    assert.deepEqual((await inspectCodexPurity([archive])).violations, []);
    const badArchive = path.join(root, "bad-rootfs.tar");
    tar(["-cf", badArchive, "-C", root, path.basename(badArtifact)], badArchive);
    assert.equal((await inspectCodexPurity([badArchive])).violations.length, 1);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
}

async function main() {
  const args = process.argv.slice(2);
  if (args.includes("--self-test")) {
    await selfTest();
    process.stdout.write("codex purity guard: self-test ok\n");
    return;
  }
  if (args.length === 0 || args.some((arg) => arg.startsWith("--"))) {
    throw new Error("usage: node scripts/ci/codex-purity-guard.mjs <artifact-path> [...]");
  }
  const state = await inspectCodexPurity(args);
  if (state.violations.length > 0) {
    for (const violation of state.violations) process.stderr.write(`codex purity guard: ${violation}\n`);
    process.exitCode = 1;
    return;
  }
  process.stdout.write(
    `codex purity guard: ok (${state.files} file(s), ${state.archiveEntries} archive entries)\n`,
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch((error) => {
    process.stderr.write(`codex purity guard: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
