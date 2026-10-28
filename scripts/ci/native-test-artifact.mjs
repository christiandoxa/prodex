#!/usr/bin/env node
// Same-run native test executables are artifacts, never cross-source caches.
import assert from "node:assert/strict";
import { runStep } from "./main-internal-test-runner.mjs";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync, chmodSync, copyFileSync, lstatSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
export const WINDOWS_SKIPS = Object.freeze([
  "login::", "quota_doctor::", "run::", "shared_state::", "super_mode::",
  "continuity_failure_reason_metrics_", "rtk::tests::",
]);
export const sha256 = (value) => createHash("sha256").update(value).digest("hex");
const lines = (value = "") => value.split(/\r?\n/u).map((line) => line.trim()).filter(Boolean);

export function command(program, args, cwd, options = {}) {
  const result = spawnSync(program, args, {
    cwd, encoding: "utf8", maxBuffer: 64 * 1024 * 1024,
    timeout: 30 * 60 * 1000, windowsHide: true, ...options,
  });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${program} failed (${result.status}): ${result.stderr ?? ""}`);
  return result.stdout;
}

export function identity(env, repository) {
  assert.match(env.GITHUB_SHA ?? "", /^[a-f0-9]{40}$/u, "exact GitHub source SHA required");
  for (const key of ["GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT"]) {
    assert.match(env[key] ?? "", /^[1-9][0-9]*$/u, `${key} required`);
  }
  assert.equal(command("git", ["rev-parse", "HEAD"], repository).trim(), env.GITHUB_SHA,
    "checkout must match workflow source");
  assert.ok(Number.isSafeInteger(Number(env.GITHUB_RUN_ATTEMPT)), "invalid run attempt");
  const checkout = path.resolve(repository).replaceAll("\\", "/");
  return {
    checkout_path_sha256: sha256(process.platform === "win32" ? checkout.toLowerCase() : checkout),
    source_sha: env.GITHUB_SHA, run_id: env.GITHUB_RUN_ID,
    attempt: Number(env.GITHUB_RUN_ATTEMPT), platform: process.platform, arch: process.arch,
  };
}

export function testInventory(output) {
  const tests = lines(output).filter((line) => line.endsWith(": test")).map((line) => line.slice(0, -6)).sort();
  assert.ok(tests.length > 0, "compiled harness must enumerate tests");
  assert.equal(new Set(tests).size, tests.length, "duplicate native test names");
  return { count: tests.length, sha256: sha256(tests.join("\n")) };
}

export function selectTestExecutable(output) {
  const artifacts = lines(output).map((line) => JSON.parse(line)).filter((entry) =>
    entry.reason === "compiler-artifact" && entry.target?.name === "prodex_app" &&
    entry.target?.kind?.includes("lib") && entry.profile?.test === true && typeof entry.executable === "string",
  );
  assert.equal(artifacts.length, 1, "one prodex-app library test executable required");
  return artifacts[0].executable;
}

export function validateRunIdentity(manifest, expected) {
  for (const key of ["source_sha", "run_id", "platform", "arch", "checkout_path_sha256"]) {
    assert.equal(manifest[key], expected[key], `test artifact ${key} mismatch`);
  }
  // Rerun-failed may reuse the successful producer from the SAME workflow run.
  assert.ok(Number.isSafeInteger(manifest.attempt) && manifest.attempt >= 1 && manifest.attempt <= expected.attempt,
    "invalid or future producing attempt");
}

export function validateManifest(manifest, expected, binary, inventory) {
  assert.equal(manifest.schema, 1);
  assert.equal(manifest.package, "prodex-app");
  validateRunIdentity(manifest, expected);
  assert.equal(manifest.binary, expected.platform === "win32" ? "prodex-app-tests.exe" : "prodex-app-tests");
  assert.ok(binary.length > 0, "empty test executable");
  assert.equal(manifest.binary_sha256, sha256(binary), "test executable checksum mismatch");
  assert.ok(Number.isSafeInteger(manifest.inventory?.count) && manifest.inventory.count > 0, "native inventory missing");
  assert.match(manifest.inventory.sha256, /^[a-f0-9]{64}$/u);
  if (inventory) assert.deepEqual(manifest.inventory, inventory, "native test inventory changed");
}

export function testPlan(filters, skips, windows = false) {
  const excluded = [...lines(skips), ...(windows ? WINDOWS_SKIPS : [])];
  return (lines(filters).length ? lines(filters) : [null]).map((filter) => ({
    filter,
    args: [...(filter ? [filter] : []), "--test-threads=1", ...excluded.flatMap((value) => ["--skip", value])],
  }));
}

function pack(directory, env = process.env) {
  const expected = identity(env, root);
  const output = command("cargo", ["test", "--locked", "-p", "prodex-app", "--lib", "--no-run",
    "--message-format=json-render-diagnostics"], root, { stdio: ["ignore", "pipe", "inherit"] });
  const source = selectTestExecutable(output);
  assert.ok(lstatSync(source).isFile() && !lstatSync(source).isSymbolicLink());
  const bytes = readFileSync(source);
  const inventory = testInventory(command(source, ["--list", "--format", "terse"], path.join(root, "crates/prodex-app")));
  const manifest = {
    schema: 1, package: "prodex-app", ...expected,
    binary: process.platform === "win32" ? "prodex-app-tests.exe" : "prodex-app-tests",
    binary_sha256: sha256(bytes), inventory,
  };
  mkdirSync(directory, { recursive: true });
  copyFileSync(source, path.join(directory, manifest.binary));
  const manifestBytes = JSON.stringify(manifest, null, 2) + "\n";
  writeFileSync(path.join(directory, "manifest.json"), manifestBytes);
  assert.ok(env.GITHUB_OUTPUT, "producer output file required");
  appendFileSync(env.GITHUB_OUTPUT, `manifest_sha256=${sha256(manifestBytes)}\n`);
  process.stdout.write(`Packed ${inventory.count} ${expected.platform}/${expected.arch} tests from ${expected.source_sha}\n`);
}

export function verifiedBundle(directory, expected, manifestDigest, { repository = root } = {}) {
  assert.match(manifestDigest ?? "", /^[a-f0-9]{64}$/u, "independent producer manifest digest required");
  const manifestPath = path.join(directory, "manifest.json");
  assert.ok(lstatSync(manifestPath).isFile() && !lstatSync(manifestPath).isSymbolicLink());
  const manifestBytes = readFileSync(manifestPath);
  assert.equal(sha256(manifestBytes), manifestDigest, "manifest differs from producing job output");
  const manifest = JSON.parse(manifestBytes);
  const binaryName = expected.platform === "win32" ? "prodex-app-tests.exe" : "prodex-app-tests";
  assert.equal(manifest.binary, binaryName, "unexpected executable path");
  const executable = path.resolve(directory, binaryName);
  assert.ok(lstatSync(executable).isFile() && !lstatSync(executable).isSymbolicLink());
  const binary = readFileSync(executable);
  validateManifest(manifest, expected, binary);
  if (process.platform !== "win32") chmodSync(executable, 0o700);
  // Match Cargo's documented package-root working directory. Test fixtures and
  // compile-time source paths still refer to this exact same-source checkout.
  const cwd = path.join(repository, "crates/prodex-app");
  const listing = command(executable, ["--list", "--format", "terse"], cwd);
  const inventory = testInventory(listing);
  validateManifest(manifest, expected, binary, inventory);
  return { executable, cwd, inventory, listing };
}

// Supplying a bundle opts in to strict same-run verification. A broken binding
// must never silently fall back to compiling or executing an unverified file.
export function environmentBundle(env = process.env) {
  if (env.PRODEX_TEST_BUNDLE === undefined) return null;
  assert.ok(env.PRODEX_TEST_BUNDLE.trim(), "native test bundle path required");
  return verifiedBundle(path.resolve(root, env.PRODEX_TEST_BUNDLE), identity(env, root), env.TEST_MANIFEST_SHA256);
}

export async function runBundle(directory, expected, manifestDigest, { repository = root, filters = "", skips = "" } = {}) {
  const { executable, cwd, inventory } = verifiedBundle(directory, expected, manifestDigest, { repository });
  let failed = false;
  for (const entry of testPlan(filters, skips, expected.platform === "win32")) {
    process.stdout.write(`Native test partition: ${entry.filter ?? "remainder"}\n`);
    let stdoutTail = "", sawZeroTests = false;
    try {
      // Stream progress immediately instead of buffering minutes of native test
      // output. Reuse the existing deadline/process-tree runner, without retries.
      await runStep({
        label: `native:${entry.filter ?? "remainder"}`, command: executable,
        args: entry.args, cwd, timeoutMs: 30 * 60 * 1000, failOnZeroTests: true,
        observeStdout(chunk) {
          const text = stdoutTail + chunk;
          sawZeroTests ||= /\brunning 0 tests\b/u.test(text);
          stdoutTail = text.slice(-65536);
        },
      });
      const summary = [...stdoutTail.matchAll(/test result: ok\. (\d+) passed; 0 failed; (\d+) ignored/gu)].at(-1);
      assert.ok(!sawZeroTests && summary, "native partition must report successful nonempty execution");
    } catch (error) {
      failed = true;
      process.stderr.write(`Native test partition failed or matched no tests: ${entry.filter ?? "remainder"}\n${error.message}\n`);
    }
  }
  assert.equal(failed, false, "one or more native test partitions failed");
  return inventory;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    const [mode, bundle, ...extra] = process.argv.slice(2);
    assert.ok(["pack", "run"].includes(mode) && bundle && extra.length === 0,
      "usage: node scripts/ci/native-test-artifact.mjs pack|run BUNDLE_DIRECTORY");
    const directory = path.resolve(root, bundle);
    if (mode === "pack") pack(directory);
    else await runBundle(directory, identity(process.env, root), process.env.TEST_MANIFEST_SHA256, {
      filters: process.env.PRODEX_APP_FILTERS, skips: process.env.PRODEX_APP_SKIP_FILTERS,
    });
  } catch (error) {
    process.stderr.write(`native-test-artifact: ${error.message}\n`);
    process.exitCode = 1;
  }
}
