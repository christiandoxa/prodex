import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import test from "node:test";
import { promisify } from "node:util";

import { parseArgs, preflightSteps } from "./preflight.mjs";

const SCRIPT_PATH = new URL("./preflight.mjs", import.meta.url).pathname;
const execFileAsync = promisify(execFile);

test("preflight enables storage postgres proof from CLI flag", () => {
  const args = parseArgs(["node", "preflight.mjs", "--storage-postgres-proof"]);
  const labels = preflightSteps(args).map((step) => step.label);

  assert.equal(args.storagePostgresProof, true);
  assert.ok(labels.includes("storage-postgres-proof"));
});

test("preflight enables storage postgres proof from environment", () => {
  const previous = process.env.PRODEX_PREFLIGHT_STORAGE_POSTGRES_PROOF;
  process.env.PRODEX_PREFLIGHT_STORAGE_POSTGRES_PROOF = "1";
  try {
    const args = parseArgs(["node", "preflight.mjs"]);
    const labels = preflightSteps(args).map((step) => step.label);

    assert.equal(args.storagePostgresProof, true);
    assert.ok(labels.includes("storage-postgres-proof"));
  } finally {
    if (previous === undefined) {
      delete process.env.PRODEX_PREFLIGHT_STORAGE_POSTGRES_PROOF;
    } else {
      process.env.PRODEX_PREFLIGHT_STORAGE_POSTGRES_PROOF = previous;
    }
  }
});

test("preflight keeps storage postgres proof opt-in by default", () => {
  const previous = process.env.PRODEX_PREFLIGHT_STORAGE_POSTGRES_PROOF;
  delete process.env.PRODEX_PREFLIGHT_STORAGE_POSTGRES_PROOF;
  try {
    const args = parseArgs(["node", "preflight.mjs"]);
    const labels = preflightSteps(args).map((step) => step.label);

    assert.equal(args.storagePostgresProof, false);
    assert.ok(!labels.includes("storage-postgres-proof"));
  } finally {
    if (previous !== undefined) {
      process.env.PRODEX_PREFLIGHT_STORAGE_POSTGRES_PROOF = previous;
    }
  }
});

test("preflight runs runtime hotpath guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("runtime-hotpath-guard-self-test") >= 0);
  assert.ok(labels.indexOf("runtime-hotpath-guard") > labels.indexOf("runtime-hotpath-guard-self-test"));
});

test("preflight runs config boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("config-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("config-boundary-guard") > labels.indexOf("config-boundary-guard-self-test"));
});

test("preflight runs deployment security guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("deployment-security-guard-self-test") >= 0);
  assert.ok(labels.indexOf("deployment-security-guard") > labels.indexOf("deployment-security-guard-self-test"));
});

test("preflight runs domain boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("domain-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("domain-boundary-guard") > labels.indexOf("domain-boundary-guard-self-test"));
});

test("preflight runs application boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("application-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("application-boundary-guard") > labels.indexOf("application-boundary-guard-self-test"));
});

test("preflight runs auth boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("auth-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("auth-boundary-guard") > labels.indexOf("auth-boundary-guard-self-test"));
});

test("preflight runs control-plane boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("control-plane-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("control-plane-boundary-guard") > labels.indexOf("control-plane-boundary-guard-self-test"));
});

test("preflight runs observability boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("observability-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("observability-boundary-guard") > labels.indexOf("observability-boundary-guard-self-test"));
});

test("preflight runs gateway HTTP boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("gateway-http-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("gateway-http-boundary-guard") > labels.indexOf("gateway-http-boundary-guard-self-test"));
});

test("preflight runs gateway core boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("gateway-core-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("gateway-core-boundary-guard") > labels.indexOf("gateway-core-boundary-guard-self-test"));
});

test("preflight runs provider SPI boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("provider-spi-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("provider-spi-boundary-guard") > labels.indexOf("provider-spi-boundary-guard-self-test"));
});

test("preflight runs storage boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("storage-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("storage-boundary-guard") > labels.indexOf("storage-boundary-guard-self-test"));
});

test("preflight runs Postgres storage boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("storage-postgres-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("storage-postgres-boundary-guard") > labels.indexOf("storage-postgres-boundary-guard-self-test"));
});

test("preflight runs Redis storage boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("storage-redis-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("storage-redis-boundary-guard") > labels.indexOf("storage-redis-boundary-guard-self-test"));
});

test("preflight runs SQLite storage boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("storage-sqlite-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("storage-sqlite-boundary-guard") > labels.indexOf("storage-sqlite-boundary-guard-self-test"));
});

test("preflight runs enterprise docs guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("enterprise-docs-guard-self-test") >= 0);
  assert.ok(labels.indexOf("enterprise-docs-guard") > labels.indexOf("enterprise-docs-guard-self-test"));
});

test("preflight runs enterprise ID boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("enterprise-id-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("enterprise-id-boundary-guard") > labels.indexOf("enterprise-id-boundary-guard-self-test"));
});

test("preflight runs enterprise binaries guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("enterprise-binaries-guard-self-test") >= 0);
  assert.ok(labels.indexOf("enterprise-binaries-guard") > labels.indexOf("enterprise-binaries-guard-self-test"));
});

test("preflight runs crate boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("crate-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("crate-boundary-guard") > labels.indexOf("crate-boundary-guard-self-test"));
});

test("preflight dry-run prints storage postgres proof step when enabled", async () => {
  const { stdout } = await execFileAsync(process.execPath, [SCRIPT_PATH, "--dry-run", "--storage-postgres-proof"]);

  assert.match(stdout, /storage-postgres-proof: npm run ci:storage-postgres-proof/);
});
