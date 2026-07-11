#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const DOMAIN_MANIFEST = "crates/prodex-domain/Cargo.toml";
const DOMAIN_SRC_DIR = "crates/prodex-domain/src";
const DOMAIN_OBSERVABILITY = "crates/prodex-domain/src/observability.rs";
const DOMAIN_HEALTH = "crates/prodex-domain/src/health.rs";
const DOMAIN_SECRETS = "crates/prodex-domain/src/secrets.rs";
const DOMAIN_LIB = "crates/prodex-domain/src/lib.rs";
const ALLOWED_DEPENDENCIES = new Set(["serde", "uuid"]);
const ALLOWED_DEV_DEPENDENCIES = new Set(["serde_json"]);
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
  { name: "database", pattern: /\b(rusqlite|sqlx)\s*::/u },
  { name: "cli", pattern: /\bclap\s*::/u },
  { name: "transport", pattern: /\btungstenite\s*::/u },
  { name: "provider SDK", pattern: /\b(openai|anthropic|gemini|copilot)\s*::/u },
]);
const REQUIRED_OBSERVABILITY_SNIPPETS = Object.freeze([
  "pub fn tenant_trace_attribute(tenant_id: TenantId) -> TelemetryAttribute",
  "TelemetryAttribute::trace_only(\"tenant_id\", tenant_id.to_string())",
]);
const REQUIRED_HEALTH_SNIPPETS = Object.freeze([
  "pub active_policy_revision: Option<PolicyRevisionId>",
  "active_policy_revision: snapshot.active_policy_revision",
]);
const REQUIRED_SECRET_SNIPPETS = Object.freeze([
  "pub struct SecretRef",
  "pub fn is_well_formed(&self) -> bool",
  "fn secret_ref_part_is_well_formed(value: &str) -> bool",
  'f.write_str("<redacted-secret-ref>")',
]);
const REQUIRED_LIB_SNIPPETS = Object.freeze(["#![forbid(unsafe_code)]", "tenant_trace_attribute"]);

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

export function validateDomainManifest(tomlText, manifestPath = DOMAIN_MANIFEST) {
  const sections = parseDependencySections(tomlText);
  const errors = [];
  const dependencies = sections.get("dependencies") ?? new Set();
  const devDependencies = sections.get("dev-dependencies") ?? new Set();

  for (const dep of dependencies) {
    if (!ALLOWED_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dependencies] must stay pure; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-domain cannot depend on forbidden runtime/framework crate '${dep}'`);
    }
  }

  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dev-dependencies] must stay narrow; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-domain tests cannot depend on forbidden runtime/framework crate '${dep}'`);
    }
  }

  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${manifestPath}: target-specific dependencies are not allowed in prodex-domain`);
    }
  }

  return errors;
}

export function validateDomainSource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: prodex-domain cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
  return errors;
}

export function validateDomainRequiredContracts(sourceText, sourcePath = "source.rs") {
  const required = sourcePath === DOMAIN_OBSERVABILITY
    ? REQUIRED_OBSERVABILITY_SNIPPETS
    : sourcePath === DOMAIN_HEALTH
      ? REQUIRED_HEALTH_SNIPPETS
    : sourcePath === DOMAIN_SECRETS
      ? REQUIRED_SECRET_SNIPPETS
    : sourcePath === DOMAIN_LIB
      ? REQUIRED_LIB_SNIPPETS
      : [];
  const errors = [];
  for (const snippet of required) {
    if (!sourceText.includes(snippet)) {
      errors.push(`${sourcePath}: missing required domain boundary contract '${snippet}'`);
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

async function validateDomainSources() {
  const srcRoot = path.join(repoRoot, DOMAIN_SRC_DIR);
  const files = await rustFilesUnder(srcRoot);
  const errors = [];
  for (const file of files) {
    const source = await fs.readFile(file, "utf8");
    const relative = path.relative(repoRoot, file);
    errors.push(...validateDomainSource(source, relative));
    errors.push(...validateDomainRequiredContracts(source, relative));
  }
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
  const valid = `
[package]
name = "prodex-domain"

[dependencies]
serde = { workspace = true }
uuid = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
`;
  assertSelfTest(validateDomainManifest(valid, "valid/Cargo.toml").length === 0, "valid manifest rejected");

  const invalidRuntime = `${valid}\nreqwest = "0.12"\n`;
  assertSelfTest(
    validateDomainManifest(invalidRuntime, "invalid/Cargo.toml").some((error) => error.includes("reqwest")),
    "forbidden runtime dependency accepted",
  );

  const invalidTarget = `${valid}\n[target.'cfg(unix)'.dependencies]\ntokio = "1"\n`;
  assertSelfTest(
    validateDomainManifest(invalidTarget, "invalid-target/Cargo.toml").some((error) => error.includes("target-specific")),
    "target-specific dependency accepted",
  );

  assertSelfTest(
    validateDomainSource("use std::fs;", "bad.rs").some((error) => error.includes("filesystem")),
    "filesystem source boundary accepted",
  );
  assertSelfTest(
    validateDomainSource("let client = reqwest::Client::new();", "bad.rs").some((error) => error.includes("http client")),
    "http client source boundary accepted",
  );
  assertSelfTest(
    validateDomainSource("use std::fmt;\nuse serde::Serialize;", "good.rs").length === 0,
    "safe source rejected",
  );
  assertSelfTest(
    validateDomainRequiredContracts(
      `
pub fn tenant_trace_attribute(tenant_id: TenantId) -> TelemetryAttribute {
    TelemetryAttribute::trace_only("tenant_id", tenant_id.to_string())
}
`,
      DOMAIN_OBSERVABILITY,
    ).length === 0,
    "tenant trace contract rejected",
  );
  assertSelfTest(
    validateDomainRequiredContracts("pub fn tenant_trace_attribute() {}", DOMAIN_OBSERVABILITY).some((error) =>
      error.includes("trace_only"),
    ),
    "missing tenant trace-only contract accepted",
  );
  assertSelfTest(
    validateDomainRequiredContracts(
      `
pub struct HealthProbeResponsePlan {
    pub active_policy_revision: Option<PolicyRevisionId>,
}
pub fn plan_health_probe_response(snapshot: HealthSnapshot) -> HealthProbeResponsePlan {
    active_policy_revision: snapshot.active_policy_revision
}
`,
      DOMAIN_HEALTH,
    ).length === 0,
    "health active policy revision contract rejected",
  );
  assertSelfTest(
    validateDomainRequiredContracts("pub struct HealthProbeResponsePlan {}", DOMAIN_HEALTH).some((error) =>
      error.includes("active_policy_revision"),
    ),
    "missing health active policy revision contract accepted",
  );
  assertSelfTest(
    validateDomainRequiredContracts(
      `
pub struct SecretRef {}
impl SecretRef {
    pub fn is_well_formed(&self) -> bool { true }
}
fn secret_ref_part_is_well_formed(value: &str) -> bool { !value.is_empty() }
impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted-secret-ref>")
    }
}
`,
      DOMAIN_SECRETS,
    ).length === 0,
    "secret reference contract rejected",
  );
  assertSelfTest(
    validateDomainRequiredContracts("pub struct SecretRef {}", DOMAIN_SECRETS).some((error) =>
      error.includes("redacted-secret-ref"),
    ),
    "missing secret reference redaction contract accepted",
  );
  assertSelfTest(
    validateDomainRequiredContracts(
      "#![forbid(unsafe_code)]\npub use observability::{ tenant_trace_attribute };",
      DOMAIN_LIB,
    ).length === 0,
    "tenant trace export rejected",
  );
  assertSelfTest(
    validateDomainRequiredContracts("pub use observability::{ tenant_trace_attribute };", DOMAIN_LIB).some((error) =>
      error.includes("forbid(unsafe_code)"),
    ),
    "missing domain unsafe forbid accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }

  const manifestPath = path.join(repoRoot, DOMAIN_MANIFEST);
  const manifest = await fs.readFile(manifestPath, "utf8");
  const errors = [...validateDomainManifest(manifest), ...(await validateDomainSources())];
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`domain-boundary-guard: ${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
