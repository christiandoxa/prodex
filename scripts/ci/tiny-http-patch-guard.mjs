#!/usr/bin/env node
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { repoRoot } from "../npm/common.mjs";

export async function verifyTinyHttpPatch(root = repoRoot) {
  const directory = path.join(root, "vendor/tiny_http");
  const manifest = JSON.parse(await fs.readFile(path.join(directory, "PRODEX-PATCH.json"), "utf8"));
  assert.equal(manifest.crate, "tiny_http");
  assert.equal(manifest.version, "0.12.0");
  assert.equal(manifest.crate_archive_sha256, "389915df6413a2e74fb181895f933386023c71110878cd0825588928e64cdc82");
  assert.deepEqual(manifest.patched_files, ["src/lib.rs", "src/util/refined_tcp_stream.rs"]);
  assert.deepEqual(manifest.new_files, ["src/accept_retry.rs"]);
  assert.ok(manifest.original_files["LICENSE-APACHE"]);
  assert.ok(manifest.original_files["LICENSE-MIT"]);
  const expected = { ...manifest.original_files, ...manifest.patched_sha256 };
  for (const [name, hash] of Object.entries(expected)) {
    assert.ok(!path.isAbsolute(name) && !name.split("/").includes(".."), "vendor path boundary");
    const digest = createHash("sha256").update(await fs.readFile(path.join(directory, name))).digest("hex");
    assert.equal(digest, hash, `unreviewed tiny_http source change: ${name}`);
  }
  const cargo = await fs.readFile(path.join(root, "Cargo.toml"), "utf8");
  assert.match(cargo, /\[patch\.crates-io\][\s\S]*tiny_http = \{ path = "vendor\/tiny_http" \}/);
  const retry = await fs.readFile(path.join(directory, "src/accept_retry.rs"), "utf8");
  assert.ok(retry.includes("while !shutdown.load(Ordering::Relaxed)"));
  assert.ok(retry.includes(".min(250)"));
  assert.ok(retry.includes("std::thread::sleep"));
  const stream = await fs.readFile(path.join(directory, "src/util/refined_tcp_stream.rs"), "utf8");
  assert.ok(!stream.includes("tcp_stream.try_clone().unwrap()"));
  assert.ok(stream.includes('until_shutdown("split accepted stream"'));
  return { sourceFilesVerified: Object.keys(expected).length, patchedFiles: manifest.patched_files.length, verifiedPaths: Object.keys(expected).map((name) => `vendor/tiny_http/${name}`) };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  verifyTinyHttpPatch().then((result) => {
    console.log(`tiny-http patch guard: ${result.sourceFilesVerified} source files verified; transport-only repair intact`);
  }).catch((error) => { console.error(`tiny-http patch guard: ${error.message}`); process.exitCode = 1; });
}
