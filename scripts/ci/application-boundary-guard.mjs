#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const MANIFEST = "crates/prodex-application/Cargo.toml";
const SRC_DIR = "crates/prodex-application/src";
const ALLOWED_DEPENDENCIES = new Set([
  "prodex_authn",
  "prodex_authz",
  "prodex_config",
  "prodex_control_plane",
  "prodex_domain",
  "prodex_gateway_core",
  "prodex_gateway_http",
  "prodex_observability",
  "prodex_provider_core",
  "prodex_provider_spi",
  "prodex_storage",
  "prodex_storage_postgres",
  "prodex_storage_redis",
  "prodex_storage_sqlite",
]);
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
  { name: "CLI", pattern: /\b(clap|prodex_cli)\s*::/u },
  { name: "http framework", pattern: /\b(axum|hyper|tower)\s*::/u },
  { name: "http client", pattern: /\breqwest\s*::/u },
  { name: "database driver", pattern: /\b(postgres|rusqlite|sqlx|redis)\s*::/u },
  { name: "provider SDK", pattern: /\b(openai|anthropic|gemini|copilot)\s*::/u },
  { name: "transport implementation", pattern: /\btungstenite\s*::/u },
]);
const REQUIRED_CONTROL_PLANE_HTTP_BINDING_SNIPPETS = Object.freeze([
  [
    "#![forbid(unsafe_code)]",
    "application boundary must forbid unsafe code",
  ],
  [
    "plan_application_control_plane_http_route",
    "application boundary must expose canonical control-plane HTTP route binding",
  ],
  [
    "fn validate_control_plane_http_action",
    "application boundary must centralize control-plane HTTP action validation",
  ],
  [
    "plan_application_control_plane_idempotency_from_http",
    "application boundary must validate control-plane HTTP idempotency headers",
  ],
  [
    "validate_control_plane_http_action(&action, http)?",
    "precomputed-fingerprint idempotency must validate HTTP route/action binding",
  ],
  [
    "plan_application_control_plane_idempotency_from_http_digest",
    "application boundary must validate control-plane HTTP digest idempotency",
  ],
  [
    "plan_application_control_plane_page_request_from_http_query",
    "application boundary must validate control-plane HTTP pagination query parameters",
  ],
  [
    "page_request_from_query(query.as_ref())",
    "application boundary must delegate control-plane pagination query parsing to gateway HTTP",
  ],
  [
    "plan_application_control_plane_page_request_error_response",
    "application boundary must expose redacted control-plane pagination query responses",
  ],
  [
    "plan_application_control_plane_precondition_from_http",
    "application boundary must validate control-plane HTTP If-Match preconditions",
  ],
  [
    "entity_tag_from_if_match_headers(&http.headers)",
    "application boundary must delegate If-Match parsing to gateway HTTP",
  ],
  [
    "plan_application_control_plane_precondition_error_response",
    "application boundary must expose redacted control-plane precondition responses",
  ],
  [
    "plan_application_control_plane_with_audit_storage_from_http",
    "application boundary must bind control-plane HTTP route validation before audit storage planning",
  ],
  [
    "fn validate_control_plane_http_action_for_audit",
    "application boundary must centralize control-plane audit route/action validation",
  ],
  [
    "route.http.requires_audit",
    "control-plane audit binding must require the HTTP route audit contract",
  ],
  [
    "action.operation.requires_immutable_audit()",
    "control-plane audit binding must require immutable action audit semantics",
  ],
  [
    "plan_application_control_plane_audit_correlation_from_http",
    "application boundary must build audit correlation context from HTTP trace context",
  ],
  [
    "plan_gateway_http_request(request.http_policy, request.http)",
    "audit correlation must reuse gateway HTTP trace parsing and policy validation",
  ],
  [
    "CorrelationContext::new(request.request_id)",
    "audit correlation must start from canonical request id",
  ],
  [
    ".with_audit_event_id(audit_write.event.id)",
    "audit correlation must attach immutable audit event id",
  ],
  [
    "plan_application_control_plane_audit_correlation_error_response",
    "application boundary must expose redacted audit correlation errors",
  ],
  [
    "plan_application_control_plane_audit_emission_span",
    "application boundary must expose an audit emission span planner",
  ],
  [
    "GatewaySpanKind::AuditEmission",
    "audit emission span planner must use the canonical audit emission span kind",
  ],
  [
    "tenant_trace_attribute(tenant_id)",
    "audit emission span planner must carry tenant id as trace-only telemetry",
  ],
  [
    'TelemetryAttribute::trace_only("audit_event_id"',
    "audit emission span planner must carry audit event id as trace-only telemetry",
  ],
  [
    "plan_application_control_plane_audit_emission_span_error_response",
    "application boundary must expose redacted audit emission span errors",
  ],
  [
    "plan_application_control_plane_audit_persistence_span",
    "application boundary must expose an audit persistence span planner",
  ],
  [
    "GatewaySpanKind::Persistence",
    "audit persistence span planner must use the canonical persistence span kind",
  ],
  [
    'TelemetryAttribute::metric_label("storage_backend"',
    "audit persistence span planner must use only low-cardinality storage backend metric labels",
  ],
  [
    "application_control_plane_audit_storage_backend",
    "audit persistence span planner must derive storage backend labels from typed storage plans",
  ],
  [
    "plan_application_control_plane_audit_persistence_span_error_response",
    "application boundary must expose redacted audit persistence span errors",
  ],
  [
    "plan_application_configuration_readiness_snapshot",
    "application boundary must expose configuration-backed readiness snapshot planning",
  ],
  [
    "evaluate_config_refresh(&request.cache_state, request.now_unix_ms)",
    "configuration readiness must reuse config refresh decision semantics",
  ],
  [
    "ConfigRefreshDecision::UseLastKnownGood => request.cache_state.last_known_good_revision_id",
    "configuration readiness must expose last-known-good revision when active config is stale or invalidated",
  ],
  [
    "HealthSnapshot::new(",
    "configuration readiness must build canonical domain health snapshots",
  ],
  [
    "checks.push(application_configuration_readiness_check(refresh_decision))",
    "configuration readiness must add a config health check for refresh decisions",
  ],
  [
    "ConfigRefreshDecision::RefreshAsync => HealthCheck::new(",
    "configuration readiness must report async refresh as a degraded health check",
  ],
  [
    "ConfigRefreshDecision::RefreshRequired => HealthCheck::new(",
    "configuration readiness must report refresh-required as a failing health check",
  ],
  [
    "ApplicationControlPlaneIdempotencyError::OperationMismatch",
    "route/action mismatches must have an explicit application error",
  ],
  [
    'code: "control_plane_route_invalid"',
    "route/action mismatch response must be stable and redacted",
  ],
  [
    'message: "control-plane route is invalid"',
    "route/action mismatch response must not leak operation, tenant, or replay internals",
  ],
  [
    "plan_gateway_control_plane_route_error_response",
    "application boundary must reuse gateway control-plane route error responses",
  ],
  [
    "ApplicationControlPlaneIdempotencyErrorStatus::MethodNotAllowed",
    "application boundary must preserve method-not-allowed status for control-plane route errors",
  ],
  [
    "plan_application_usage_reconciliation_execution",
    "application boundary must own reconciliation execution policy",
  ],
  [
    "ApplicationUsageReconciliationRetryPlan",
    "application boundary must expose a typed reconciliation retry plan",
  ],
  [
    "max_attempts: 25",
    "application boundary must retain the characterized reconciliation attempt budget",
  ],
  [
    "Duration::from_millis(20)",
    "application boundary must retain the characterized reconciliation backoff",
  ],
  [
    "ApplicationUsageReconciliationAuditPlan",
    "application boundary must expose typed reconciliation audit semantics",
  ],
  [
    '"gateway_reconciliation_retry_exhausted"',
    "application boundary must classify pending reconciliation exhaustion",
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
      errors.push(`${manifestPath}: [dependencies] must stay application boundary crates only; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-application cannot depend on forbidden framework/runtime/storage/provider crate '${dep}'`);
    }
  }
  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dev-dependencies] must stay empty; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-application tests cannot depend on forbidden framework/runtime crate '${dep}'`);
    }
  }
  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${manifestPath}: target-specific dependencies are not allowed in prodex-application`);
    }
  }
  return errors;
}

export function validateSource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: prodex-application cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
  return errors;
}

export function validateApplicationBindings(sourceText, sourcePath = `${SRC_DIR}/**/*.rs`) {
  const errors = [];
  for (const [snippet, message] of REQUIRED_CONTROL_PLANE_HTTP_BINDING_SNIPPETS) {
    if (!sourceText.includes(snippet)) {
      errors.push(`${sourcePath}: ${message}; missing '${snippet}'`);
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
  const sources = [];
  for (const file of files) {
    const source = await fs.readFile(file, "utf8");
    const relativePath = path.relative(repoRoot, file);
    errors.push(...validateSource(source, relativePath));
    sources.push(`// ${relativePath}\n${source}`);
  }
  errors.push(...validateApplicationBindings(sources.join("\n")));
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
  const valid = `
[package]
name = "prodex-application"

[dependencies]
prodex_control_plane = { workspace = true }
prodex_authn = { workspace = true }
prodex_authz = { workspace = true }
prodex_domain = { workspace = true }
prodex_gateway_core = { workspace = true }
prodex_gateway_http = { workspace = true }
prodex_observability = { workspace = true }
prodex_provider_core = { workspace = true }
prodex_provider_spi = { workspace = true }
prodex_storage = { workspace = true }
prodex_storage_postgres = { workspace = true }
prodex_storage_redis = { workspace = true }
prodex_storage_sqlite = { workspace = true }
`;
  assertSelfTest(validateManifest(valid, "valid/Cargo.toml").length === 0, "valid manifest rejected");
  assertSelfTest(
    validateManifest(`${valid}\nclap = "4"\n`, "invalid-cli/Cargo.toml").some((error) => error.includes("clap")),
    "CLI dependency accepted",
  );
  assertSelfTest(
    validateManifest(`${valid}\naxum = "0.8"\n`, "invalid-http/Cargo.toml").some((error) => error.includes("axum")),
    "HTTP framework dependency accepted",
  );
  assertSelfTest(
    validateSource("let app = axum::Router::new();", "bad.rs").some((error) => error.includes("http framework")),
    "HTTP framework source accepted",
  );
  assertSelfTest(
    validateSource("use prodex_gateway_core::GatewayAdmissionPlan;", "good.rs").length === 0,
    "safe source rejected",
  );
  const validBindingSource = `
#![forbid(unsafe_code)]
pub fn plan_application_control_plane_http_route() {}
fn validate_control_plane_http_action() {}
pub fn plan_application_control_plane_idempotency_from_http() {
    validate_control_plane_http_action(&action, http)?;
}
pub fn plan_application_control_plane_idempotency_from_http_digest() {
    validate_control_plane_http_action(&action, http)?;
}
pub fn plan_application_control_plane_page_request_from_http_query() {
    page_request_from_query(query.as_ref());
}
pub fn plan_application_control_plane_page_request_error_response() {}
pub fn plan_application_control_plane_precondition_from_http() {
    entity_tag_from_if_match_headers(&http.headers);
}
pub fn plan_application_control_plane_precondition_error_response() {}
pub fn plan_application_control_plane_with_audit_storage_from_http() {}
fn validate_control_plane_http_action_for_audit() {
    route.http.requires_audit;
    action.operation.requires_immutable_audit();
}
pub fn plan_application_control_plane_audit_correlation_from_http() {
    plan_gateway_http_request(request.http_policy, request.http);
    CorrelationContext::new(request.request_id)
        .with_audit_event_id(audit_write.event.id);
}
pub fn plan_application_control_plane_audit_correlation_error_response() {}
pub fn plan_application_control_plane_audit_emission_span() {
    plan_gateway_span(
        GatewaySpanKind::AuditEmission,
        "prodex.control_plane.audit.emit",
        correlation,
        None,
        vec![
            tenant_trace_attribute(tenant_id),
            TelemetryAttribute::trace_only("audit_event_id", audit_event_id.to_string()),
        ],
    );
}
pub fn plan_application_control_plane_audit_emission_span_error_response() {}
pub fn plan_application_control_plane_audit_persistence_span() {
    let storage_backend = application_control_plane_audit_storage_backend(&audit_storage);
    plan_gateway_span(
        GatewaySpanKind::Persistence,
        "prodex.control_plane.audit.persist",
        correlation,
        None,
        vec![
            TelemetryAttribute::metric_label("storage_backend", storage_backend),
            tenant_trace_attribute(tenant_id),
            TelemetryAttribute::trace_only("audit_event_id", audit_event_id.to_string()),
        ],
    );
}
fn application_control_plane_audit_storage_backend() {}
pub fn plan_application_control_plane_audit_persistence_span_error_response() {}
pub fn plan_application_configuration_readiness_snapshot() {
    let refresh_decision = evaluate_config_refresh(&request.cache_state, request.now_unix_ms);
    let active_policy_revision = match refresh_decision {
        ConfigRefreshDecision::UseLastKnownGood => request.cache_state.last_known_good_revision_id,
    };
    let mut checks = request.checks;
    checks.push(application_configuration_readiness_check(refresh_decision));
    HealthSnapshot::new(
        request.live,
        request.startup_complete,
        request.draining,
        active_policy_revision,
        checks,
    );
}
fn application_configuration_readiness_check() {
    match refresh_decision {
        ConfigRefreshDecision::RefreshAsync => HealthCheck::new("configuration", HealthState::Degraded, None),
        ConfigRefreshDecision::RefreshRequired => HealthCheck::new("configuration", HealthState::Failing, None),
    }
}
enum ApplicationControlPlaneIdempotencyError {
    OperationMismatch,
}
let _ = ApplicationControlPlaneIdempotencyError::OperationMismatch;
let _ = Response { code: "control_plane_route_invalid", message: "control-plane route is invalid" };
let _ = plan_gateway_control_plane_route_error_response(error);
let _ = ApplicationControlPlaneIdempotencyErrorStatus::MethodNotAllowed;
struct ApplicationUsageReconciliationRetryPlan;
struct ApplicationUsageReconciliationAuditPlan;
fn plan_application_usage_reconciliation_execution() {
    let max_attempts: 25;
    Duration::from_millis(20);
    let _ = "gateway_reconciliation_retry_exhausted";
}
`;
  assertSelfTest(
    validateApplicationBindings(validBindingSource).length === 0,
    "valid split application binding rejected",
  );
  assertSelfTest(
    validateApplicationBindings(
      validBindingSource.replaceAll("validate_control_plane_http_action(&action, http)?", ""),
    ).some((error) => error.includes("precomputed-fingerprint idempotency")),
    "missing binding across application src/**/*.rs accepted",
  );
  assertSelfTest(
    validateApplicationBindings(
      validBindingSource.replace('message: "control-plane route is invalid"', 'message: format!("{operation:?}")'),
    ).some((error) => error.includes("route/action mismatch response")),
    "route/action mismatch response leak accepted",
  );
  assertSelfTest(
    validateApplicationBindings(
      validBindingSource.replace("let _ = plan_gateway_control_plane_route_error_response(error);", ""),
    ).some((error) => error.includes("gateway control-plane route error")),
    "missing gateway route error response delegation accepted",
  );
  assertSelfTest(
    validateApplicationBindings(validBindingSource.replace("#![forbid(unsafe_code)]", "")).some((error) =>
      error.includes("forbid unsafe code"),
    ),
    "missing application unsafe forbid accepted",
  );
  assertSelfTest(
    validateApplicationBindings(
      validBindingSource.replace("plan_application_usage_reconciliation_execution", "bypassed_reconciliation"),
    ).some((error) => error.includes("reconciliation execution policy")),
    "missing reconciliation execution planner accepted",
  );
  assertSelfTest(
    validateApplicationBindings(
      validBindingSource.replace('"gateway_reconciliation_retry_exhausted"', '"silent_drop"'),
    ).some((error) => error.includes("pending reconciliation exhaustion")),
    "silent pending reconciliation exhaustion accepted",
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
  process.stderr.write(`application-boundary-guard: ${message}\n`);
  process.exitCode = 1;
});
