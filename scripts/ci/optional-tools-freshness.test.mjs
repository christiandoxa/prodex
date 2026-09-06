import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs/promises";
import test from "node:test";
import {
  latestStableReleaseTag,
  parseArgs,
  runFreshnessCheck,
} from "./optional-tools-freshness.mjs";

const auditPath = "migration/optional-tools-audit.json";
const releaseSha = "0123456789abcdef0123456789abcdef01234567";

async function auditedFixture() {
  const audit = JSON.parse(await fs.readFile(auditPath, "utf8"));
  const observed = Object.fromEntries(
    audit.tools.map((tool) => [tool.id, tool.latest_stable]),
  );
  return { audit, observed };
}

function outputBuffer() {
  const chunks = [];
  return { chunks, output: { write: (chunk) => chunks.push(chunk) } };
}

test("optional-tool freshness checker validates normalization and drift rules", () => {
  const output = execFileSync(
    process.execPath,
    ["scripts/ci/optional-tools-freshness.mjs", "--self-test"],
    { encoding: "utf8" },
  );
  assert.match(output, /optional-tools-freshness: self-test ok/u);
});

test("stable component selector ignores prereleases and unrelated monorepo releases", () => {
  const releases = [
    { tag_name: "v2.7.0-rc.1", draft: false, prerelease: true },
    { tag_name: "bin-v1.1.6", draft: false, prerelease: false },
    { tag_name: "v2.6.0", draft: false, prerelease: false },
  ];
  assert.equal(latestStableReleaseTag(releases, ""), "v2.6.0");
  assert.equal(latestStableReleaseTag(releases.slice(1, 2), ""), null);
});

test("JSON evidence is real JSON and contains all six results", async () => {
  const { observed } = await auditedFixture();
  const { chunks, output } = outputBuffer();
  const evidence = await runFreshnessCheck({
    fetchLatest: async () => observed,
    json: true,
    checkpoint: "B",
    releaseSha,
    now: () => new Date("2026-09-06T05:31:00.000Z"),
    output,
  });
  const parsed = JSON.parse(chunks.join(""));
  assert.deepEqual(parsed, evidence);
  assert.deepEqual(
    {
      schema_version: parsed.schema_version,
      checkpoint: parsed.checkpoint,
      release_sha: parsed.release_sha,
      audited_at: parsed.audited_at,
    },
    {
      schema_version: 1,
      checkpoint: "B",
      release_sha: releaseSha,
      audited_at: "2026-09-06T05:31:00.000Z",
    },
  );
  assert.equal(parsed.results.length, 6);
  assert.ok(parsed.results.every((result) => result.status === "latest"));
});

test("default output remains the existing human TSV", async () => {
  const { audit, observed } = await auditedFixture();
  const { chunks, output } = outputBuffer();
  await runFreshnessCheck({ fetchLatest: async () => observed, output });
  assert.equal(
    chunks.join(""),
    audit.tools
      .map((tool) => `${tool.id}\t${tool.latest_stable}\t${tool.latest_stable}\tlatest\n`)
      .join(""),
  );
});

test("missing observed tool fails closed", async () => {
  const { observed } = await auditedFixture();
  delete observed.ponytail;
  await assert.rejects(
    runFreshnessCheck({ fetchLatest: async () => observed, output: null }),
    /optional-tool freshness drift: ponytail/u,
  );
});

test("inconsistent registry versions fail closed", async () => {
  const { observed } = await auditedFixture();
  observed.presidio = ["2.2.364", "2.2.364", "2.2.365"];
  await assert.rejects(
    runFreshnessCheck({ fetchLatest: async () => observed, output: null }),
    /optional-tool freshness drift: presidio/u,
  );
});

test("network or registry errors remain failures", async () => {
  await assert.rejects(
    runFreshnessCheck({
      fetchLatest: async () => {
        throw new Error("registry unavailable");
      },
      output: null,
    }),
    /registry unavailable/u,
  );
});

test("bad release SHA and checkpoint are rejected before checking", () => {
  assert.throws(
    () => parseArgs(["node", "checker", "--release-sha", "not-a-sha"]),
    /40-character hexadecimal SHA/u,
  );
  assert.throws(
    () => parseArgs(["node", "checker", "--checkpoint", "C", "--release-sha", releaseSha]),
    /invalid checkpoint: C/u,
  );
  assert.throws(
    () => parseArgs(["node", "checker", "--checkpoint", "A", "--release-sha", releaseSha]),
    /invalid checkpoint: A/u,
  );
  assert.throws(
    () => parseArgs(["node", "checker", "--json"]),
    /--json requires --checkpoint and --release-sha/u,
  );
});
