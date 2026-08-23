#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { runCheckedJson } from "../lib/checked-subprocess.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const write = process.argv.includes("--write");
const outPath = resolve(root, "docs/provider-capabilities.md");
const conformancePath = resolve(root, "crates/prodex-provider-core/tests/fixtures/provider_conformance_cases.json");
const conformance = JSON.parse(readFileSync(conformancePath, "utf8"));
const contractCatalog = runCheckedJson(
  "cargo",
  ["run", "--locked", "-q", "-p", "prodex-provider-core", "--example", "provider-contract-matrix"],
  { cwd: root, timeoutMs: 120_000 },
);
const contracts = contractCatalog.providers;
const harnessModes = contractCatalog.harness_modes;
const catalog = runCheckedJson(
  process.execPath,
  ["scripts/catalog/provider-catalog-check.mjs", "--json"],
  { cwd: root },
);

const endpointColumns = [
  "responses",
  "responses/compact",
  "chat-completions",
  "messages",
  "models",
  "embeddings",
  "images",
  "audio",
  "batches",
  "rerank",
  "a2a",
];
const providerCountKeys = {
  openai: "OpenAi",
  anthropic: "Anthropic",
  copilot: "Copilot",
  deepseek: "DeepSeek",
  gemini: "Gemini",
  kiro: "Kiro",
  local: "Local",
};
const claimedStatuses = new Set(["native", "passthrough", "translated"]);

function endpointStatus(contract, endpoint) {
  return contract.endpoint_status.find((item) => item.endpoint === endpoint)?.status ?? "unsupported";
}

function endpointFixtures(provider, endpoint) {
  const fixtures = { request: 0, response: 0, stream: 0 };
  for (const item of conformance) {
    if (item.provider !== provider || item.endpoint !== endpoint) continue;
    if (item.operation === "request") fixtures.request += 1;
    else if (item.operation === "response") fixtures.response += 1;
    else if (item.operation === "stream-event") fixtures.stream += 1;
  }
  return fixtures;
}

function providerFixtureSummary(provider) {
  const byOp = { request: 0, response: 0, stream: 0 };
  for (const item of conformance) {
    if (item.provider !== provider) continue;
    if (item.operation === "request") byOp.request += 1;
    else if (item.operation === "response") byOp.response += 1;
    else if (item.operation === "stream-event") byOp.stream += 1;
  }
  return `${byOp.request}/${byOp.response}/${byOp.stream}`;
}

function providerHasNonLosslessFixture(provider) {
  return conformance.some(
    (item) => item.provider === provider && item.expected_loss && item.expected_loss !== "lossless",
  );
}

function providerHasErrorFixture(provider) {
  return conformance.some(
    (item) => item.provider === provider && item.expected_error_class,
  );
}

function validateContractCoverage(contracts) {
  const issues = [];
  for (const contract of contracts) {
    const responsesEndpoint = contract.endpoint_status.find((item) => item.endpoint === "responses");
    if (contract.transform_status === "translated") {
      if (!providerHasNonLosslessFixture(contract.provider)) {
        issues.push(
          `${contract.provider} is translated but has no degraded/rejected/unsupported fixture`,
        );
      }
      if (!providerHasErrorFixture(contract.provider)) {
        issues.push(
          `${contract.provider} is translated but has no explicit error-mapping fixture`,
        );
      }
      if (!responsesEndpoint?.unsupported_params?.length) {
        issues.push(
          `${contract.provider} is translated but does not declare known responses parameter limitations`,
        );
      }
    }
    for (const endpoint of contract.endpoint_status) {
      if (!claimedStatuses.has(endpoint.status)) continue;
      const fixtures = endpointFixtures(contract.provider, endpoint.endpoint);
      if (fixtures.request === 0) {
        issues.push(
          `${contract.provider} ${endpoint.endpoint} claims ${endpoint.status} but has no request fixture`,
        );
      }
      if (fixtures.response === 0) {
        issues.push(
          `${contract.provider} ${endpoint.endpoint} claims ${endpoint.status} but has no response fixture`,
        );
      }
      if (
        endpoint.endpoint === "responses" &&
        contract.supports_streaming &&
        fixtures.stream === 0
      ) {
        issues.push(
          `${contract.provider} ${endpoint.endpoint} claims ${endpoint.status} with streaming but has no stream fixture`,
        );
      }
    }
  }
  return issues;
}

function render() {
  const lines = [
    "# Provider Capabilities",
    "",
    "Generated from `prodex_provider_core::provider_contract_catalog()`, `crates/prodex-provider-core/tests/fixtures/provider_conformance_cases.json`, and `crates/prodex-provider-core/catalog/models.json`.",
    "",
    "| Provider | Models | Transform | Streaming | Fallback | Fixtures req/resp/stream | " + endpointColumns.join(" | ") + " |",
    "|---|---:|---|---|---|---|" + endpointColumns.map(() => "---").join("|") + "|",
  ];
  for (const contract of contracts) {
    const providerKey = providerCountKeys[contract.provider];
    const modelCount = catalog.providers[providerKey] ?? 0;
    lines.push(
      [
        contract.provider,
        modelCount,
        contract.transform_status,
        String(contract.supports_streaming),
        String(contract.supports_model_fallback),
        providerFixtureSummary(contract.provider),
        ...endpointColumns.map((endpoint) => endpointStatus(contract, endpoint)),
      ].join(" | ").replace(/^/, "| ") + " |",
    );
  }
  lines.push("");
  lines.push("Status values: `native`, `translated`, `passthrough`, `emulated`, `partial`, `untested`, `unsupported`.");
  lines.push("");
  lines.push("Fixture summary counts are `request/response/stream-event` conformance cases per provider.");
  lines.push("");
  lines.push("Model counts cover deterministic offline built-ins. Imported or provider-discovered runtime routes may augment them, and Super accepts an explicit non-empty custom child model ID without requiring live discovery.");
  lines.push("");
  lines.push("## Harness modes");
  lines.push("");
  lines.push(`Default mode: \`${contractCatalog.default_harness_mode}\`. Resolved mode for this catalog: \`${contractCatalog.resolved_harness_mode}\`.`);
  lines.push("");
  lines.push("| Mode | Label | Selectable | Default effective | Canonical request routes | Request shaping | Response shaping | Stream shaping | Description |");
  lines.push("|---|---|---|---|---|---|---|---|---|");
  for (const mode of harnessModes) {
    lines.push(`| ${mode.id} | ${mode.display_label} | ${mode.selectable} | ${mode.default_effective_mode} | ${mode.supported_canonical_request_routes.join(", ")} | ${mode.request_shaping} | ${mode.response_shaping} | ${mode.stream_shaping} | ${mode.description} |`);
  }
  lines.push("");
  const translatedLimitations = contracts
    .map((contract) => {
      const responses = contract.endpoint_status.find((item) => item.endpoint === "responses");
      const unsupported = responses?.unsupported_params ?? [];
      return unsupported.length > 0 ? [contract.provider, unsupported] : null;
    })
    .filter(Boolean);
  if (translatedLimitations.length > 0) {
    lines.push("## Declared Responses parameter limitations");
    lines.push("");
    for (const [provider, unsupported] of translatedLimitations) {
      lines.push(`- \`${provider}\`: ${unsupported.map((field) => `\`${field}\``).join(", ")}`);
    }
    lines.push("");
  }
  lines.push("## Semantic compact observability");
  lines.push("");
  lines.push("Gemini and Kiro semantic compact responses expose `x-prodex-compact-mode` (`semantic` or `local-fallback`) and `x-prodex-compact-provider`. Lossy fallback also exposes `x-prodex-compact-degraded: true` plus a bounded `x-prodex-compact-reason` code: `timeout`, `unsupported`, `unavailable`, `invalid-response`, `provider-error`, or `local-policy`. Raw upstream errors are never copied into headers.");
  lines.push("");
  lines.push("Prometheus output includes `prodex_semantic_compact_total{provider,mode}` and `prodex_semantic_compact_fallback_total{provider,reason}` with fixed-cardinality labels. Local fallback preserves HTTP 200 for continuation compatibility but is not semantic success. It is intentionally lossy and retains at most 24 recent snippets, 768 bytes per snippet, and 24 KiB total.");
  lines.push("");
  lines.push("## Transport limits");
  lines.push("");
  lines.push("Capability labels describe documented HTTP/text transformations, not lossless equivalence. Translated or emulated shapes may reject unsupported fields as listed above. Gemini Live rejects unexpected upstream binary WebSocket frames predictably; it does not reinterpret them as text.");
  lines.push("");
  return `${lines.join("\n")}`;
}

const next = render();
const coverageIssues = validateContractCoverage(contracts);
if (coverageIssues.length > 0) {
  for (const issue of coverageIssues) {
    console.error(issue);
  }
  process.exitCode = 1;
}
if (write) {
  writeFileSync(outPath, next);
} else {
  const current = readFileSync(outPath, "utf8");
  if (current !== next) {
    console.error("provider capability matrix is stale; run node scripts/catalog/provider-capability-matrix.mjs --write");
    process.exitCode = 1;
  }
}
