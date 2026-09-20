import assert from "node:assert/strict";
import test from "node:test";

import { parseArgs, preflightSteps } from "./preflight.mjs";


test("preflight runs runtime hotpath guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("runtime-hotpath-guard-self-test") >= 0);
  assert.ok(labels.indexOf("runtime-hotpath-guard") > labels.indexOf("runtime-hotpath-guard-self-test"));
});

test("preflight enforces Mojo ownership and no-fallback guards", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.includes("mojo-ownership"));
  assert.ok(labels.includes("mojo-production-share"));
  assert.ok(labels.includes("mojo-authority-guard"));
  assert.ok(labels.includes("mojo-no-fallback"));
});

test("preflight runs domain boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("domain-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("domain-boundary-guard") > labels.indexOf("domain-boundary-guard-self-test"));
});

test("preflight runs production boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("production-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("production-boundary-guard") > labels.indexOf("production-boundary-guard-self-test"));
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

test("preflight runs crate boundary guard self-test before scanning workspace", () => {
  const labels = preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label);

  assert.ok(labels.indexOf("crate-boundary-guard-self-test") >= 0);
  assert.ok(labels.indexOf("crate-boundary-guard") > labels.indexOf("crate-boundary-guard-self-test"));
});

test("preflight runs clippy across the locked workspace", () => {
  const clippy = preflightSteps(parseArgs(["node", "preflight.mjs"])).find(
    (step) => step.label === "clippy",
  );

  assert.deepEqual(clippy, {
    label: "clippy",
    command: "cargo",
    args: ["clippy", "--locked", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
  });
});

test("preflight excludes retired enterprise implementation guards", () => {
  const labels = new Set(
    preflightSteps(parseArgs(["node", "preflight.mjs"])).map((step) => step.label),
  );
  for (const label of [
    "enterprise-binaries-guard",
    "application-boundary-guard",
    "auth-boundary-guard",
    "config-boundary-guard",
    "control-plane-boundary-guard",
    "observability-boundary-guard",
    "provider-spi-boundary-guard",
    "storage-boundary-guard",
    "storage-postgres-boundary-guard",
    "storage-redis-boundary-guard",
    "storage-sqlite-boundary-guard",
    "gateway-core-boundary-guard",
    "gateway-http-boundary-guard",
    "deployment-security-guard",
  ]) {
    assert.equal(labels.has(label), false, label);
    assert.equal(labels.has(label + "-self-test"), false, label + "-self-test");
  }
});
