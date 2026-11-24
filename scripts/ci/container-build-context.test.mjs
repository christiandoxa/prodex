import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function projectBuilderContext(dockerfile, source, destination) {
  let builder = false;
  for (const line of dockerfile.split(/\r?\n/u)) {
    if (/^FROM\s/iu.test(line)) builder = /\sAS\sbuilder\s*$/iu.test(line);
    if (!builder || !/^COPY\s/iu.test(line) || /^COPY\s+--from=/iu.test(line)) continue;
    const words = line.trim().split(/\s+/u).slice(1);
    const target = words.pop();
    assert.ok(target && !target.startsWith("/"), "builder COPY destinations must be relative to WORKDIR");
    for (const entry of words) {
      assert.ok(!entry.startsWith("--") && !entry.startsWith("["), "fixture expects ordinary COPY arguments");
      const output = words.length > 1 || target === "./"
        ? path.join(destination, target, path.basename(entry))
        : path.join(destination, target);
      cpSync(path.join(source, entry), output, { recursive: true });
    }
  }
}

function metadataForContext(dockerfile) {
  const root = mkdtempSync(path.join(os.tmpdir(), "prodex-container-context-"));
  try {
    const source = path.join(root, "source");
    const projected = path.join(root, "projected");
    const cargoHome = path.join(root, "cargo-home");
    for (const directory of ["src", "crates", "examples", "benches", "mojo", "vendor/tiny_http/src"]) {
      mkdirSync(path.join(source, directory), { recursive: true });
    }
    mkdirSync(projected);
    mkdirSync(cargoHome);
    writeFileSync(path.join(source, "Cargo.toml"), `[package]
name = "container-context-fixture"
version = "0.0.0"
edition = "2024"
[workspace]
[dependencies]
tiny_http = "=0.12.0"
[patch.crates-io]
tiny_http = { path = "vendor/tiny_http" }
`);
    writeFileSync(path.join(source, "src/main.rs"), "fn main() {}\n");
    writeFileSync(path.join(source, "vendor/tiny_http/Cargo.toml"), `[package]
name = "tiny_http"
version = "0.12.0"
edition = "2021"
`);
    writeFileSync(path.join(source, "vendor/tiny_http/src/lib.rs"), "pub fn fixture() {}\n");
    for (const file of ["install.sh", "install.ps1"]) writeFileSync(path.join(source, file), "fixture\n");
    const options = {
      cwd: source,
      encoding: "utf8",
      env: { ...process.env, CARGO_HOME: cargoHome, CARGO_TARGET_DIR: path.join(root, "target") },
      timeout: 20000,
    };
    const lock = spawnSync("cargo", ["generate-lockfile", "--offline"], options);
    assert.equal(lock.status, 0, lock.stderr || String(lock.error));
    projectBuilderContext(dockerfile, source, projected);
    return spawnSync("cargo", ["metadata", "--offline", "--locked", "--format-version", "1"], {
      ...options,
      cwd: projected,
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("Docker builder COPY context resolves the repository's local vendor patch", () => {
  const result = metadataForContext(readFileSync(path.join(repoRoot, "Dockerfile"), "utf8"));
  assert.equal(result.status, 0, result.stderr || String(result.error));
  assert.equal(JSON.parse(result.stdout).packages[0].name, "container-context-fixture");
});

test("Cargo rejects a builder context that omits the vendored patch", () => {
  const dockerfile = readFileSync(path.join(repoRoot, "Dockerfile"), "utf8")
    .split(/\r?\n/u).filter((line) => !/^COPY\s+vendor(?:\/|\s)/u.test(line)).join("\n");
  const result = metadataForContext(dockerfile);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /tiny_http/u);
  assert.match(result.stderr, /failed to (?:load|read)|No such file|cannot find|does not contain packages/iu);
});
