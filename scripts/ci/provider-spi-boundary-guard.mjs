#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const SPI_MANIFEST = "crates/prodex-provider-spi/Cargo.toml";
const SPI_SRC_DIR = "crates/prodex-provider-spi/src";
const SPI_LIB = "crates/prodex-provider-spi/src/lib.rs";
const ALLOWED_DEPENDENCIES = new Set(["prodex_domain", "prodex_provider_core"]);
const ALLOWED_DEV_DEPENDENCIES = new Set([]);
const FORBIDDEN_DEPENDENCIES = new Set([
  "anyhow",
  "axum",
  "clap",
  "hyper",
  "reqwest",
  "rusqlite",
  "sqlx",
  "tokio",
  "tower",
  "tungstenite",
]);
const FORBIDDEN_SOURCE_PATTERNS = Object.freeze([
  { name: "filesystem", pattern: /\bstd\s*::\s*fs\b/u },
  { name: "environment", pattern: /\bstd\s*::\s*env\b/u },
  { name: "network", pattern: /\bstd\s*::\s*net\b/u },
  { name: "process", pattern: /\bstd\s*::\s*process\b/u },
  { name: "async runtime", pattern: /\btokio\s*::/u },
  { name: "http framework", pattern: /\b(axum|hyper|tower)\s*::/u },
  { name: "http client", pattern: /\breqwest\s*::/u },
  { name: "database", pattern: /\b(rusqlite|sqlx|postgres|redis)\s*::/u },
  { name: "transport", pattern: /\btungstenite\s*::/u },
  { name: "provider SDK", pattern: /\b(openai|anthropic|gemini|copilot)\s*::/u },
]);
const REQUIRED_SOURCE_SNIPPETS = Object.freeze([
  "pub enum ProviderRetryDecisionStatus",
  "pub struct ProviderRetryDecisionResponsePlan",
  "pub fn plan_provider_retry_decision_response(",
  "ProviderRetryDecision::Allowed => None",
  "code: \"provider_retry_not_safe\"",
  "message: \"provider retry is not safe\"",
  "code: \"provider_retry_budget_exhausted\"",
  "message: \"provider retry budget is exhausted\"",
]);

function stripComment(line) {
  let inString = false;
  let escaped = false;
  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (char === "\\") {
      escaped = true;
      continue;
    }
    if (char === '"') {
      inString = !inString;
      continue;
    }
    if (char === "#" && !inString) {
      return line.slice(0, index).trim();
    }
  }
  return line.trim();
}

export function parseDependencySections(tomlText) {
  const sections = new Map();
  let currentSection = null;
  for (const rawLine of tomlText.split(/\r?\n/u)) {
    const line = stripComment(rawLine);
    if (!line) continue;
    const sectionMatch = line.match(/^\[([^\]]+)\]$/u);
    if (sectionMatch) {
      currentSection = sectionMatch[1];
      if (!sections.has(currentSection)) sections.set(currentSection, new Set());
      continue;
    }
    if (!currentSection) continue;
    const depMatch = line.match(/^([A-Za-z0-9_-]+)\s*=/u);
    if (depMatch) {
      sections.get(currentSection).add(depMatch[1]);
    }
  }
  return sections;
}

function sorted(values) {
  return [...values].sort((left, right) => left.localeCompare(right));
}

export function validateProviderSpiManifest(tomlText, manifestPath = SPI_MANIFEST) {
  const sections = parseDependencySections(tomlText);
  const errors = [];
  const dependencies = sections.get("dependencies") ?? new Set();
  const devDependencies = sections.get("dev-dependencies") ?? new Set();

  for (const dep of dependencies) {
    if (!ALLOWED_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dependencies] must stay transport-neutral; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-provider-spi cannot depend on forbidden runtime/framework crate '${dep}'`);
    }
  }

  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dev-dependencies] must stay empty; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-provider-spi tests cannot depend on forbidden runtime/framework crate '${dep}'`);
    }
  }

  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${manifestPath}: target-specific dependencies are not allowed in prodex-provider-spi`);
    }
  }

  return errors;
}

export function validateProviderSpiSource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: prodex-provider-spi cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
  if (sourcePath === SPI_LIB) {
    if (!sourceText.includes("#![forbid(unsafe_code)]")) {
      errors.push(`${sourcePath}: prodex-provider-spi crate root must forbid unsafe code`);
    }
    for (const snippet of REQUIRED_SOURCE_SNIPPETS) {
      if (!sourceText.includes(snippet)) {
        errors.push(`${sourcePath}: missing required provider SPI contract '${snippet}'`);
      }
    }
  }
  return errors;
}

async function rustFilesUnder(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await rustFilesUnder(fullPath)));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      files.push(fullPath);
    }
  }
  return files;
}

async function validateProviderSpiSources() {
  const srcRoot = path.join(repoRoot, SPI_SRC_DIR);
  const files = await rustFilesUnder(srcRoot);
  const errors = [];
  for (const file of files) {
    const source = await fs.readFile(file, "utf8");
    const relative = path.relative(repoRoot, file);
    errors.push(...validateProviderSpiSource(source, relative));
  }
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
  const valid = `
[package]
name = "prodex-provider-spi"

[dependencies]
prodex_domain = { workspace = true }
prodex_provider_core = { workspace = true }
`;
  assertSelfTest(validateProviderSpiManifest(valid, "valid/Cargo.toml").length === 0, "valid manifest rejected");

  const invalidRuntime = `${valid}\nreqwest = "0.12"\n`;
  assertSelfTest(
    validateProviderSpiManifest(invalidRuntime, "invalid/Cargo.toml").some((error) => error.includes("reqwest")),
    "forbidden runtime dependency accepted",
  );

  const invalidDomainOnly = `${valid}\nserde_json = "1"\n`;
  assertSelfTest(
    validateProviderSpiManifest(invalidDomainOnly, "invalid-extra/Cargo.toml").some((error) => error.includes("serde_json")),
    "extra dependency accepted",
  );

  const invalidTarget = `${valid}\n[target.'cfg(unix)'.dependencies]\ntokio = "1"\n`;
  assertSelfTest(
    validateProviderSpiManifest(invalidTarget, "invalid-target/Cargo.toml").some((error) => error.includes("target-specific")),
    "target-specific dependency accepted",
  );

  assertSelfTest(
    validateProviderSpiSource("use std::net::TcpStream;", "bad.rs").some((error) => error.includes("network")),
    "network source boundary accepted",
  );
  assertSelfTest(
    validateProviderSpiSource("let client = reqwest::Client::new();", "bad.rs").some((error) => error.includes("http client")),
    "http client source boundary accepted",
  );
  assertSelfTest(
    validateProviderSpiSource("use std::fmt;\nuse prodex_domain::TenantId;", "good.rs").length === 0,
    "safe source rejected",
  );
  const validRetryResponse = `
pub enum ProviderRetryDecisionStatus {}
pub struct ProviderRetryDecisionResponsePlan {}
pub fn plan_provider_retry_decision_response() {
    ProviderRetryDecision::Allowed => None;
    code: "provider_retry_not_safe";
    message: "provider retry is not safe";
    code: "provider_retry_budget_exhausted";
    message: "provider retry budget is exhausted";
}
`;
  assertSelfTest(
    validateProviderSpiSource(`#![forbid(unsafe_code)]\n${validRetryResponse}`, SPI_LIB).length === 0,
    "valid provider retry response contract rejected",
  );
  assertSelfTest(
    validateProviderSpiSource(validRetryResponse, SPI_LIB).some((error) => error.includes("forbid unsafe code")),
    "crate root without unsafe forbid accepted",
  );
  assertSelfTest(
    validateProviderSpiSource(
      `#![forbid(unsafe_code)]\n${validRetryResponse.replace('message: "provider retry is not safe";', "")}`,
      SPI_LIB,
    ).some((error) => error.includes("provider retry is not safe")),
    "missing provider retry safe response accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }

  const manifestPath = path.join(repoRoot, SPI_MANIFEST);
  const manifest = await fs.readFile(manifestPath, "utf8");
  const errors = [...validateProviderSpiManifest(manifest), ...(await validateProviderSpiSources())];
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`provider-spi-boundary-guard: ${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
