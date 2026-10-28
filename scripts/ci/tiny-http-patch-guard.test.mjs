import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";
import { verifyTinyHttpPatch } from "./tiny-http-patch-guard.mjs";

test("tiny_http repair preserves audited vendor source and bounded retry", async () => {
  const result = await verifyTinyHttpPatch();
  assert.equal(result.patchedFiles, 5);
  assert.ok(result.sourceFilesVerified >= 20);
});

test("tiny_http guard rejects edits outside the reviewed transport patch", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-vendor-integrity-"));
  try {
    await fs.cp(path.join(repoRoot, "vendor/tiny_http"), path.join(root, "vendor/tiny_http"), { recursive: true });
    await fs.copyFile(path.join(repoRoot, "Cargo.toml"), path.join(root, "Cargo.toml"));
    await fs.appendFile(path.join(root, "vendor/tiny_http/src/request.rs"), "\n// unreviewed mutation\n");
    await assert.rejects(verifyTinyHttpPatch(root), /unreviewed tiny_http source change: src\/request.rs/);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
