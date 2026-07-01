#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const MANIFEST = "crates/prodex-storage-postgres/Cargo.toml";
const SRC_DIR = "crates/prodex-storage-postgres/src";
const LIB = "crates/prodex-storage-postgres/src/lib.rs";
const ALLOWED_DEPENDENCIES = new Set(["prodex_domain", "prodex_storage"]);
const ALLOWED_DEV_DEPENDENCIES = new Set([]);
const FORBIDDEN_DEPENDENCIES = new Set([
  "anyhow",
  "axum",
  "clap",
  "hyper",
  "postgres",
  "redis",
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
  { name: "database driver", pattern: /\b(postgres|rusqlite|sqlx|redis)\s*::/u },
  { name: "transport", pattern: /\btungstenite\s*::/u },
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
    if (depMatch) sections.get(currentSection).add(depMatch[1]);
  }
  return sections;
}

function sorted(values) {
  return [...values].sort((left, right) => left.localeCompare(right));
}

export function validateManifest(tomlText, manifestPath = MANIFEST) {
  const sections = parseDependencySections(tomlText);
  const errors = [];
  const dependencies = sections.get("dependencies") ?? new Set();
  const devDependencies = sections.get("dev-dependencies") ?? new Set();

  for (const dep of dependencies) {
    if (!ALLOWED_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dependencies] must stay SQL-plan boundary crates only; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-storage-postgres cannot depend on forbidden driver/runtime/framework crate '${dep}'`);
    }
  }
  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dev-dependencies] must stay empty; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-storage-postgres tests cannot depend on forbidden driver/runtime/framework crate '${dep}'`);
    }
  }
  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${manifestPath}: target-specific dependencies are not allowed in prodex-storage-postgres`);
    }
  }
  return errors;
}

export function validateSource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: prodex-storage-postgres cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
  if (sourcePath === LIB && !sourceText.includes("#![forbid(unsafe_code)]")) {
    errors.push(`${sourcePath}: prodex-storage-postgres crate root must forbid unsafe code`);
  }
  return errors;
}

async function rustFilesUnder(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) files.push(...(await rustFilesUnder(fullPath)));
    else if (entry.isFile() && entry.name.endsWith(".rs")) files.push(fullPath);
  }
  return files;
}

async function validateSources() {
  const files = await rustFilesUnder(path.join(repoRoot, SRC_DIR));
  const errors = [];
  for (const file of files) {
    const source = await fs.readFile(file, "utf8");
    errors.push(...validateSource(source, path.relative(repoRoot, file)));
  }
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
  const valid = `
[package]
name = "prodex-storage-postgres"

[dependencies]
prodex_domain = { workspace = true }
prodex_storage = { workspace = true }
`;
  assertSelfTest(validateManifest(valid, "valid/Cargo.toml").length === 0, "valid manifest rejected");
  assertSelfTest(
    validateManifest(`${valid}\npostgres = "0.19"\n`, "invalid/Cargo.toml").some((error) => error.includes("postgres")),
    "database driver dependency accepted",
  );
  assertSelfTest(
    validateManifest(`${valid}\nserde_json = "1"\n`, "invalid-extra/Cargo.toml").some((error) => error.includes("serde_json")),
    "extra dependency accepted",
  );
  assertSelfTest(
    validateSource("let client = postgres::Client::connect(url, tls);", "bad.rs").some((error) => error.includes("database driver")),
    "database driver source accepted",
  );
  assertSelfTest(
    validateSource("pub const SQL: &str = \"CREATE TABLE prodex_usage_ledger\";", "good.rs").length === 0,
    "SQL text source rejected",
  );
  assertSelfTest(
    validateSource("#![forbid(unsafe_code)]\npub const SQL: &str = \"SELECT 1\";", LIB).length === 0,
    "unsafe-forbidden crate root rejected",
  );
  assertSelfTest(
    validateSource("pub const SQL: &str = \"SELECT 1\";", LIB).some((error) => error.includes("forbid unsafe code")),
    "crate root without unsafe forbid accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  const manifest = await fs.readFile(path.join(repoRoot, MANIFEST), "utf8");
  const errors = [...validateManifest(manifest), ...(await validateSources())];
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`storage-postgres-boundary-guard: ${message}\n`);
  process.exitCode = 1;
});
