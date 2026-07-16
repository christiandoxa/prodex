#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseDependencySections } from "./boundary-guard-utils.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const CONTROL_PLANE_MANIFEST = "crates/prodex-control-plane/Cargo.toml";
const CONTROL_PLANE_SRC_DIR = "crates/prodex-control-plane/src";
const ALLOWED_DEPENDENCIES = new Set(["prodex_config", "prodex_domain"]);
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

function sorted(values) {
  return [...values].sort((left, right) => left.localeCompare(right));
}

export function validateControlPlaneManifest(tomlText, manifestPath = CONTROL_PLANE_MANIFEST) {
  const sections = parseDependencySections(tomlText);
  const errors = [];
  const dependencies = sections.get("dependencies") ?? new Set();
  const devDependencies = sections.get("dev-dependencies") ?? new Set();

  for (const dep of dependencies) {
    if (!ALLOWED_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dependencies] must stay control-plane boundary crates only; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-control-plane cannot depend on forbidden runtime/framework/storage-driver crate '${dep}'`);
    }
  }

  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dev-dependencies] must stay empty; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-control-plane tests cannot depend on forbidden runtime/framework crate '${dep}'`);
    }
  }

  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${manifestPath}: target-specific dependencies are not allowed in prodex-control-plane`);
    }
  }

  return errors;
}

export function validateControlPlaneSource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: prodex-control-plane cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
  if (sourcePath === "crates/prodex-control-plane/src/lib.rs" && !sourceText.includes("#![forbid(unsafe_code)]")) {
    errors.push(`${sourcePath}: prodex-control-plane crate root must forbid unsafe code`);
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

async function validateControlPlaneSources() {
  const files = await rustFilesUnder(path.join(repoRoot, CONTROL_PLANE_SRC_DIR));
  const errors = [];
  for (const file of files) {
    const source = await fs.readFile(file, "utf8");
    errors.push(...validateControlPlaneSource(source, path.relative(repoRoot, file)));
  }
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
  const valid = `
[package]
name = "prodex-control-plane"

[dependencies]
prodex_config = { workspace = true }
prodex_domain = { workspace = true }
`;
  assertSelfTest(validateControlPlaneManifest(valid, "valid/Cargo.toml").length === 0, "valid manifest rejected");
  assertSelfTest(
    validateControlPlaneManifest(`${valid}\naxum = "0.8"\n`, "invalid/Cargo.toml").some((error) => error.includes("axum")),
    "forbidden HTTP dependency accepted",
  );
  assertSelfTest(
    validateControlPlaneManifest(`${valid}\nserde_json = "1"\n`, "invalid-extra/Cargo.toml").some((error) => error.includes("serde_json")),
    "extra dependency accepted",
  );
  assertSelfTest(
    validateControlPlaneManifest(`${valid}\n[target.'cfg(unix)'.dependencies]\ntokio = "1"\n`, "invalid-target/Cargo.toml").some((error) => error.includes("target-specific")),
    "target-specific dependency accepted",
  );
  assertSelfTest(
    validateControlPlaneSource("use std::net::TcpStream;", "bad.rs").some((error) => error.includes("network")),
    "network source boundary accepted",
  );
  assertSelfTest(
    validateControlPlaneSource("let pool = sqlx::PgPool::connect_lazy(url);", "bad.rs").some((error) => error.includes("database")),
    "database source boundary accepted",
  );
  assertSelfTest(
    validateControlPlaneSource("use prodex_domain::TenantContext;", "good.rs").length === 0,
    "safe source rejected",
  );
  assertSelfTest(
    validateControlPlaneSource("#![forbid(unsafe_code)]\nuse prodex_domain::TenantContext;", "crates/prodex-control-plane/src/lib.rs").length === 0,
    "unsafe-forbidden crate root rejected",
  );
  assertSelfTest(
    validateControlPlaneSource("use prodex_domain::TenantContext;", "crates/prodex-control-plane/src/lib.rs").some((error) => error.includes("forbid unsafe code")),
    "crate root without unsafe forbid accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }

  const manifest = await fs.readFile(path.join(repoRoot, CONTROL_PLANE_MANIFEST), "utf8");
  const errors = [...validateControlPlaneManifest(manifest), ...(await validateControlPlaneSources())];
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`control-plane-boundary-guard: ${message}\n`);
  process.exitCode = 1;
});
