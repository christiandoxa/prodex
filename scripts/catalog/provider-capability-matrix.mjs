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
  local: "Local",
};

function endpointStatus(contract, endpoint) {
  return contract.endpoint_status.find((item) => item.endpoint === endpoint)?.status ?? "unsupported";
}

function providerFixtureSummary(provider) {
  const providerCases = conformance.filter((item) => item.provider === provider);
  const byOp = { request: 0, response: 0, stream: 0 };
  for (const item of providerCases) {
    if (item.operation === "request") byOp.request += 1;
    else if (item.operation === "response") byOp.response += 1;
    else if (item.operation === "stream-event") byOp.stream += 1;
  }
  return `${byOp.request}/${byOp.response}/${byOp.stream}`;
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
  lines.push("Status values: `native`, `translated`, `passthrough`, `unsupported`, `partial`, `untested`.");
  lines.push("");
  lines.push("Fixture summary counts are `request/response/stream-event` conformance cases per provider.");
  lines.push("");
  return `${lines.join("\n")}`;
}

const next = render();
if (write) {
  writeFileSync(outPath, next);
} else {
  const current = readFileSync(outPath, "utf8");
  if (current !== next) {
    console.error("provider capability matrix is stale; run npm run docs:provider-capabilities");
    process.exitCode = 1;
  }
}
