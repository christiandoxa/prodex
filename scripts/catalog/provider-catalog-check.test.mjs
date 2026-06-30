#!/usr/bin/env node
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";

const root = resolve(import.meta.dirname, "../..");
const script = resolve(root, "scripts/catalog/provider-catalog-check.mjs");

function run(args = []) {
  return spawnSync(process.execPath, [script, ...args], {
    cwd: root,
    encoding: "utf8",
  });
}

test("provider catalog reports non-zero model and provider counts", () => {
  const result = run(["--json"]);
  assert.equal(result.status, 0, result.stderr);
  const summary = JSON.parse(result.stdout);
  assert.ok(summary.sources.some((source) => source.endsWith("models.rs")));
  assert.ok(summary.model_count > 0);
  assert.ok(summary.provider_count > 0);
  assert.ok(summary.providers.OpenAi > 0);
});

test("empty catalog fixture fails", () => {
  const dir = mkdtempSync(join(tmpdir(), "prodex-provider-catalog-"));
  const fixture = join(dir, "empty.rs");
  writeFileSync(fixture, "pub const OPENAI_ENDPOINTS: &str = \"unused\";\n");

  const result = run([`--source=${fixture}`, "--json"]);
  assert.notEqual(result.status, 0);
  const summary = JSON.parse(result.stdout);
  assert.equal(summary.model_count, 0);
  assert.match(summary.issues.join("\n"), /model_count is 0/);
  assert.match(summary.issues.join("\n"), /required provider missing: OpenAi/);
});
