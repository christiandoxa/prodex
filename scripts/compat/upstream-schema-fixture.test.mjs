import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = fileURLToPath(new URL("../../", import.meta.url));
const baselinePath = join(root, "scripts/compat/upstream-baseline.json");
const fixtureRoot = join(
  root,
  "crates/prodex-app/tests/fixtures/compat_replay/upstream_codex_schema",
);

function runGuard(args = []) {
  return spawnSync(
    process.execPath,
    ["scripts/compat/check-upstream-baseline.mjs", ...args],
    { cwd: root, encoding: "utf8" },
  );
}

test("default offline guard validates committed native lifecycle schemas", () => {
  const result = runGuard();
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

test("default offline guard rejects a stale committed schema digest", () => {
  const directory = mkdtempSync(join(tmpdir(), "prodex-upstream-schema-"));
  try {
    const baseline = JSON.parse(readFileSync(baselinePath, "utf8"));
    baseline.codex.compatibility.app_server_protocol.stable_v2_sha256[
      "ThreadResumeResponse.json"
    ] = "0".repeat(64);
    const mutatedPath = join(directory, "baseline.json");
    writeFileSync(mutatedPath, JSON.stringify(baseline));
    const result = runGuard(["--baseline", mutatedPath]);
    assert.equal(result.status, 1, result.stdout + result.stderr);
    assert.match(
      result.stdout + result.stderr,
      /generated stable v2 schema SHA-256 mismatch: ThreadResumeResponse\.json/,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("explicit native schema validation rejects an obsolete answer display contract", () => {
  const directory = mkdtempSync(join(tmpdir(), "prodex-upstream-schema-"));
  try {
    cpSync(fixtureRoot, join(directory, "v2"), { recursive: true });
    const schemaPath = join(directory, "v2", "ThreadResumeResponse.json");
    const schema = JSON.parse(readFileSync(schemaPath, "utf8"));
    schema.definitions.AgentMessageDisplay = {
      type: "object",
      required: ["action"],
      properties: { action: { type: "string" } },
    };
    writeFileSync(schemaPath, JSON.stringify(schema, null, 2) + "\n");
    const result = runGuard(["--schemas", directory]);
    assert.equal(result.status, 1, result.stdout + result.stderr);
    assert.match(
      result.stdout + result.stderr,
      /generated stable v2 schema SHA-256 mismatch: ThreadResumeResponse\.json/,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("missing protocol metadata remains a reported baseline error", () => {
  const directory = mkdtempSync(join(tmpdir(), "prodex-upstream-schema-"));
  try {
    const baseline = JSON.parse(readFileSync(baselinePath, "utf8"));
    delete baseline.codex.compatibility.app_server_protocol;
    const mutatedPath = join(directory, "baseline.json");
    writeFileSync(mutatedPath, JSON.stringify(baseline));
    const result = runGuard(["--baseline", mutatedPath]);
    assert.equal(result.status, 1, result.stdout + result.stderr);
    assert.match(result.stdout, /codex.compatibility.app_server_protocol is missing/);
    assert.equal(result.stderr, "");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
