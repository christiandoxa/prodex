#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const sourcePath = resolve(root, "crates/prodex-provider-core/src/lib.rs");
const source = readFileSync(sourcePath, "utf8");
const json = process.argv.includes("--json");
const stringConstants = new Map(
  [...source.matchAll(/pub const ([A-Z0-9_]+): &str = "([^"]+)";/g)].map((match) => [
    match[1],
    match[2],
  ]),
);
const validEndpointSets = new Set(["OPENAI_ENDPOINTS", "CORE_TEXT_ENDPOINTS", "GEMINI_ENDPOINTS"]);

function collectModelCalls(text) {
  const calls = [];
  let searchFrom = 0;
  while (true) {
    const marker = text.indexOf("model!(", searchFrom);
    if (marker === -1) {
      return calls;
    }
    let index = marker + "model!(".length;
    let depth = 1;
    let inString = false;
    let escaped = false;
    let body = "";
    for (; index < text.length; index += 1) {
      const ch = text[index];
      if (inString) {
        body += ch;
        if (escaped) {
          escaped = false;
        } else if (ch === "\\") {
          escaped = true;
        } else if (ch === "\"") {
          inString = false;
        }
        continue;
      }
      if (ch === "\"") {
        inString = true;
        body += ch;
        continue;
      }
      if (ch === "(" || ch === "[" || ch === "{") {
        depth += 1;
      } else if (ch === ")" || ch === "]" || ch === "}") {
        depth -= 1;
        if (depth === 0) {
          break;
        }
      }
      body += ch;
    }
    calls.push(body);
    searchFrom = index + 1;
  }
}

function splitTopLevelArgs(text) {
  const args = [];
  let depth = 0;
  let inString = false;
  let escaped = false;
  let current = "";
  for (const ch of text) {
    if (inString) {
      current += ch;
      if (escaped) {
        escaped = false;
      } else if (ch === "\\") {
        escaped = true;
      } else if (ch === "\"") {
        inString = false;
      }
      continue;
    }
    if (ch === "\"") {
      inString = true;
      current += ch;
      continue;
    }
    if (ch === "(" || ch === "[" || ch === "{") {
      depth += 1;
    } else if (ch === ")" || ch === "]" || ch === "}") {
      depth -= 1;
    }
    if (ch === "," && depth === 0) {
      args.push(current.trim());
      current = "";
    } else {
      current += ch;
    }
  }
  if (current.trim()) {
    args.push(current.trim());
  }
  return args;
}

function rustString(value) {
  const trimmed = value.trim();
  if (!trimmed.startsWith("\"")) {
    return stringConstants.get(trimmed) ?? trimmed;
  }
  return JSON.parse(trimmed);
}

function aliasValues(value) {
  const trimmed = value.trim();
  if (!trimmed.startsWith("[") || !trimmed.endsWith("]")) {
    return [];
  }
  const inner = trimmed.slice(1, -1).trim();
  if (!inner) {
    return [];
  }
  return splitTopLevelArgs(inner).map(rustString);
}

const models = collectModelCalls(source).map((body, index) => {
  const args = splitTopLevelArgs(body);
  if (args.length !== 10) {
    throw new Error(`model! call ${index + 1} has ${args.length} args, expected 10`);
  }
  return {
    provider: args[0].split("::").pop(),
    ownedBy: rustString(args[1]),
    id: rustString(args[2]),
    displayName: rustString(args[3]),
    context: args[5],
    inputCost: args[6],
    outputCost: args[7],
    endpoints: args[8],
    aliases: aliasValues(args[9]),
  };
});

const issues = [];
const seenIds = new Map();
const seenNames = new Map();
const providerCounts = new Map();
for (const model of models) {
  providerCounts.set(model.provider, (providerCounts.get(model.provider) ?? 0) + 1);
  if (!model.id || model.id.includes(" ")) {
    issues.push(`invalid id for ${model.provider}: ${model.id}`);
  }
  if (!model.displayName || !model.ownedBy) {
    issues.push(`missing display/owner metadata for ${model.provider}:${model.id}`);
  }
  if (model.provider !== "Local" && model.context === "None") {
    issues.push(`missing context window for ${model.provider}:${model.id}`);
  }
  if (!validEndpointSets.has(model.endpoints)) {
    issues.push(`unknown endpoint set for ${model.provider}:${model.id}: ${model.endpoints}`);
  }
  const idKey = `${model.provider}:${model.id.toLowerCase()}`;
  if (seenIds.has(idKey)) {
    issues.push(`duplicate model id ${model.provider}:${model.id}`);
  }
  seenIds.set(idKey, model);
  for (const alias of model.aliases) {
    const aliasKey = `${model.provider}:${alias.toLowerCase()}`;
    if (seenNames.has(aliasKey)) {
      issues.push(`duplicate alias/name ${model.provider}:${alias}`);
    }
    seenNames.set(aliasKey, model);
  }
}

const summary = {
  source: sourcePath,
  models: models.length,
  providers: Object.fromEntries([...providerCounts.entries()].sort()),
  issues,
};

if (json) {
  console.log(JSON.stringify(summary, null, 2));
} else {
  console.log(
    `provider catalog: ${summary.models} models across ${Object.keys(summary.providers).length} providers`,
  );
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
