#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseDependencySections } from "./boundary-guard-utils.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const GATEWAY_CORE_MANIFEST = "crates/prodex-gateway-core/Cargo.toml";
const GATEWAY_CORE_SRC_DIR = "crates/prodex-gateway-core/src";
const GATEWAY_CORE_LIB = "crates/prodex-gateway-core/src/lib.rs";
const ALLOWED_DEPENDENCIES = new Set([
  "prodex_authz",
  "prodex_domain",
  "prodex_observability",
  "prodex_provider_core",
  "prodex_provider_spi",
  "prodex_storage",
  "serde",
  "sha2",
]);
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
  { name: "blocking lock", pattern: /\bstd\s*::\s*sync\s*::\s*(Arc\s*,\s*)?Mutex\b/u },
  { name: "async runtime", pattern: /\btokio\s*::/u },
  { name: "http framework", pattern: /\b(axum|hyper|tower)\s*::/u },
  { name: "http client", pattern: /\breqwest\s*::/u },
  { name: "database", pattern: /\b(rusqlite|sqlx|postgres|redis)\s*::/u },
  { name: "transport", pattern: /\btungstenite\s*::/u },
  { name: "provider SDK", pattern: /\b(openai|anthropic|gemini|copilot)\s*::/u },
]);

function sorted(values) {
  return [...values].sort((left, right) => left.localeCompare(right));
}

export function validateGatewayCoreManifest(tomlText, manifestPath = GATEWAY_CORE_MANIFEST) {
  const sections = parseDependencySections(tomlText);
  const errors = [];
  const dependencies = sections.get("dependencies") ?? new Set();
  const devDependencies = sections.get("dev-dependencies") ?? new Set();

  for (const dep of dependencies) {
    if (!ALLOWED_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dependencies] must stay HTTP-neutral boundary crates only; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-gateway-core cannot depend on forbidden runtime/framework/storage-driver crate '${dep}'`);
    }
  }

  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dev-dependencies] must stay empty; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-gateway-core tests cannot depend on forbidden runtime/framework crate '${dep}'`);
    }
  }

  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${manifestPath}: target-specific dependencies are not allowed in prodex-gateway-core`);
    }
  }

  return errors;
}

export function validateGatewayCoreSource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  if (sourcePath === GATEWAY_CORE_LIB && !sourceText.includes("#![forbid(unsafe_code)]")) {
    errors.push(`${sourcePath}: missing required core boundary contract '#![forbid(unsafe_code)]'`);
  }
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: prodex-gateway-core cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
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

async function validateGatewayCoreSources() {
  const srcRoot = path.join(repoRoot, GATEWAY_CORE_SRC_DIR);
  const files = await rustFilesUnder(srcRoot);
  const errors = [];
  for (const file of files) {
    const source = await fs.readFile(file, "utf8");
    errors.push(...validateGatewayCoreSource(source, path.relative(repoRoot, file)));
  }
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
  const valid = `
[package]
name = "prodex-gateway-core"

[dependencies]
prodex_authz = { workspace = true }
prodex_domain = { workspace = true }
prodex_observability = { workspace = true }
prodex_provider_core = { workspace = true }
prodex_provider_spi = { workspace = true }
prodex_storage = { workspace = true }
serde = { workspace = true }
sha2 = { workspace = true }
`;
  assertSelfTest(validateGatewayCoreManifest(valid, "valid/Cargo.toml").length === 0, "valid manifest rejected");
  assertSelfTest(
    validateGatewayCoreManifest(`${valid}\naxum = "0.8"\n`, "invalid/Cargo.toml").some((error) => error.includes("axum")),
    "forbidden HTTP dependency accepted",
  );
  assertSelfTest(
    validateGatewayCoreManifest(`${valid}\nserde_json = "1"\n`, "invalid-extra/Cargo.toml").some((error) => error.includes("serde_json")),
    "extra dependency accepted",
  );
  assertSelfTest(
    validateGatewayCoreManifest(`${valid}\n[target.'cfg(unix)'.dependencies]\ntokio = "1"\n`, "invalid-target/Cargo.toml").some((error) => error.includes("target-specific")),
    "target-specific dependency accepted",
  );
  assertSelfTest(
    validateGatewayCoreSource("use std::net::TcpStream;", "bad.rs").some((error) => error.includes("network")),
    "network source boundary accepted",
  );
  assertSelfTest(
    validateGatewayCoreSource("let router = axum::Router::new();", "bad.rs").some((error) => error.includes("http framework")),
    "HTTP framework source boundary accepted",
  );
  assertSelfTest(
    validateGatewayCoreSource("use std::fmt;\nuse prodex_domain::TenantContext;", "good.rs").length === 0,
    "safe source rejected",
  );
  assertSelfTest(
    validateGatewayCoreSource("#![forbid(unsafe_code)]\nuse std::fmt;", GATEWAY_CORE_LIB).length === 0,
    "gateway-core unsafe forbid rejected",
  );
  assertSelfTest(
    validateGatewayCoreSource("use std::fmt;", GATEWAY_CORE_LIB).some((error) => error.includes("forbid(unsafe_code)")),
    "missing gateway-core unsafe forbid accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }

  const manifestPath = path.join(repoRoot, GATEWAY_CORE_MANIFEST);
  const manifest = await fs.readFile(manifestPath, "utf8");
  const errors = [...validateGatewayCoreManifest(manifest), ...(await validateGatewayCoreSources())];
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`gateway-core-boundary-guard: ${message}\n`);
  process.exitCode = 1;
});
