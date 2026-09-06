#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { repoRoot } from "../npm/common.mjs";

const auditPath = path.join(repoRoot, "migration", "optional-tools-audit.json");
const runtimeInventoryPath = path.join(repoRoot, "crates/prodex-optional-tools/src/lib.rs");
const FRESHNESS_SCHEMA_VERSION = 1;
const VALID_CHECKPOINTS = new Set(["B"]);
const RELEASE_SHA_PATTERN = /^[0-9a-f]{40}$/u;

export function parseArgs(argv) {
  const args = {
    check: false,
    json: false,
    checkpoint: null,
    releaseSha: null,
    selfTest: false,
  };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--check") {
      args.check = true;
      continue;
    }
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--self-test") {
      args.selfTest = true;
      continue;
    }
    if (value === "--checkpoint" || value === "--release-sha") {
      const optionValue = argv[++index];
      if (!optionValue || optionValue.startsWith("--")) {
        throw new Error(`${value} requires a value`);
      }
      if (value === "--checkpoint") args.checkpoint = optionValue;
      else args.releaseSha = optionValue;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  if (args.checkpoint !== null && !VALID_CHECKPOINTS.has(args.checkpoint)) {
    throw new Error(`invalid checkpoint: ${args.checkpoint}`);
  }
  if (args.releaseSha !== null && !RELEASE_SHA_PATTERN.test(args.releaseSha)) {
    throw new Error("--release-sha must be a 40-character hexadecimal SHA");
  }
  if ((args.checkpoint === null) !== (args.releaseSha === null)) {
    throw new Error("--checkpoint and --release-sha must be provided together");
  }
  if (args.json && args.checkpoint === null) {
    throw new Error("--json requires --checkpoint and --release-sha");
  }
  return args;
}

function normalizeVersion(value) {
  const match = String(value ?? "")
    .trim()
    .match(/^v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)(?:\+[0-9A-Za-z.-]+)?$/u);
  return match?.[1] ?? null;
}

export function runtimePinnedVersions(source) {
  const constants = new Map([
    ["caveman", ["CAVEMAN_VETTED_VERSION", ""]],
    ["rtk", ["RTK_RECOMMENDED_VERSION", ""]],
    ["codebase-memory-mcp", ["CODEBASE_MEMORY_RECOMMENDED_VERSION", ""]],
    ["playwright-mcp", ["PLAYWRIGHT_MCP_PACKAGE", "@playwright/mcp@"]],
    ["ponytail", ["PONYTAIL_VETTED_VERSION", ""]],
    ["presidio", ["PRESIDIO_RECOMMENDED_VERSION", ""]],
  ]);
  return Object.fromEntries([...constants].map(([id, [name, prefix]]) => {
    const match = source.match(
      new RegExp(`^pub(?:\\(crate\\))? const ${name}: &str = "([^"]+)";`, "mu"),
    );
    if (!match || !match[1].startsWith(prefix)) {
      throw new Error(`runtime optional-tool inventory is missing ${name}`);
    }
    return [id, match[1].slice(prefix.length)];
  }));
}

export function releaseTagPrefix(value) {
  return String(value ?? "")
    .trim()
    .match(/^(.*?)(?:v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)(?:\+[0-9A-Za-z.-]+)?$/u)?.[1] ?? null;
}

export function latestStableReleaseTag(releases, componentPrefix = null) {
  if (!Array.isArray(releases)) return null;
  return (
    releases.find(
      (release) => {
        const tag = String(release?.tag_name ?? "").trim();
        const version = normalizeVersion(tag);
        return (
          release?.draft === false &&
          release?.prerelease === false &&
          version &&
          !version.includes("-") &&
          (componentPrefix === null || releaseTagPrefix(tag) === componentPrefix)
        );
      },
    )?.tag_name ?? null
  );
}

export function compareFreshness(audit, observed, pinned) {
  if (!Array.isArray(audit?.tools)) throw new Error("optional-tool audit tools must be an array");
  if (!observed || typeof observed !== "object" || Array.isArray(observed)) {
    throw new Error("optional-tool observations must be an object");
  }
  if (!pinned || typeof pinned !== "object" || Array.isArray(pinned)) {
    throw new Error("runtime optional-tool pins must be an object");
  }
  return audit.tools.map((tool) => {
    const actual = Object.hasOwn(observed, tool.id) ? observed[tool.id] : undefined;
    const expected = normalizeVersion(tool.latest_stable);
    const versions = Array.isArray(actual) ? actual : actual == null ? [] : [actual];
    const normalized = versions.map(normalizeVersion);
    const runtimePin = normalizeVersion(pinned[tool.id]);
    const consistent =
      normalized.length > 0 &&
      normalized.every((version) => version !== null && version === normalized[0]);
    return {
      id: tool.id,
      expected,
      observed: normalized,
      pinned: runtimePin,
      status:
        consistent && normalized[0] === expected && runtimePin === expected ? "latest" : "drift",
    };
  });
}

function githubRepository(repository) {
  try {
    const url = new URL(repository);
    if (url.protocol !== "https:" || url.hostname !== "github.com") return null;
    const parts = url.pathname.split("/").filter(Boolean);
    if (parts.length !== 2) return null;
    const name = parts[1].replace(/\.git$/u, "");
    return name ? `${parts[0]}/${name}` : null;
  } catch {
    return null;
  }
}

function auditTool(audit, id) {
  const tool = audit.tools.find((candidate) => candidate.id === id);
  if (!tool) throw new Error(`optional-tool audit is missing ${id}`);
  return tool;
}

function validateAudit(audit) {
  if (audit?.schema_version !== FRESHNESS_SCHEMA_VERSION) {
    throw new Error(`optional-tool audit schema must be ${FRESHNESS_SCHEMA_VERSION}`);
  }
  if (!Array.isArray(audit.tools) || audit.tools.length !== 6) {
    throw new Error("optional-tool audit must contain six tools");
  }
  if (new Set(audit.tools.map((tool) => tool.id)).size !== audit.tools.length) {
    throw new Error("optional-tool audit tool ids must be unique");
  }
}

export async function fetchJson(
  url,
  {
    fetchImpl = fetch,
    delay = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds)),
  } = {},
) {
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    let response;
    try {
      response = await fetchImpl(url, {
        headers: {
          accept: "application/json",
          "user-agent": "prodex-optional-tools-freshness",
        },
        signal: AbortSignal.timeout(10_000),
      });
    } catch (error) {
      if (attempt === 2) throw error;
      await delay(250);
      continue;
    }
    if (response.ok) return response.json();
    const error = new Error(`${url} returned HTTP ${response.status}`);
    if (
      attempt === 2 ||
      ![408, 425, 429].includes(response.status) && response.status < 500
    ) {
      throw error;
    }
    await delay(250);
  }
  throw new Error(`${url} could not be checked`);
}

async function observedLatest(audit) {
  const observed = Object.fromEntries(
    await Promise.all(audit.tools.map(async (tool) => {
      const repository = githubRepository(tool.repository);
      const componentPrefix = releaseTagPrefix(tool.release);
      if (!repository || componentPrefix === null) {
        throw new Error(`invalid GitHub release selector for ${tool.id}`);
      }
      const releases = await fetchJson(
        `https://api.github.com/repos/${repository}/releases?per_page=100`,
      );
      const tag = latestStableReleaseTag(releases, componentPrefix);
      if (!tag) throw new Error(`no stable ${componentPrefix}semver release found for ${repository}`);
      return [tool.id, tag];
    })),
  );
  const playwright = await fetchJson("https://registry.npmjs.org/@playwright%2fmcp/latest");
  const playwrightTool = auditTool(audit, "playwright-mcp");
  observed[playwrightTool.id] = [observed[playwrightTool.id], playwright.version];
  const [analyzer, anonymizer] = await Promise.all([
    fetchJson("https://pypi.org/pypi/presidio-analyzer/json"),
    fetchJson("https://pypi.org/pypi/presidio-anonymizer/json"),
  ]);
  const presidio = auditTool(audit, "presidio");
  observed[presidio.id] = [
    observed[presidio.id],
    analyzer?.info?.version,
    anonymizer?.info?.version,
  ];
  return observed;
}

export async function runFreshnessCheck({
  fetchLatest = observedLatest,
  inventorySource = null,
  json = false,
  checkpoint = null,
  releaseSha = null,
  now = () => new Date(),
  output = process.stdout,
} = {}) {
  const audit = JSON.parse(await fs.readFile(auditPath, "utf8"));
  validateAudit(audit);
  if (checkpoint !== null && !VALID_CHECKPOINTS.has(checkpoint)) {
    throw new Error(`invalid checkpoint: ${checkpoint}`);
  }
  if (releaseSha !== null && !RELEASE_SHA_PATTERN.test(releaseSha)) {
    throw new Error("--release-sha must be a 40-character hexadecimal SHA");
  }
  if ((checkpoint === null) !== (releaseSha === null)) {
    throw new Error("--checkpoint and --release-sha must be provided together");
  }
  if (json && checkpoint === null) {
    throw new Error("--json requires --checkpoint and --release-sha");
  }
  const pinned = runtimePinnedVersions(
    inventorySource ?? await fs.readFile(runtimeInventoryPath, "utf8"),
  );
  const results = compareFreshness(audit, await fetchLatest(audit), pinned);
  const evidence = {
    schema_version: FRESHNESS_SCHEMA_VERSION,
    checkpoint,
    release_sha: releaseSha,
    audited_at: now().toISOString(),
    results,
  };
  if (output !== null) {
    if (json) output.write(`${JSON.stringify(evidence, null, 2)}\n`);
    else {
      for (const result of results) {
        output.write(
          `${result.id}\t${result.expected}\t${result.observed.join(",")}\t${result.status}\n`,
        );
      }
    }
  }
  const drift = results.filter((result) => result.status !== "latest");
  if (drift.length > 0) {
    throw new Error(`optional-tool freshness drift: ${drift.map((result) => result.id).join(", ")}`);
  }
  return json ? evidence : results;
}

function selfTest() {
  const audit = {
    tools: [
      { id: "one", latest_stable: "v1.2.3" },
      { id: "two", latest_stable: "0.4.5" },
    ],
  };
  assert.deepEqual(
    compareFreshness(
      audit,
      { one: "1.2.3", two: ["v0.4.5", "0.4.5"] },
      { one: "1.2.3", two: "0.4.5" },
    ),
    [
      {
        id: "one",
        expected: "1.2.3",
        observed: ["1.2.3"],
        pinned: "1.2.3",
        status: "latest",
      },
      {
        id: "two",
        expected: "0.4.5",
        observed: ["0.4.5", "0.4.5"],
        pinned: "0.4.5",
        status: "latest",
      },
    ],
  );
  assert.equal(
    compareFreshness(
      audit,
      { one: "1.2.4", two: "0.4.5" },
      { one: "1.2.3", two: "0.4.5" },
    )[0].status,
    "drift",
  );
  assert.equal(normalizeVersion("v1.2.3+build.4"), "1.2.3");
  assert.equal(normalizeVersion("latest"), null);
  assert.equal(
    latestStableReleaseTag([
      { tag_name: "bin-v1.1.5", draft: false, prerelease: false },
      { tag_name: "v2.5.0", draft: false, prerelease: false },
    ], ""),
    "v2.5.0",
  );
  assert.equal(
    latestStableReleaseTag([{ tag_name: "v2.5.0-rc.1", draft: false, prerelease: false }], ""),
    null,
  );
  assert.equal(latestStableReleaseTag([{ tag_name: "bin-v1.1.6", draft: false, prerelease: false }], ""), null);
  assert.equal(latestStableReleaseTag([{ tag_name: "v2.5.0" }], ""), null);
  assert.deepEqual(
    parseArgs([
      "node",
      "optional-tools-freshness.mjs",
      "--check",
      "--json",
      "--checkpoint",
      "B",
      "--release-sha",
      "0123456789abcdef0123456789abcdef01234567",
    ]),
    {
      check: true,
      json: true,
      checkpoint: "B",
      releaseSha: "0123456789abcdef0123456789abcdef01234567",
      selfTest: false,
    },
  );
}

async function main(argv = process.argv) {
  const args = parseArgs(argv);
  if (args.selfTest) {
    selfTest();
    process.stdout.write("optional-tools-freshness: self-test ok\n");
    return;
  }
  await runFreshnessCheck(args);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`optional-tools-freshness: ${error.message}\n`);
    process.exitCode = 1;
  });
}
