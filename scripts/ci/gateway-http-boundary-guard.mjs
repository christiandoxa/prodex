#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const MANIFEST = "crates/prodex-gateway-http/Cargo.toml";
const SRC_DIR = "crates/prodex-gateway-http/src";
const ALLOWED_DEPENDENCIES = new Set(["prodex_domain", "prodex_gateway_core", "prodex_observability"]);
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
  "serde_json",
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
  { name: "provider SDK", pattern: /\b(openai|anthropic|gemini|copilot)\s*::/u },
  { name: "transport implementation", pattern: /\btungstenite\s*::/u },
]);
const REQUIRED_CONTROL_PLANE_ROUTE_SNIPPETS = Object.freeze([
  [
    "#![forbid(unsafe_code)]",
    "gateway HTTP boundary must forbid unsafe code",
  ],
  [
    "pub struct GatewayHttpApiVersionPlan",
    "gateway HTTP boundary must expose a framework-neutral API version plan",
  ],
  [
    "pub fn plan_gateway_http_api_version",
    "gateway HTTP boundary must evaluate versioned path API policy",
  ],
  [
    "requested_api_version_for_path(path, default_version)",
    "gateway HTTP API version planner must derive explicit path versions",
  ],
  [
    "evaluate_api_version(requested, policies, now_unix_ms)",
    "gateway HTTP API version planner must delegate lifecycle decisions to domain policy",
  ],
  [
    "pub fn plan_gateway_http_api_version_error_response",
    "gateway HTTP boundary must expose redacted API version error responses",
  ],
  [
    "pub fn entity_tag_from_if_match_headers",
    "gateway HTTP boundary must parse If-Match entity tags for optimistic concurrency",
  ],
  [
    "header.normalized_name() == \"if-match\"",
    "gateway HTTP If-Match parser must use normalized header names",
  ],
  [
    "GatewayHttpEntityTagError::Duplicate",
    "gateway HTTP If-Match parser must reject duplicate precondition headers",
  ],
  [
    "GatewayHttpIdempotencyKeyError::Duplicate",
    "gateway HTTP idempotency parser must reject duplicate idempotency headers",
  ],
  [
    "GatewayHttpPlanError::DuplicateTraceContext",
    "gateway HTTP trace parser must reject duplicate traceparent headers",
  ],
  [
    "pub fn plan_gateway_http_entity_tag_error_response",
    "gateway HTTP boundary must expose redacted entity tag error responses",
  ],
  [
    "pub fn page_request_from_query",
    "gateway HTTP boundary must parse pagination query parameters",
  ],
  [
    "PageRequest::new(limit, cursor)",
    "gateway HTTP pagination planner must delegate page limit semantics to domain PageRequest",
  ],
  [
    "Cursor::new(value.to_string())",
    "gateway HTTP pagination planner must validate cursors with domain Cursor",
  ],
  [
    "GatewayHttpPaginationQueryError::DuplicateLimit",
    "gateway HTTP pagination planner must reject duplicate limit parameters",
  ],
  [
    "GatewayHttpPaginationQueryError::DuplicateCursor",
    "gateway HTTP pagination planner must reject duplicate cursor parameters",
  ],
  [
    "pub fn plan_gateway_http_pagination_query_error_response",
    "gateway HTTP boundary must expose redacted pagination query error responses",
  ],
  [
    "pub struct GatewayHttpDrainPlan",
    "gateway HTTP boundary must expose a framework-neutral drain plan",
  ],
  [
    "pub enum GatewayHttpDrainPlanError",
    "gateway HTTP boundary must expose drain planning errors",
  ],
  [
    "pub fn plan_gateway_http_drain",
    "gateway HTTP boundary must expose the drain planner",
  ],
  [
    "readiness_fails_before_drain: true",
    "gateway HTTP drain plan must require readiness failure before draining",
  ],
  [
    ".saturating_add(prestop_delay_ms)",
    "gateway HTTP drain plan must include preStop delay in required termination grace",
  ],
  [
    "GatewayHttpDrainPlanError::PreStopDelayRequired",
    "gateway HTTP drain plan must reject missing preStop delay",
  ],
  [
    "GatewayHttpDrainPlanError::TerminationGraceTooShort",
    "gateway HTTP drain plan must reject termination grace shorter than drain budget",
  ],
  [
    "pub enum GatewayControlPlaneOperation",
    "gateway HTTP boundary must expose explicit control-plane operations",
  ],
  [
    "pub fn plan_control_plane_route",
    "gateway HTTP boundary must expose the control-plane route planner",
  ],
  [
    "pub fn plan_gateway_control_plane_route_error_response",
    "gateway HTTP boundary must expose redacted control-plane route error responses",
  ],
  [
    "GatewayControlPlaneRouteError::NotControlPlaneRoute",
    "control-plane route planner must reject data-plane and unknown mounts",
  ],
  [
    "GatewayControlPlaneRouteError::MethodNotAllowed",
    "control-plane route planner must fail closed on unsupported methods",
  ],
  [
    "matches_control_plane_mount(path, \"/admin\")",
    "control-plane route classifier must preserve segment-boundary admin mount matching",
  ],
  [
    "matches_control_plane_mount(path, \"/scim\")",
    "control-plane route classifier must preserve segment-boundary SCIM mount matching",
  ],
  [
    "strip_control_plane_mount(path, \"/v1/admin\")",
    "control-plane route planner must support versioned admin routes without prefix confusion",
  ],
  [
    "strip_control_plane_mount(path, \"/v1/scim\")",
    "control-plane route planner must support versioned SCIM routes without prefix confusion",
  ],
  [
    "control_plane_operation_allows_method",
    "control-plane route planner must enforce method-specific operations",
  ],
  [
    "requires_idempotency: operation.requires_idempotency()",
    "control-plane route plan must surface idempotency requirements",
  ],
  [
    "requires_audit: operation.requires_audit()",
    "control-plane route plan must surface audit requirements",
  ],
  [
    'code: "control_plane_route_invalid"',
    "control-plane route errors must use a stable redacted invalid-route code",
  ],
  [
    'message: "control-plane route is invalid"',
    "control-plane invalid-route response must not leak paths or operation internals",
  ],
  [
    'code: "control_plane_method_not_allowed"',
    "control-plane route method errors must use a stable redacted method code",
  ],
  [
    'message: "HTTP method is not allowed for this control-plane route"',
    "control-plane method response must not leak operations or method internals",
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
    if (char === "#" && !inString) return line.slice(0, index).trim();
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
      errors.push(`${manifestPath}: [dependencies] must stay HTTP policy boundary crates only; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-gateway-http cannot depend on forbidden framework/runtime/storage/provider crate '${dep}'`);
    }
  }
  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dev-dependencies] must stay empty; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-gateway-http tests cannot depend on forbidden framework/runtime crate '${dep}'`);
    }
  }
  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${manifestPath}: target-specific dependencies are not allowed in prodex-gateway-http`);
    }
  }
  return errors;
}

export function validateSource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: prodex-gateway-http cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
  if (sourcePath.endsWith("crates/prodex-gateway-http/src/lib.rs") || sourcePath === "src/lib.rs") {
    for (const [snippet, message] of REQUIRED_CONTROL_PLANE_ROUTE_SNIPPETS) {
      if (!sourceText.includes(snippet)) {
        errors.push(`${sourcePath}: ${message}; missing '${snippet}'`);
      }
    }
    if (sourceText.includes('path.starts_with("/admin")') || sourceText.includes('path.starts_with("/scim")')) {
      errors.push(`${sourcePath}: control-plane route matching must stay segment-boundary based, not raw starts_with`);
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
name = "prodex-gateway-http"

[dependencies]
prodex_domain = { workspace = true }
prodex_gateway_core = { workspace = true }
prodex_observability = { workspace = true }
`;
  assertSelfTest(validateManifest(valid, "valid/Cargo.toml").length === 0, "valid manifest rejected");
  assertSelfTest(
    validateManifest(`${valid}\naxum = "0.8"\n`, "invalid/Cargo.toml").some((error) => error.includes("axum")),
    "HTTP framework dependency accepted",
  );
  assertSelfTest(
    validateManifest(`${valid}\ntokio = "1"\n`, "invalid-runtime/Cargo.toml").some((error) => error.includes("tokio")),
    "async runtime dependency accepted",
  );
  assertSelfTest(
    validateSource("let app = axum::Router::new();", "bad.rs").some((error) => error.includes("http framework")),
    "HTTP framework source accepted",
  );
  assertSelfTest(
    validateSource("use prodex_gateway_core::GatewayAdmissionPlan;", "good.rs").length === 0,
    "safe source rejected",
  );
  const validRouteSource = `
#![forbid(unsafe_code)]
pub struct GatewayHttpApiVersionPlan {}
pub fn plan_gateway_http_api_version() {
    requested_api_version_for_path(path, default_version);
    evaluate_api_version(requested, policies, now_unix_ms);
}
pub fn plan_gateway_http_api_version_error_response() {}
pub fn entity_tag_from_if_match_headers() {
    header.normalized_name() == "if-match";
    GatewayHttpEntityTagError::Duplicate;
}
pub fn idempotency_key_from_headers() {
    GatewayHttpIdempotencyKeyError::Duplicate;
}
pub fn trace_context_from_headers() {
    GatewayHttpPlanError::DuplicateTraceContext;
}
pub fn plan_gateway_http_entity_tag_error_response() {}
pub fn page_request_from_query() {
    Cursor::new(value.to_string());
    GatewayHttpPaginationQueryError::DuplicateLimit;
    GatewayHttpPaginationQueryError::DuplicateCursor;
    PageRequest::new(limit, cursor);
}
pub fn plan_gateway_http_pagination_query_error_response() {}
pub struct GatewayHttpDrainPlan {}
pub enum GatewayHttpDrainPlanError {}
pub fn plan_gateway_http_drain() {
    readiness_fails_before_drain: true;
    if prestop_delay_ms == 0 {
        return Err(GatewayHttpDrainPlanError::PreStopDelayRequired);
    }
    policy.connection_drain_timeout_ms
        .saturating_add(prestop_delay_ms);
    GatewayHttpDrainPlanError::PreStopDelayRequired;
    GatewayHttpDrainPlanError::TerminationGraceTooShort;
}
pub enum GatewayControlPlaneOperation {}
pub fn plan_control_plane_route() {
    GatewayControlPlaneRouteError::NotControlPlaneRoute;
    GatewayControlPlaneRouteError::MethodNotAllowed;
    matches_control_plane_mount(path, "/admin");
    matches_control_plane_mount(path, "/scim");
    strip_control_plane_mount(path, "/v1/admin");
    strip_control_plane_mount(path, "/v1/scim");
    control_plane_operation_allows_method(operation, method);
    requires_idempotency: operation.requires_idempotency();
    requires_audit: operation.requires_audit();
}
pub fn plan_gateway_control_plane_route_error_response() {
    code: "control_plane_route_invalid";
    message: "control-plane route is invalid";
    code: "control_plane_method_not_allowed";
    message: "HTTP method is not allowed for this control-plane route";
}
`;
  assertSelfTest(
    validateSource(validRouteSource, "crates/prodex-gateway-http/src/lib.rs").length === 0,
    "valid control-plane route planner rejected",
  );
  assertSelfTest(
    validateSource(
      validRouteSource.replace("control_plane_operation_allows_method(operation, method);", ""),
      "crates/prodex-gateway-http/src/lib.rs",
    ).some((error) => error.includes("method-specific operations")),
    "missing method-specific route gate accepted",
  );
  assertSelfTest(
    validateSource(
      `${validRouteSource}\nlet admin = path.starts_with("/admin");`,
      "crates/prodex-gateway-http/src/lib.rs",
    ).some((error) => error.includes("segment-boundary")),
    "raw prefix control-plane route matching accepted",
  );
  assertSelfTest(
    validateSource(
      validRouteSource.replace('message: "control-plane route is invalid";', 'message: format!("{path}")'),
      "crates/prodex-gateway-http/src/lib.rs",
    ).some((error) => error.includes("invalid-route response")),
    "control-plane route error response leak accepted",
  );
  assertSelfTest(
    validateSource(
      validRouteSource.replace("#![forbid(unsafe_code)]", ""),
      "crates/prodex-gateway-http/src/lib.rs",
    ).some((error) => error.includes("forbid unsafe code")),
    "missing gateway-http unsafe forbid accepted",
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
  process.stderr.write(`gateway-http-boundary-guard: ${message}\n`);
  process.exitCode = 1;
});
