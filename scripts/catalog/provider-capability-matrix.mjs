#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const write = process.argv.includes("--write");
const outPath = resolve(root, "docs/provider-capabilities.md");
const fixturePath = resolve(
  root,
  "crates/prodex-provider-core/tests/fixtures/provider_contracts.json",
);
const contracts = JSON.parse(readFileSync(fixturePath, "utf8"));
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
  if (!contract.required_endpoints.includes(endpoint)) {
    return "unsupported";
  }
  if (contract.transform_status === "passthrough") {
    return contract.provider === "openai" ? "native" : "passthrough";
  }
  return "translated";
}

function render() {
  const lines = [
    "# Provider Capabilities",
    "",
    "Generated from `crates/prodex-provider-core/tests/fixtures/provider_contracts.json`, which is checked against `ProviderAdapterContract`, and `scripts/catalog/provider-catalog-check.mjs`.",
    "",
    "| Provider | Models | Transform | Streaming | Fallback | " +
      endpointColumns.join(" | ") +
      " |",
    "|---|---:|---|---|---|" + endpointColumns.map(() => "---").join("|") + "|",
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
        ...endpointColumns.map((endpoint) => endpointStatus(contract, endpoint)),
      ].join(" | ").replace(/^/, "| ") + " |",
    );
  }
  lines.push("");
  lines.push("Status values: `native`, `translated`, `passthrough`, `unsupported`.");
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
