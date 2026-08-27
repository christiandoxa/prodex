#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const auditPath = path.join(repoRoot, "migration", "optional-tools-audit.json");
const githubTools = new Map([
  ["caveman", "JuliusBrussee/caveman"],
  ["rtk", "rtk-ai/rtk"],
  ["codebase-memory-mcp", "DeusData/codebase-memory-mcp"],
  ["playwright-mcp", "microsoft/playwright-mcp"],
  ["ponytail", "DietrichGebert/ponytail"],
  ["presidio", "data-privacy-stack/presidio"],
]);

function normalizeVersion(value) {
  const match = String(value ?? "")
    .trim()
    .match(/^v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)(?:\+[0-9A-Za-z.-]+)?$/u);
  return match?.[1] ?? null;
}

export function compareFreshness(audit, observed) {
  return audit.tools.map((tool) => {
    const actual = observed[tool.id];
    const expected = normalizeVersion(tool.latest_stable);
    const versions = Array.isArray(actual) ? actual : [actual];
    const normalized = versions.map(normalizeVersion);
    const consistent = normalized.length > 0 && normalized.every((version) => version === normalized[0]);
    return {
      id: tool.id,
      expected,
      observed: normalized,
      status: consistent && normalized[0] === expected ? "latest" : "drift",
    };
  });
}

async function fetchJson(url) {
  const response = await fetch(url, {
    headers: {
      accept: "application/json",
      "user-agent": "prodex-optional-tools-freshness",
    },
    signal: AbortSignal.timeout(10_000),
  });
  if (!response.ok) throw new Error(`${url} returned HTTP ${response.status}`);
  return response.json();
}

async function observedLatest() {
  const observed = Object.fromEntries(
    await Promise.all(
      [...githubTools.entries()].map(async ([id, repository]) => {
        const release = await fetchJson(`https://api.github.com/repos/${repository}/releases/latest`);
        return [id, release.tag_name];
      }),
    ),
  );
  const playwright = await fetchJson("https://registry.npmjs.org/@playwright%2fmcp/latest");
  observed["playwright-mcp"] = [observed["playwright-mcp"], playwright.version];
  const [analyzer, anonymizer] = await Promise.all([
    fetchJson("https://pypi.org/pypi/presidio-analyzer/json"),
    fetchJson("https://pypi.org/pypi/presidio-anonymizer/json"),
  ]);
  observed.presidio = [observed.presidio, analyzer.info.version, anonymizer.info.version];
  return observed;
}

export async function runFreshnessCheck({ fetchLatest = observedLatest } = {}) {
  const audit = JSON.parse(await fs.readFile(auditPath, "utf8"));
  assert.equal(audit.schema_version, 1, "optional-tool audit schema must be 1");
  const results = compareFreshness(audit, await fetchLatest());
  for (const result of results) {
    process.stdout.write(
      `${result.id}\t${result.expected}\t${result.observed.join(",")}\t${result.status}\n`,
    );
  }
  const drift = results.filter((result) => result.status !== "latest");
  if (drift.length > 0) {
    throw new Error(`optional-tool freshness drift: ${drift.map((result) => result.id).join(", ")}`);
  }
  return results;
}

function selfTest() {
  const audit = {
    tools: [
      { id: "one", latest_stable: "v1.2.3" },
      { id: "two", latest_stable: "0.4.5" },
    ],
  };
  assert.deepEqual(
    compareFreshness(audit, { one: "1.2.3", two: ["v0.4.5", "0.4.5"] }),
    [
      { id: "one", expected: "1.2.3", observed: ["1.2.3"], status: "latest" },
      { id: "two", expected: "0.4.5", observed: ["0.4.5", "0.4.5"], status: "latest" },
    ],
  );
  assert.equal(compareFreshness(audit, { one: "1.2.4", two: "0.4.5" })[0].status, "drift");
  assert.equal(normalizeVersion("v1.2.3+build.4"), "1.2.3");
  assert.equal(normalizeVersion("latest"), null);
}

if (process.argv.includes("--self-test")) {
  selfTest();
  process.stdout.write("optional-tools-freshness: self-test ok\n");
} else {
  runFreshnessCheck().catch((error) => {
    process.stderr.write(`optional-tools-freshness: ${error.message}\n`);
    process.exitCode = 1;
  });
}
