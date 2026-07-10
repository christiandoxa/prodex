#!/usr/bin/env node
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const STORAGE_MANIFEST = "crates/prodex-storage/Cargo.toml";
const STORAGE_SRC_DIR = "crates/prodex-storage/src";
const STORAGE_LIB = "crates/prodex-storage/src/lib.rs";
const ALLOWED_DEPENDENCIES = new Set(["prodex_domain"]);
const ALLOWED_DEV_DEPENDENCIES = new Set([]);
const FORBIDDEN_DEPENDENCIES = new Set([
  "anyhow",
  "axum",
  "clap",
  "deadpool-postgres",
  "hyper",
  "postgres",
  "redis",
  "reqwest",
  "rusqlite",
  "sqlx",
  "tokio",
  "tokio-postgres",
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
  { name: "database", pattern: /\b(rusqlite|sqlx|postgres|redis|tokio_postgres|deadpool_postgres)\s*::/u },
  { name: "transport", pattern: /\btungstenite\s*::/u },
]);
const REQUIRED_SOURCE_SNIPPETS = Object.freeze([
  "pub struct MultiReplicaAccountingEvidence",
  "pub topology: StorageTopology",
  "pub struct MultiReplicaAccountingVerificationPlan",
  "EvidenceTopologyMismatch",
  "if evidence.topology != spec.topology",
  "expected: spec.topology",
  "actual: evidence.topology",
  "topology: evidence.topology",
]);
const OPTIONAL_POSTGRES_EVIDENCE_TEST = Object.freeze({
  command: "cargo",
  args: [
    "test",
    "-q",
    "-p",
    "prodex-storage-postgres",
    "postgres_atomic_reservation_allows_only_one_concurrent_claim_per_budget_scope",
    "--",
    "--test-threads=1",
  ],
});

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

export function validateStorageManifest(tomlText, manifestPath = STORAGE_MANIFEST) {
  const sections = parseDependencySections(tomlText);
  const errors = [];
  const dependencies = sections.get("dependencies") ?? new Set();
  const devDependencies = sections.get("dev-dependencies") ?? new Set();

  for (const dep of dependencies) {
    if (!ALLOWED_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dependencies] must stay adapter-neutral; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-storage cannot depend on forbidden implementation crate '${dep}'`);
    }
  }

  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dev-dependencies] must stay empty; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-storage tests cannot depend on forbidden implementation crate '${dep}'`);
    }
  }

  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${manifestPath}: target-specific dependencies are not allowed in prodex-storage`);
    }
  }

  return errors;
}

export function validateStorageSource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: prodex-storage cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
  return errors;
}

export function validateStorageRequiredContracts(sourceText, sourcePath = STORAGE_LIB) {
  const errors = [];
  if (!sourceText.includes("#![forbid(unsafe_code)]")) {
    errors.push(`${sourcePath}: prodex-storage crate root must forbid unsafe code`);
  }
  for (const snippet of REQUIRED_SOURCE_SNIPPETS) {
    if (!sourceText.includes(snippet)) {
      errors.push(`${sourcePath}: missing required storage boundary contract '${snippet}'`);
    }
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

async function validateStorageSources() {
  const srcRoot = path.join(repoRoot, STORAGE_SRC_DIR);
  const files = await rustFilesUnder(srcRoot);
  const errors = [];
  for (const file of files) {
    const source = await fs.readFile(file, "utf8");
    const relativePath = path.relative(repoRoot, file);
    errors.push(...validateStorageSource(source, relativePath));
    if (relativePath === STORAGE_LIB) {
      errors.push(...validateStorageRequiredContracts(source, STORAGE_LIB));
    }
  }
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runCommand(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: repoRoot,
      stdio: "inherit",
      env: { ...process.env, ...(options.env ?? {}) },
    });
    child.on("error", reject);
    child.on("exit", (code, signal) => {
      if (signal) {
        reject(new Error(`${command} ${args.join(" ")} exited from signal ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(new Error(`${command} ${args.join(" ")} exited with code ${code}`));
        return;
      }
      resolve();
    });
  });
}

function runSelfTest() {
  const valid = `
[package]
name = "prodex-storage"

[dependencies]
prodex_domain = { workspace = true }
`;
  assertSelfTest(validateStorageManifest(valid, "valid/Cargo.toml").length === 0, "valid manifest rejected");
  assertSelfTest(
    validateStorageManifest(`${valid}\nsqlx = "0.8"\n`, "invalid/Cargo.toml").some((error) => error.includes("sqlx")),
    "forbidden sqlx dependency accepted",
  );
  assertSelfTest(
    validateStorageManifest(`${valid}\nserde_json = "1"\n`, "invalid-extra/Cargo.toml").some((error) => error.includes("serde_json")),
    "extra dependency accepted",
  );
  assertSelfTest(
    validateStorageManifest(`${valid}\n[target.'cfg(unix)'.dependencies]\nredis = "0.25"\n`, "invalid-target/Cargo.toml").some((error) => error.includes("target-specific")),
    "target-specific dependency accepted",
  );
  assertSelfTest(
    validateStorageSource("use rusqlite::Connection;", "bad.rs").some((error) => error.includes("database")),
    "database source boundary accepted",
  );
  assertSelfTest(
    validateStorageSource("use std::fs;", "bad.rs").some((error) => error.includes("filesystem")),
    "filesystem source boundary accepted",
  );
  assertSelfTest(
    validateStorageSource("use std::fmt;\nuse prodex_domain::TenantId;", "good.rs").length === 0,
    "safe source rejected",
  );
  assertSelfTest(
    validateStorageRequiredContracts(
      `
#![forbid(unsafe_code)]
pub struct MultiReplicaAccountingEvidence {
    pub topology: StorageTopology,
}
pub struct MultiReplicaAccountingVerificationPlan {
    pub topology: StorageTopology,
}
EvidenceTopologyMismatch
if evidence.topology != spec.topology
expected: spec.topology
actual: evidence.topology
topology: evidence.topology
`,
      "good.rs",
    ).length === 0,
    "required multi-replica topology contracts rejected",
  );
  assertSelfTest(
    validateStorageRequiredContracts("pub struct MultiReplicaAccountingEvidence {}", "bad.rs").some((error) =>
      error.includes("forbid unsafe code"),
    ),
    "crate root without unsafe forbid accepted",
  );
  assertSelfTest(
    validateStorageRequiredContracts("#![forbid(unsafe_code)]\npub struct MultiReplicaAccountingEvidence {}", "bad.rs").some((error) =>
      error.includes("EvidenceTopologyMismatch"),
    ),
    "missing topology mismatch contract accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }

  const manifest = await fs.readFile(path.join(repoRoot, STORAGE_MANIFEST), "utf8");
  const errors = [...validateStorageManifest(manifest), ...(await validateStorageSources())];
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
    return;
  }

  const postgresUrl = process.env.PRODEX_TEST_POSTGRES_URL;
  if (!postgresUrl) {
    process.stdout.write(
      "storage-boundary-guard: skipping optional Postgres execution proof; set PRODEX_TEST_POSTGRES_URL to enable it\n",
    );
    return;
  }

  process.stdout.write(
    "storage-boundary-guard: running optional Postgres execution proof\n",
  );
  await runCommand(
    OPTIONAL_POSTGRES_EVIDENCE_TEST.command,
    OPTIONAL_POSTGRES_EVIDENCE_TEST.args,
    { env: { PRODEX_TEST_POSTGRES_URL: postgresUrl } },
  );
}

main().catch((error) => {
  process.stderr.write(`storage-boundary-guard: ${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
