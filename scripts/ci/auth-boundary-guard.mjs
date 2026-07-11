#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const CRATES = Object.freeze([
  {
    name: "prodex-authn",
    manifest: "crates/prodex-authn/Cargo.toml",
    srcDir: "crates/prodex-authn/src",
    lib: "crates/prodex-authn/src/lib.rs",
    allowedDependencies: new Set(["prodex_domain"]),
  },
  {
    name: "prodex-authz",
    manifest: "crates/prodex-authz/Cargo.toml",
    srcDir: "crates/prodex-authz/src",
    lib: "crates/prodex-authz/src/lib.rs",
    allowedDependencies: new Set(["prodex_domain"]),
  },
]);
const ALLOWED_DEV_DEPENDENCIES = new Set([]);
const FORBIDDEN_DEPENDENCIES = new Set([
  "anyhow",
  "axum",
  "clap",
  "hyper",
  "jsonwebtoken",
  "openidconnect",
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
  { name: "jwt/oidc implementation", pattern: /\b(jsonwebtoken|openidconnect)\s*::/u },
  { name: "transport", pattern: /\btungstenite\s*::/u },
  { name: "plaintext JWKS URL", pattern: /http:\/\/[^"]*jwks/iu },
]);
const REQUIRED_AUTHZ_SOURCE_SNIPPETS = Object.freeze([
  [
    "Self::BreakGlassAdmin => AuthorizationRequirement::new(",
    "authz boundary must model an explicit break-glass boundary",
  ],
  [
    "CredentialScope::BreakGlass",
    "authz boundary must require BreakGlass scope only on the explicit break-glass boundary",
  ],
  [
    "if requirement.required_scope != CredentialScope::ControlPlane",
    "control-plane resolver must reject non-control-plane scopes",
  ],
  [
    "if requirement.required_scope != CredentialScope::DataPlane",
    "data-plane resolver must reject non-data-plane scopes",
  ],
  [
    "principal.credential_scope == requirement.required_scope",
    "boundary authorization must compare principal scope to the exact boundary requirement",
  ],
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

export function validateAuthManifest(tomlText, crateConfig) {
  const sections = parseDependencySections(tomlText);
  const errors = [];
  const dependencies = sections.get("dependencies") ?? new Set();
  const devDependencies = sections.get("dev-dependencies") ?? new Set();

  for (const dep of dependencies) {
    if (!crateConfig.allowedDependencies.has(dep)) {
      errors.push(`${crateConfig.manifest}: [dependencies] must stay boundary-only; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${crateConfig.manifest}: ${crateConfig.name} cannot depend on forbidden runtime/framework/security implementation crate '${dep}'`);
    }
  }

  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${crateConfig.manifest}: [dev-dependencies] must stay empty; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${crateConfig.manifest}: ${crateConfig.name} tests cannot depend on forbidden runtime/framework crate '${dep}'`);
    }
  }

  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${crateConfig.manifest}: target-specific dependencies are not allowed in ${crateConfig.name}`);
    }
  }

  return errors;
}

export function validateAuthSource(sourceText, sourcePath = "source.rs", crateName = "auth crate", crateLib = null) {
  const errors = [];
  if (sourcePath === crateLib && !sourceText.includes("#![forbid(unsafe_code)]")) {
    errors.push(`${sourcePath}: missing required auth boundary contract '#![forbid(unsafe_code)]'`);
  }
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: ${crateName} cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
  if (crateName === "prodex-authz") {
    for (const [snippet, message] of REQUIRED_AUTHZ_SOURCE_SNIPPETS) {
      if (!sourceText.includes(snippet)) {
        errors.push(`${sourcePath}: ${message}`);
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

async function validateCrate(crateConfig) {
  const manifestPath = path.join(repoRoot, crateConfig.manifest);
  const manifest = await fs.readFile(manifestPath, "utf8");
  const errors = validateAuthManifest(manifest, crateConfig);
  const files = await rustFilesUnder(path.join(repoRoot, crateConfig.srcDir));
  for (const file of files) {
    const source = await fs.readFile(file, "utf8");
    errors.push(...validateAuthSource(source, path.relative(repoRoot, file), crateConfig.name, crateConfig.lib));
  }
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
  const crateConfig = CRATES[0];
  const valid = `
[package]
name = "prodex-authn"

[dependencies]
prodex_domain = { workspace = true }
`;
  assertSelfTest(validateAuthManifest(valid, crateConfig).length === 0, "valid manifest rejected");
  assertSelfTest(
    validateAuthManifest(`${valid}\nreqwest = "0.12"\n`, crateConfig).some((error) => error.includes("reqwest")),
    "forbidden reqwest dependency accepted",
  );
  assertSelfTest(
    validateAuthManifest(`${valid}\nserde_json = "1"\n`, crateConfig).some((error) => error.includes("serde_json")),
    "extra dependency accepted",
  );
  assertSelfTest(
    validateAuthManifest(`${valid}\n[target.'cfg(unix)'.dependencies]\ntokio = "1"\n`, crateConfig).some((error) => error.includes("target-specific")),
    "target-specific dependency accepted",
  );
  assertSelfTest(
    validateAuthSource("use std::net::TcpStream;", "bad.rs", "prodex-authn").some((error) => error.includes("network")),
    "network source boundary accepted",
  );
  assertSelfTest(
    validateAuthSource("let token = jsonwebtoken::decode();", "bad.rs", "prodex-authn").some((error) => error.includes("jwt/oidc")),
    "jwt implementation source boundary accepted",
  );
  assertSelfTest(
    validateAuthSource('let url = "http://issuer.example.com/jwks.json";', "bad.rs", "prodex-authn").some((error) => error.includes("plaintext JWKS URL")),
    "plaintext JWKS URL accepted",
  );
  assertSelfTest(
    validateAuthSource("use std::fmt;\nuse prodex_domain::Principal;", "good.rs", "prodex-authn").length === 0,
    "safe source rejected",
  );
  assertSelfTest(
    validateAuthSource("#![forbid(unsafe_code)]\nuse std::fmt;", crateConfig.lib, "prodex-authn", crateConfig.lib).length === 0,
    "authn unsafe forbid rejected",
  );
  assertSelfTest(
    validateAuthSource("use std::fmt;", crateConfig.lib, "prodex-authn", crateConfig.lib).some((error) =>
      error.includes("forbid(unsafe_code)"),
    ),
    "missing authn unsafe forbid accepted",
  );
  assertSelfTest(
    validateAuthSource(
      `#![forbid(unsafe_code)]
Self::BreakGlassAdmin => AuthorizationRequirement::new(
    ResourceKind::Configuration,
    ResourceAction::Update,
    CredentialScope::BreakGlass,
    Role::Admin,
)
if requirement.required_scope != CredentialScope::ControlPlane {}
if requirement.required_scope != CredentialScope::DataPlane {}
if principal.credential_scope == requirement.required_scope {}
`,
      CRATES[1].lib,
      "prodex-authz",
      CRATES[1].lib,
    ).length === 0,
    "authz unsafe forbid rejected",
  );
  assertSelfTest(
    validateAuthSource("use std::fmt;", CRATES[1].lib, "prodex-authz", CRATES[1].lib).some((error) =>
      error.includes("forbid(unsafe_code)"),
    ),
    "missing authz unsafe forbid accepted",
  );
  const validAuthzSource = `
Self::BreakGlassAdmin => AuthorizationRequirement::new(
    ResourceKind::Configuration,
    ResourceAction::Update,
    CredentialScope::BreakGlass,
    Role::Admin,
)
if requirement.required_scope != CredentialScope::ControlPlane {}
if requirement.required_scope != CredentialScope::DataPlane {}
if principal.credential_scope == requirement.required_scope {}
`;
  assertSelfTest(
    validateAuthSource(validAuthzSource, "authz.rs", "prodex-authz").length === 0,
    "valid authz semantics rejected",
  );
  assertSelfTest(
    validateAuthSource(
      validAuthzSource.replace("CredentialScope::BreakGlass", "CredentialScope::ControlPlane"),
      "authz.rs",
      "prodex-authz",
    ).some((error) => error.includes("BreakGlass scope")),
    "missing explicit break-glass scope accepted",
  );
  assertSelfTest(
    validateAuthSource(
      validAuthzSource.replace("principal.credential_scope == requirement.required_scope", "true"),
      "authz.rs",
      "prodex-authz",
    ).some((error) => error.includes("exact boundary requirement")),
    "missing exact scope comparison accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }

  const allErrors = [];
  for (const crateConfig of CRATES) {
    allErrors.push(...(await validateCrate(crateConfig)));
  }
  if (allErrors.length > 0) {
    for (const error of allErrors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`auth-boundary-guard: ${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
