#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const write = process.argv.includes("--write");
const outPath = resolve(root, "docs/provider-capabilities.md");
const conformancePath = resolve(root, "crates/prodex-provider-core/tests/fixtures/provider_conformance_cases.json");
const conformance = JSON.parse(readFileSync(conformancePath, "utf8"));
const contracts = JSON.parse(
  spawnSync(
    "cargo",
    ["run", "-q", "-p", "prodex-provider-core", "--example", "provider-contract-matrix"],
    {
      cwd: root,
      encoding: "utf8",
    },
  ).stdout,
);
const catalog = JSON.parse(
  spawnSync(process.execPath, ["scripts/catalog/provider-catalog-check.mjs", "--json"], {
    cwd: root,
    encoding: "utf8",
  }).stdout,
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
const v1TranslatedProviders = new Set(["deepseek", "gemini"]);

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
    if (contract.transform_status === "translated" && v1TranslatedProviders.has(contract.provider)) {
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
    "Generated from `prodex_provider_core::provider_adapter_contract_matrix()`, `crates/prodex-provider-core/tests/fixtures/provider_conformance_cases.json`, and `crates/prodex-provider-core/catalog/models.json`.",
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
    console.error("provider capability matrix is stale; run npm run docs:provider-capabilities");
    process.exitCode = 1;
  }
}
