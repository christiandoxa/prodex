#!/usr/bin/env node
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
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
  assert.ok(summary.sources.some((source) => source.endsWith("models.json")));
  assert.ok(summary.model_count > 0);
  assert.ok(summary.provider_count > 0);
  assert.ok(summary.providers.OpenAi > 0);
  assert.ok(summary.providers.Kiro > 0);
  const catalog = JSON.parse(
    readFileSync(resolve(root, "crates/prodex-provider-core/catalog/models.json"), "utf8"),
  );
  const luna = catalog.find((model) => model.provider === "openai" && model.id === "gpt-5.6-luna");
  assert.ok(luna?.supported_reasoning_efforts.includes("max"));
});

test("empty catalog fixture fails", () => {
  const dir = mkdtempSync(join(tmpdir(), "prodex-provider-catalog-"));
  const fixture = join(dir, "empty.json");
  writeFileSync(fixture, "[]\n");

  const result = run([`--source=${fixture}`, "--json"]);
  assert.notEqual(result.status, 0);
  const summary = JSON.parse(result.stdout);
  assert.equal(summary.model_count, 0);
  assert.match(summary.issues.join("\n"), /model_count is 0/);
  assert.match(summary.issues.join("\n"), /required provider missing: openai/);
});

test("non-positive context windows fail", () => {
  const dir = mkdtempSync(join(tmpdir(), "prodex-provider-catalog-"));
  const fixture = join(dir, "invalid-context-window.json");
  const catalog = JSON.parse(
    readFileSync(resolve(root, "crates/prodex-provider-core/catalog/models.json"), "utf8"),
  );
  const model = catalog.find((entry) => entry.provider === "openai");

  for (const contextWindow of [0, -1]) {
    model.context_window_tokens = contextWindow;
    writeFileSync(fixture, JSON.stringify(catalog));
    const result = run([`--source=${fixture}`, "--json"]);
    assert.notEqual(result.status, 0);
    assert.match(
      JSON.parse(result.stdout).issues.join("\n"),
      /context_window_tokens must be a positive safe integer/,
    );
  }
});
