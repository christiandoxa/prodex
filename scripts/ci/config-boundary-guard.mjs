#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseDependencySections } from "./boundary-guard-utils.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const CONFIG_MANIFEST = "crates/prodex-config/Cargo.toml";
const CONFIG_SRC_DIR = "crates/prodex-config/src";
const CONFIG_LIB = "crates/prodex-config/src/lib.rs";
const RUNTIME_CONFIG_ENVIRONMENT = "crates/prodex-app/src/runtime_config/environment.rs";
const RUNTIME_GEMINI_HOT_PATH_FILES = Object.freeze([
  "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_util.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_policy.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_extensions.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_memory.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_session.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_tool_output.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live/connection.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live_translation.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_oauth_pool.rs",
]);
const RUNTIME_GEMINI_ENV_KEYS = Object.freeze([
  "HOME",
  "GEMINI_CLI_HOME",
  "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
  "GEMINI_CLI_SYSTEM_DEFAULTS_PATH",
  "PRODEX_GEMINI_EXTENSION_DIRS",
  "PRODEX_GEMINI_EXTENSIONS",
  "PRODEX_GEMINI_EXPORT_FILE",
  "PRODEX_GEMINI_CHECKPOINT_EXPORT_FILE",
  "PRODEX_GEMINI_SESSION_FILE",
  "PRODEX_GEMINI_CHECKPOINT_FILE",
  "PRODEX_GEMINI_IMPORT_FILE",
  "PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD",
  "PRODEX_GEMINI_TOOL_OUTPUT_DIR",
  "PRODEX_GEMINI_DISABLE_MEMORY",
  "PRODEX_GEMINI_DISABLE_CONTEXT_FILES",
  "PRODEX_GEMINI_LOAD_MEMORY",
  "PRODEX_GEMINI_MEMORY",
  "PRODEX_GEMINI_EXTENSION_MEMORY",
  "PRODEX_GEMINI_LIVE_URL",
  "PRODEX_GEMINI_LIVE_MODEL",
  "PRODEX_GEMINI_STICKY_FRESH_OAUTH",
]);
const RUNTIME_GEMINI_POLICY =
  "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_policy.rs";
const RUNTIME_GATEWAY_TOPOLOGY_HOT_PATH_FILES = Object.freeze([
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_state_store_config.rs",
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_startup.rs",
]);
const RUNTIME_GATEWAY_TOPOLOGY =
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_state_store_config.rs";
const RUNTIME_GATEWAY_STARTUP =
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_startup.rs";
const RUNTIME_GATEWAY_TOPOLOGY_ENV_KEYS = Object.freeze([
  "PRODEX_GATEWAY_REPLICA_COUNT",
  "PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS",
]);
const RUNTIME_GATEWAY_STARTUP_CONFIG_FILES = Object.freeze([
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_config.rs",
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_provider_config.rs",
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_auth_config.rs",
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_state_store_config.rs",
]);
const RUNTIME_GATEWAY_STARTUP_FUNCTIONS = Object.freeze([
  [RUNTIME_GATEWAY_STARTUP_CONFIG_FILES[0], "resolve_gateway_launch_config_for_service_mode"],
  [RUNTIME_GATEWAY_STARTUP_CONFIG_FILES[0], "resolve_gateway_refresh_candidate_for_service_mode"],
  [RUNTIME_GATEWAY_STARTUP_CONFIG_FILES[1], "resolve_gateway_provider_config_with_resolver"],
  [RUNTIME_GATEWAY_STARTUP_CONFIG_FILES[1], "resolve_gateway_provider_credentials_with_resolver"],
  [RUNTIME_GATEWAY_STARTUP_CONFIG_FILES[2], "resolve_gateway_auth_config_with_resolver"],
  [RUNTIME_GATEWAY_STARTUP_CONFIG_FILES[3], "gateway_state_store_config_with_resolver"],
]);
const RUNTIME_GATEWAY_STARTUP_ENV_KEYS = Object.freeze([
  "OPENAI_BASE_URL",
  "PRODEX_DEEPSEEK_STRICT_TOOLS",
  "PRODEX_DEEPSEEK_BETA_BASE_URL",
  "PRODEX_DEEPSEEK_WEB_SEARCH_MODE",
  "PRODEX_GATEWAY_TOKEN",
  "PRODEX_GATEWAY_POSTGRES_URL",
  "PRODEX_GATEWAY_REDIS_URL",
  "OPENAI_API_KEYS",
  "OPENAI_API_KEY",
  "ANTHROPIC_API_KEYS",
  "ANTHROPIC_API_KEY",
  "GITHUB_COPILOT_API_KEYS",
  "GITHUB_COPILOT_API_KEY",
  "DEEPSEEK_API_KEYS",
  "DEEPSEEK_API_KEY",
  "GEMINI_API_KEYS",
  "GOOGLE_API_KEYS",
  "GEMINI_API_KEY",
  "GOOGLE_API_KEY",
]);
const RUNTIME_GATEWAY_PROVIDER_CONFIG = RUNTIME_GATEWAY_STARTUP_CONFIG_FILES[1];
const RUNTIME_GATEWAY_SECRET_CONFIG =
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_secret_config.rs";
const RUNTIME_OIDC_HOT_PATH_FILES = Object.freeze([
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_sso_config.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/admin.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/cache.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/endpoint_policy.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/token_claims.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/transport.rs",
]);
const RUNTIME_OIDC_ENV_KEYS = Object.freeze([
  "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS",
  "PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS",
  "PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS",
  "PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS",
]);
const RUNTIME_ENV_READ_PATTERN = /\b(?:std\s*::\s*)?env\s*::\s*(?:var|var_os|vars|vars_os)\s*\(/u;
const ALLOWED_DEPENDENCIES = new Set(["prodex_domain"]);
const ALLOWED_DEV_DEPENDENCIES = new Set([]);
const FORBIDDEN_DEPENDENCIES = new Set([
  "anyhow",
  "axum",
  "clap",
  "config",
  "figment",
  "hyper",
  "notify",
  "reqwest",
  "rusqlite",
  "serde_json",
  "sqlx",
  "tokio",
  "toml",
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
  { name: "config parser/watcher", pattern: /\b(toml|serde_json|notify|figment|config)\s*::/u },
  { name: "transport", pattern: /\btungstenite\s*::/u },
]);
const REQUIRED_SOURCE_SNIPPETS = Object.freeze([
  "#![forbid(unsafe_code)]",
  "pub enum ConfigRefreshError",
  "pub enum ConfigRefreshErrorStatus",
  "pub struct ConfigRefreshErrorResponsePlan",
  "pub enum ConfigSecretSource",
  "Reference(SecretRef)",
  "RawSecretMaterial",
  "pub struct ConfigSecretReferencePlan",
  "pub enum ConfigSecretReferenceError",
  "pub fn plan_config_secret_reference_error_response(",
  "pub fn plan_config_secret_reference(",
  "ConfigSecretSource::Reference(reference)",
  "ConfigSecretSource::RawSecretMaterial =>",
  "if !reference.is_well_formed()",
  "Err(ConfigSecretReferenceError::RawSecretMaterialRejected)",
  "message: \"configuration secrets must use secret references\"",
  "pub fn config_refresh_error_for_decision(",
  "pub fn plan_config_refresh_error_response(",
  "ConfigRefreshDecision::RefreshRequired => Some(ConfigRefreshError::RefreshRequired)",
  "ConfigRefreshDecision::RejectedInvalidated =>",
  "code: \"configuration_refresh_required\"",
  "code: \"configuration_revision_unavailable\"",
  "message: \"configuration is not currently available\"",
]);

function sorted(values) {
  return [...values].sort((left, right) => left.localeCompare(right));
}

export function validateConfigManifest(tomlText, manifestPath = CONFIG_MANIFEST) {
  const sections = parseDependencySections(tomlText);
  const errors = [];
  const dependencies = sections.get("dependencies") ?? new Set();
  const devDependencies = sections.get("dev-dependencies") ?? new Set();

  for (const dep of dependencies) {
    if (!ALLOWED_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dependencies] must stay boundary-only; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-config cannot depend on forbidden parser/runtime/framework crate '${dep}'`);
    }
  }

  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dev-dependencies] must stay empty; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-config tests cannot depend on forbidden parser/runtime/framework crate '${dep}'`);
    }
  }

  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${manifestPath}: target-specific dependencies are not allowed in prodex-config`);
    }
  }
  return errors;
}

export function validateConfigSource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: prodex-config cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
  return errors;
}

export function validateConfigRequiredContracts(sourceText, sourcePath = CONFIG_LIB) {
  const errors = [];
  for (const snippet of REQUIRED_SOURCE_SNIPPETS) {
    if (!sourceText.includes(snippet)) {
      errors.push(`${sourcePath}: missing required configuration boundary contract '${snippet}'`);
    }
  }
  return errors;
}

function validateRuntimeSnapshotBoundary(environmentSource, hotPathSources, environmentKeys, boundaryName) {
  const errors = [];
  for (const key of environmentKeys) {
    const count = environmentSource.split(`"${key}"`).length - 1;
    if (count !== 1) {
      errors.push(`${RUNTIME_CONFIG_ENVIRONMENT}: expected exactly one startup read entry for ${key}, found ${count}`);
    }
  }
  for (const [file, source] of hotPathSources) {
    source.split(/\r?\n/u).forEach((line, index) => {
      if (RUNTIME_ENV_READ_PATTERN.test(line)) {
        errors.push(`${file}:${index + 1}: ${boundaryName} hot path must use RuntimeConfig, not '${line.trim()}'`);
      }
    });
  }
  return errors;
}

export function validateRuntimeGeminiConfigBoundary(environmentSource, hotPathSources) {
  const errors = validateRuntimeSnapshotBoundary(
    environmentSource,
    hotPathSources,
    RUNTIME_GEMINI_ENV_KEYS,
    "runtime Gemini",
  );
  const policy = hotPathSources.find(([file]) => file === RUNTIME_GEMINI_POLICY)?.[1] ?? "";
  if (!policy.includes("gemini_settings_source_paths_for_config_home(")) {
    errors.push(`${RUNTIME_GEMINI_POLICY}: Gemini policy must use the explicit pure settings path planner`);
  }
  if (/\bgemini_settings_source_paths_for\s*\(/u.test(policy)) {
    errors.push(`${RUNTIME_GEMINI_POLICY}: Gemini policy must not call the environment-reading compatibility planner`);
  }
  for (const field of [
    "config.config_dir",
    "config.system_defaults_path",
    "config.system_settings_path",
  ]) {
    if (!policy.includes(field)) {
      errors.push(`${RUNTIME_GEMINI_POLICY}: Gemini policy must consume snapshotted ${field}`);
    }
  }
  return errors;
}

export function validateRuntimeOidcConfigBoundary(environmentSource, hotPathSources) {
  return validateRuntimeSnapshotBoundary(
    environmentSource,
    hotPathSources,
    RUNTIME_OIDC_ENV_KEYS,
    "runtime OIDC",
  );
}

export function validateRuntimeGatewayTopologyConfigBoundary(environmentSource, hotPathSources) {
  const errors = [];
  for (const key of RUNTIME_GATEWAY_TOPOLOGY_ENV_KEYS) {
    const count = environmentSource.split(`"${key}"`).length - 1;
    if (count !== 1) {
      errors.push(`${RUNTIME_CONFIG_ENVIRONMENT}: expected exactly one startup read entry for ${key}, found ${count}`);
    }
    for (const [file, source] of hotPathSources) {
      if (source.includes(`"${key}"`)) {
        errors.push(`${file}: runtime gateway topology must use RuntimeConfig, not ${key}`);
      }
    }
  }
  const topology = hotPathSources.find(([file]) => file === RUNTIME_GATEWAY_TOPOLOGY)?.[1] ?? "";
  for (const field of [
    "config.replica_count",
    "config.require_multi_replica_accounting_checks",
  ]) {
    if (!topology.includes(field)) {
      errors.push(`${RUNTIME_GATEWAY_TOPOLOGY}: gateway topology must consume ${field}`);
    }
  }
  const startup = hotPathSources.find(([file]) => file === RUNTIME_GATEWAY_STARTUP)?.[1] ?? "";
  const snapshot = startup.indexOf("RuntimeConfig::from_gateway_env_policy_and_cli(");
  const resolution = startup.indexOf("resolve_gateway_launch_config_for_service_mode(");
  if (snapshot < 0 || resolution < 0 || snapshot >= resolution) {
    errors.push(`${RUNTIME_GATEWAY_STARTUP}: RuntimeConfig snapshot must precede gateway config resolution`);
  }
  if ((startup.match(/Arc::clone\(&runtime_config\)/gu) ?? []).length < 2) {
    errors.push(`${RUNTIME_GATEWAY_STARTUP}: refresh and proxy workers must share the gateway RuntimeConfig Arc`);
  }
  return errors;
}

function rustFunctionSource(source, name) {
  const declaration = new RegExp(`\\bfn\\s+${name}\\s*\\(`, "u").exec(source);
  if (!declaration) return "";
  const brace = source.indexOf("{", declaration.index);
  if (brace < 0) return "";
  let depth = 0;
  for (let index = brace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    else if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(declaration.index, index + 1);
    }
  }
  return "";
}

export function validateRuntimeGatewayStartupConfigBoundary(environmentSource, sources) {
  const errors = [];
  const byFile = new Map(sources);
  for (const key of RUNTIME_GATEWAY_STARTUP_ENV_KEYS) {
    const count = environmentSource.split(`"${key}"`).length - 1;
    if (count !== 1) {
      errors.push(`${RUNTIME_CONFIG_ENVIRONMENT}: expected exactly one gateway startup entry for ${key}, found ${count}`);
    }
  }
  for (const [file, name] of RUNTIME_GATEWAY_STARTUP_FUNCTIONS) {
    const body = rustFunctionSource(byFile.get(file) ?? "", name);
    if (!body) {
      errors.push(`${file}: missing gateway startup function ${name}`);
    } else if (RUNTIME_ENV_READ_PATTERN.test(body)) {
      errors.push(`${file}: ${name} must consume RuntimeConfig instead of reading the environment`);
    }
  }
  const provider = byFile.get(RUNTIME_GATEWAY_PROVIDER_CONFIG) ?? "";
  for (const forbidden of ["_api_keys_from_request_or_env", "RuntimeGeminiModelResolution::from_current_settings"] ) {
    if (provider.includes(forbidden)) {
      errors.push(`${RUNTIME_GATEWAY_PROVIDER_CONFIG}: production gateway provider resolution must not use ${forbidden}`);
    }
  }
  const secrets = byFile.get(RUNTIME_GATEWAY_SECRET_CONFIG) ?? "";
  const secretEnvironmentReads = secrets
    .split(/\r?\n/u)
    .filter((line) => RUNTIME_ENV_READ_PATTERN.test(line)).length;
  if (secretEnvironmentReads !== 1) {
    errors.push(`${RUNTIME_GATEWAY_SECRET_CONFIG}: deferred secret resolver must own the sole gateway environment material read`);
  }
  const startup = byFile.get(RUNTIME_GATEWAY_STARTUP) ?? "";
  if (!startup.includes("RuntimeConfig::from_gateway_env_policy_and_cli")) {
    errors.push(`${RUNTIME_GATEWAY_STARTUP}: dedicated gateway startup must build the gateway RuntimeConfig snapshot`);
  }
  if ((startup.match(/resolve_gateway_launch_config_for_service_mode\s*\(/gu) ?? []).length !== 1) {
    errors.push(`${RUNTIME_GATEWAY_STARTUP}: full gateway launch resolution must run exactly once`);
  }
  if (!startup.includes("resolve_gateway_refresh_candidate_for_service_mode(")) {
    errors.push(`${RUNTIME_GATEWAY_STARTUP}: secret refresh must use the secret-only refresh adapter`);
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

async function validateConfigSources() {
  const srcRoot = path.join(repoRoot, CONFIG_SRC_DIR);
  const files = await rustFilesUnder(srcRoot);
  const errors = [];
  for (const file of files) {
    const source = await fs.readFile(file, "utf8");
    errors.push(...validateConfigSource(source, path.relative(repoRoot, file)));
    if (path.relative(repoRoot, file) === CONFIG_LIB) {
      errors.push(...validateConfigRequiredContracts(source, CONFIG_LIB));
    }
  }
  return errors;
}

async function validateRuntimeGeminiConfigSources() {
  const environmentSource = await fs.readFile(path.join(repoRoot, RUNTIME_CONFIG_ENVIRONMENT), "utf8");
  const hotPathSources = await Promise.all(
    RUNTIME_GEMINI_HOT_PATH_FILES.map(async (file) => [file, await fs.readFile(path.join(repoRoot, file), "utf8")]),
  );
  return validateRuntimeGeminiConfigBoundary(environmentSource, hotPathSources);
}

async function validateRuntimeOidcConfigSources() {
  const environmentSource = await fs.readFile(path.join(repoRoot, RUNTIME_CONFIG_ENVIRONMENT), "utf8");
  const hotPathSources = await Promise.all(
    RUNTIME_OIDC_HOT_PATH_FILES.map(async (file) => [file, await fs.readFile(path.join(repoRoot, file), "utf8")]),
  );
  return validateRuntimeOidcConfigBoundary(environmentSource, hotPathSources);
}

async function validateRuntimeGatewayTopologyConfigSources() {
  const environmentSource = await fs.readFile(path.join(repoRoot, RUNTIME_CONFIG_ENVIRONMENT), "utf8");
  const hotPathSources = await Promise.all(
    RUNTIME_GATEWAY_TOPOLOGY_HOT_PATH_FILES.map(async (file) => [
      file,
      await fs.readFile(path.join(repoRoot, file), "utf8"),
    ]),
  );
  return validateRuntimeGatewayTopologyConfigBoundary(environmentSource, hotPathSources);
}

async function validateRuntimeGatewayStartupConfigSources() {
  const environmentSource = await fs.readFile(path.join(repoRoot, RUNTIME_CONFIG_ENVIRONMENT), "utf8");
  const files = [...new Set([
    ...RUNTIME_GATEWAY_STARTUP_CONFIG_FILES,
    RUNTIME_GATEWAY_SECRET_CONFIG,
    RUNTIME_GATEWAY_STARTUP,
  ])];
  const sources = await Promise.all(
    files.map(async (file) => [file, await fs.readFile(path.join(repoRoot, file), "utf8")]),
  );
  return validateRuntimeGatewayStartupConfigBoundary(environmentSource, sources);
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
  const valid = `
[package]
name = "prodex-config"

[dependencies]
prodex_domain = { workspace = true }
`;
  assertSelfTest(validateConfigManifest(valid, "valid/Cargo.toml").length === 0, "valid manifest rejected");
  assertSelfTest(
    validateConfigManifest(`${valid}\ntoml = "0.8"\n`, "invalid/Cargo.toml").some((error) => error.includes("toml")),
    "forbidden toml dependency accepted",
  );
  assertSelfTest(
    validateConfigManifest(`${valid}\nserde_json = "1"\n`, "invalid-extra/Cargo.toml").some((error) => error.includes("serde_json")),
    "extra dependency accepted",
  );
  assertSelfTest(
    validateConfigManifest(`${valid}\n[target.'cfg(unix)'.dependencies]\nnotify = "6"\n`, "invalid-target/Cargo.toml").some((error) => error.includes("target-specific")),
    "target-specific dependency accepted",
  );
  assertSelfTest(
    validateConfigSource("let parsed = toml::from_str(input);", "bad.rs").some((error) => error.includes("config parser")),
    "parser source boundary accepted",
  );
  assertSelfTest(
    validateConfigSource("use std::fs;", "bad.rs").some((error) => error.includes("filesystem")),
    "filesystem source boundary accepted",
  );
  assertSelfTest(
    validateConfigSource("use std::fmt;\nuse prodex_domain::TenantId;", "good.rs").length === 0,
    "safe source rejected",
  );
  assertSelfTest(
    validateConfigRequiredContracts(
      `
#![forbid(unsafe_code)]
pub enum ConfigRefreshError {}
pub enum ConfigRefreshErrorStatus {}
pub struct ConfigRefreshErrorResponsePlan {}
pub enum ConfigSecretSource { Reference(SecretRef), RawSecretMaterial }
pub struct ConfigSecretReferencePlan {}
pub enum ConfigSecretReferenceError {}
pub fn plan_config_secret_reference_error_response() {}
pub fn plan_config_secret_reference() {}
ConfigSecretSource::Reference(reference)
ConfigSecretSource::RawSecretMaterial =>
if !reference.is_well_formed()
Err(ConfigSecretReferenceError::RawSecretMaterialRejected)
message: "configuration secrets must use secret references"
pub fn config_refresh_error_for_decision() {}
pub fn plan_config_refresh_error_response() {}
ConfigRefreshDecision::RefreshRequired => Some(ConfigRefreshError::RefreshRequired)
ConfigRefreshDecision::RejectedInvalidated =>
code: "configuration_refresh_required"
code: "configuration_revision_unavailable"
message: "configuration is not currently available"
`,
      "good.rs",
    ).length === 0,
    "required refresh contracts rejected",
  );
  assertSelfTest(
    validateConfigRequiredContracts("pub enum ConfigRefreshError {}", "bad.rs").some((error) =>
      error.includes("plan_config_refresh_error_response"),
    ),
    "missing refresh response contract accepted",
  );
  assertSelfTest(
    validateConfigRequiredContracts(
      `
#![forbid(unsafe_code)]
pub enum ConfigRefreshError {}
pub enum ConfigRefreshErrorStatus {}
pub struct ConfigRefreshErrorResponsePlan {}
pub enum ConfigSecretSource { Reference(SecretRef) }
pub struct ConfigSecretReferencePlan {}
pub enum ConfigSecretReferenceError {}
pub fn plan_config_secret_reference_error_response() {}
pub fn plan_config_secret_reference() {}
ConfigSecretSource::Reference(reference)
if !reference.is_well_formed()
message: "configuration secrets must use secret references"
pub fn config_refresh_error_for_decision() {}
pub fn plan_config_refresh_error_response() {}
ConfigRefreshDecision::RefreshRequired => Some(ConfigRefreshError::RefreshRequired)
ConfigRefreshDecision::RejectedInvalidated =>
code: "configuration_refresh_required"
code: "configuration_revision_unavailable"
message: "configuration is not currently available"
`,
      "bad-secret.rs",
    ).some((error) => error.includes("RawSecretMaterial")),
    "missing raw-secret rejection contract accepted",
  );
  assertSelfTest(
    validateConfigRequiredContracts(
      `
pub enum ConfigRefreshError {}
pub enum ConfigRefreshErrorStatus {}
pub struct ConfigRefreshErrorResponsePlan {}
pub enum ConfigSecretSource { Reference(SecretRef), RawSecretMaterial }
pub struct ConfigSecretReferencePlan {}
pub enum ConfigSecretReferenceError {}
pub fn plan_config_secret_reference_error_response() {}
pub fn plan_config_secret_reference() {}
ConfigSecretSource::Reference(reference)
ConfigSecretSource::RawSecretMaterial =>
if !reference.is_well_formed()
Err(ConfigSecretReferenceError::RawSecretMaterialRejected)
message: "configuration secrets must use secret references"
pub fn config_refresh_error_for_decision() {}
pub fn plan_config_refresh_error_response() {}
ConfigRefreshDecision::RefreshRequired => Some(ConfigRefreshError::RefreshRequired)
ConfigRefreshDecision::RejectedInvalidated =>
code: "configuration_refresh_required"
code: "configuration_revision_unavailable"
message: "configuration is not currently available"
`,
      "bad.rs",
    ).some((error) => error.includes("forbid(unsafe_code)")),
    "missing config unsafe forbid accepted",
  );
  const runtimeEnvironment = RUNTIME_GEMINI_ENV_KEYS.map((key) => `"${key}",`).join("\n");
  const geminiPolicy = `gemini_settings_source_paths_for_config_home(
    config.config_dir,
    config.system_settings_path,
    config.system_defaults_path,
  );`;
  assertSelfTest(
    validateRuntimeGeminiConfigBoundary(runtimeEnvironment, [
      [RUNTIME_GEMINI_POLICY, geminiPolicy],
      ["good.rs", "env::current_dir();"],
    ]).length === 0,
    "typed Gemini runtime config rejected",
  );
  assertSelfTest(
    validateRuntimeGeminiConfigBoundary(runtimeEnvironment, [
      [RUNTIME_GEMINI_POLICY, geminiPolicy],
      ["bad.rs", "let value = std::env::var(\"KEY\");"],
    ]).some((error) => error.includes("RuntimeConfig")),
    "Gemini request-path environment read accepted",
  );
  assertSelfTest(
    validateRuntimeGeminiConfigBoundary(
      runtimeEnvironment.replace('"PRODEX_GEMINI_LIVE_MODEL",', ""),
      [[RUNTIME_GEMINI_POLICY, geminiPolicy]],
    ).some((error) => error.includes("PRODEX_GEMINI_LIVE_MODEL")),
    "missing Gemini startup key accepted",
  );
  assertSelfTest(
    validateRuntimeGeminiConfigBoundary(runtimeEnvironment, [
      [RUNTIME_GEMINI_POLICY, `${geminiPolicy}\ngemini_settings_source_paths_for(home, cwd);`],
    ]).some((error) => error.includes("compatibility planner")),
    "environment-reading Gemini compatibility planner accepted in request path",
  );
  const oidcEnvironment = RUNTIME_OIDC_ENV_KEYS.map((key) => `"${key}",`).join("\n");
  assertSelfTest(
    validateRuntimeOidcConfigBoundary(oidcEnvironment, [["good.rs", "env::current_dir();"]]).length === 0,
    "typed OIDC runtime config rejected",
  );
  assertSelfTest(
    validateRuntimeOidcConfigBoundary(oidcEnvironment, [["bad.rs", "let value = env::var_os(\"KEY\");"]]).some(
      (error) => error.includes("RuntimeConfig"),
    ),
    "OIDC request-path environment read accepted",
  );
  assertSelfTest(
    validateRuntimeOidcConfigBoundary(
      oidcEnvironment.replace('"PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS",', ""),
      [],
    ).some((error) => error.includes("PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS")),
    "missing OIDC startup key accepted",
  );
  const topologyEnvironment = RUNTIME_GATEWAY_TOPOLOGY_ENV_KEYS.map((key) => `"${key}",`).join("\n");
  const topologySource =
    "let replicas = config.replica_count; let gate = config.require_multi_replica_accounting_checks;";
  const gatewayStartupSource = `
    let runtime_config = Arc::new(RuntimeConfig::from_gateway_env_policy_and_cli(&paths));
    resolve_gateway_launch_config_for_service_mode();
    let refresh_runtime_config = Arc::clone(&runtime_config);
    start_runtime_gateway_rewrite_proxy_with_runtime_config(Arc::clone(&runtime_config));`;
  const topologyHotPaths = [
    [RUNTIME_GATEWAY_TOPOLOGY, topologySource],
    [RUNTIME_GATEWAY_STARTUP, gatewayStartupSource],
  ];
  assertSelfTest(
    validateRuntimeGatewayTopologyConfigBoundary(topologyEnvironment, topologyHotPaths).length === 0,
    "typed gateway topology config rejected",
  );
  assertSelfTest(
    validateRuntimeGatewayTopologyConfigBoundary(topologyEnvironment, [
      [RUNTIME_GATEWAY_TOPOLOGY, `${topologySource} env::var(\"PRODEX_GATEWAY_REPLICA_COUNT\");`],
      [RUNTIME_GATEWAY_STARTUP, gatewayStartupSource],
    ]).some((error) => error.includes("must use RuntimeConfig")),
    "gateway topology environment read accepted",
  );
  const gatewayEnvironment = RUNTIME_GATEWAY_STARTUP_ENV_KEYS.map((key) => `"${key}",`).join("\n");
  const gatewaySources = [
    [RUNTIME_GATEWAY_STARTUP_CONFIG_FILES[0], `
      fn resolve_gateway_launch_config_for_service_mode() {}
      fn resolve_gateway_refresh_candidate_for_service_mode() {}
    `],
    [RUNTIME_GATEWAY_PROVIDER_CONFIG, `
      fn resolve_gateway_provider_config_with_resolver() {}
      fn resolve_gateway_provider_credentials_with_resolver() {}
    `],
    [RUNTIME_GATEWAY_STARTUP_CONFIG_FILES[2], "fn resolve_gateway_auth_config_with_resolver() {}"],
    [RUNTIME_GATEWAY_STARTUP_CONFIG_FILES[3], "fn gateway_state_store_config_with_resolver() {}"],
    [RUNTIME_GATEWAY_SECRET_CONFIG, "fn resolve() { env::var(name); }"],
    [RUNTIME_GATEWAY_STARTUP, `
      RuntimeConfig::from_gateway_env_policy_and_cli();
      resolve_gateway_launch_config_for_service_mode();
      resolve_gateway_refresh_candidate_for_service_mode();
    `],
  ];
  assertSelfTest(
    validateRuntimeGatewayStartupConfigBoundary(gatewayEnvironment, gatewaySources).length === 0,
    "typed gateway startup config rejected",
  );
  assertSelfTest(
    validateRuntimeGatewayStartupConfigBoundary(gatewayEnvironment, gatewaySources.map(([file, source]) => [
      file,
      file === RUNTIME_GATEWAY_PROVIDER_CONFIG
        ? source.replace("fn resolve_gateway_provider_config_with_resolver() {}", "fn resolve_gateway_provider_config_with_resolver() { env::var(\"KEY\"); }")
        : source,
    ])).some((error) => error.includes("must consume RuntimeConfig")),
    "gateway startup environment read accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  const manifest = await fs.readFile(path.join(repoRoot, CONFIG_MANIFEST), "utf8");
  const errors = [
    ...validateConfigManifest(manifest),
    ...(await validateConfigSources()),
    ...(await validateRuntimeGeminiConfigSources()),
    ...(await validateRuntimeOidcConfigSources()),
    ...(await validateRuntimeGatewayTopologyConfigSources()),
    ...(await validateRuntimeGatewayStartupConfigSources()),
  ];
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`config-boundary-guard: ${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
