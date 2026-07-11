#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const json = process.argv.includes("--json");
const sourceArg = process.argv.find((arg) => arg.startsWith("--source="));
const catalogPath = sourceArg
  ? resolve(root, sourceArg.slice("--source=".length))
  : resolve(root, "crates/prodex-provider-core/catalog/models.json");
const models = JSON.parse(readFileSync(catalogPath, "utf8"));
const requiredProviders = ["openai", "anthropic", "copilot", "deepseek", "gemini", "kiro", "local"];
const validEndpoints = new Set([
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
]);
const requiredFeatureFlags = [
  "tools",
  "json_schema",
  "vision",
  "audio",
  "web_search",
  "reasoning",
];
const providerCounts = new Map();
const issues = [];
const seenIds = new Set();
const seenAliases = new Set();

for (const model of models) {
  providerCounts.set(model.provider, (providerCounts.get(model.provider) ?? 0) + 1);
  if (!model.provider || !requiredProviders.includes(model.provider)) {
    issues.push(`unknown provider for model ${model.id}`);
  }
  if (!model.id || /\s/.test(model.id)) {
    issues.push(`invalid id for ${model.provider}:${model.id}`);
  }
  if (!model.owned_by || !model.display_name) {
    issues.push(`missing display/owner metadata for ${model.provider}:${model.id}`);
  }
  if (model.provider !== "local" && model.context_window_tokens == null) {
    issues.push(`missing context window for ${model.provider}:${model.id}`);
  }
  if (!Array.isArray(model.supported_endpoints) || model.supported_endpoints.length === 0) {
    issues.push(`missing endpoint list for ${model.provider}:${model.id}`);
  } else {
    for (const endpoint of model.supported_endpoints) {
      if (!validEndpoints.has(endpoint)) {
        issues.push(`unknown endpoint ${endpoint} for ${model.provider}:${model.id}`);
      }
    }
  }
  if (!model.feature_flags || typeof model.feature_flags !== "object" || Array.isArray(model.feature_flags)) {
    issues.push(`missing feature_flags for ${model.provider}:${model.id}`);
  } else {
    for (const key of requiredFeatureFlags) {
      if (typeof model.feature_flags[key] !== "boolean") {
        issues.push(`feature_flags.${key} must be boolean for ${model.provider}:${model.id}`);
      }
    }
  }
  if (typeof model.pricing_known !== "boolean") {
    issues.push(`pricing_known must be boolean for ${model.provider}:${model.id}`);
  } else if (model.pricing_known) {
    if (model.input_cost_per_million_microusd == null || model.output_cost_per_million_microusd == null) {
      issues.push(`pricing_known=true requires both input/output pricing for ${model.provider}:${model.id}`);
    }
  }
  const idKey = `${model.provider}:${String(model.id).toLowerCase()}`;
  if (seenIds.has(idKey)) {
    issues.push(`duplicate model id ${model.provider}:${model.id}`);
  }
  seenIds.add(idKey);
  for (const alias of model.aliases ?? []) {
    const aliasKey = `${model.provider}:${String(alias).toLowerCase()}`;
    if (seenAliases.has(aliasKey)) {
      issues.push(`duplicate alias ${model.provider}:${alias}`);
    }
    seenAliases.add(aliasKey);
  }
}

if (models.length === 0) {
  issues.push("model_count is 0");
}

for (const provider of requiredProviders) {
  if (!providerCounts.has(provider)) {
    issues.push(`required provider missing: ${provider}`);
  }
}

const summary = {
  source: catalogPath,
  sources: [catalogPath],
  models: models.length,
  model_count: models.length,
  provider_count: providerCounts.size,
  providers: {
    OpenAi: providerCounts.get("openai") ?? 0,
    Anthropic: providerCounts.get("anthropic") ?? 0,
    Copilot: providerCounts.get("copilot") ?? 0,
    DeepSeek: providerCounts.get("deepseek") ?? 0,
    Gemini: providerCounts.get("gemini") ?? 0,
    Kiro: providerCounts.get("kiro") ?? 0,
    Local: providerCounts.get("local") ?? 0,
  },
  issues,
};

if (json) {
  console.log(JSON.stringify(summary, null, 2));
} else {
  console.log(`provider catalog: ${summary.model_count} models across ${summary.provider_count} providers`);
  console.log(`  source: ${catalogPath}`);
  for (const [provider, count] of Object.entries(summary.providers)) {
    console.log(`  ${provider}: ${count}`);
  }
  if (issues.length > 0) {
    console.error("provider catalog issues:");
    for (const issue of issues) {
      console.error(`  - ${issue}`);
    }
  }
}

process.exitCode = issues.length > 0 ? 1 : 0;
