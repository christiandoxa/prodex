#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const FILES = Object.freeze({
  root: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite.rs",
  admin: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_router.rs",
  adminExecution:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_execution.rs",
  adminAuth:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/admin.rs",
  adminKeys:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_keys.rs",
  adminScim:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_scim.rs",
  adminScope:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_scope.rs",
  adminToctouTests:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_admin_toctou.rs",
  adapter:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_boundary.rs",
  dataPlaneAdapter:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_data_plane.rs",
  directRuntime:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_runtime.rs",
  serve: "src/enterprise_serve.rs",
  providerConfig:
    "crates/prodex-app/src/app_commands/runtime_launch/gateway_provider_config.rs",
  providerAdapter:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_transport/projected_credential.rs",
  application: "crates/prodex-application/src/request_context.rs",
  gatewayRoute: "crates/prodex-gateway-http/src/route.rs",
  applicationBoundaryTests:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_application_boundary.rs",
  authn: "crates/prodex-authn/src/evidence.rs",
  oidcAdapter:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_auth/token_claims.rs",
  pipeline:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline.rs",
  governance:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline_governance.rs",
  dispatch:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline_dispatch.rs",
  providerSender:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_upstream.rs",
  providerErrorPolicy:
    "crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge_error_policy.rs",
  providerCopilot:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_copilot.rs",
  providerDeepSeek:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_deepseek_send.rs",
  providerGeminiOpenAi:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_openai.rs",
  providerGemini:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_send.rs",
  applicationProvider: "crates/prodex-application/src/provider.rs",
  providerSpi: "crates/prodex-provider-spi/src/lib.rs",
  keys: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_keys.rs",
  ledger: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_ledger.rs",
  reconciliationWorker:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_reconciliation_worker.rs",
  reconciliationAudit:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_reconciliation_audit.rs",
  responseSpend:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_response_spend.rs",
  observability:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_transport/observability.rs",
  modules: "crates/prodex-app/src/runtime_launch/proxy_startup.rs",
});

function functionBody(source, name) {
  const start = source.indexOf(`fn ${name}`);
  if (start < 0) return undefined;
  const open = source.indexOf("{", start);
  if (open < 0) return undefined;
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") depth -= 1;
    if (depth === 0) return source.slice(open + 1, index);
  }
  return undefined;
}

function requireText(errors, source, needle, message) {
  if (!source.includes(needle)) errors.push(message);
}

function forbidText(errors, source, needle, message) {
  if (source.includes(needle)) errors.push(message);
}

function requireBefore(errors, source, before, after, message) {
  const beforeIndex = source.indexOf(before);
  const afterIndex = source.indexOf(after);
  if (beforeIndex < 0 || afterIndex < 0 || beforeIndex >= afterIndex) errors.push(message);
}

function requireOrdered(errors, source, needles, message) {
  for (let index = 1; index < needles.length; index += 1) {
    requireBefore(errors, source, needles[index - 1], needles[index], message);
  }
}

function requireCount(errors, source, needle, expected, message) {
  const actual = source.split(needle).length - 1;
  if (actual !== expected) errors.push(message);
}

function requirePattern(errors, source, pattern, message) {
  if (!pattern.test(source)) errors.push(message);
}

export function validateProductionBoundary(sources) {
  const errors = [];
  const root = functionBody(sources.root, "handle_runtime_local_rewrite_proxy_request");
  if (!root) {
    errors.push(`${FILES.root}: production gateway handler is missing`);
  } else {
    requireText(
      errors,
      root,
      "run_runtime_local_rewrite_pipeline(RuntimeLocalRewriteRequest::tiny(request), target, shared);",
      `${FILES.root}: production handler must delegate to the typed request pipeline`,
    );
  }

  const pipeline = functionBody(sources.pipeline, "try_run_runtime_local_rewrite_pipeline");
  if (!pipeline) {
    errors.push(`${FILES.pipeline}: typed production pipeline is missing`);
  } else {
    requireOrdered(
      errors,
      pipeline,
      [
        "runtime_local_rewrite_canonical_context(request, target, shared)",
        "runtime_local_rewrite_authenticate(canonical, shared)",
        "runtime_local_rewrite_bounded_admission(authenticated, shared)",
        "runtime_local_rewrite_dispatch_websocket(admitted, shared)",
        "runtime_local_rewrite_capture_body(admitted, shared)",
        "runtime_local_rewrite_prepare_constraints(captured, shared)",
        "runtime_local_rewrite_dispatch_control_plane(prepared, shared)",
        "runtime_local_rewrite_pre_reservation_governance(prepared, shared)",
        "runtime_local_rewrite_reserve_virtual_key(governed, shared)",
        "runtime_local_rewrite_post_reservation_governance(reserved, shared)",
        "runtime_local_rewrite_apply_constraints(reserved)",
        "runtime_local_rewrite_dispatch_provider(ready, shared)",
      ],
      `${FILES.pipeline}: canonical/auth/admission/governance/reservation/dispatch stages must remain ordered`,
    );
  }
  for (const stageType of [
    "struct RuntimeLocalRewriteCanonicalRequest",
    "struct RuntimeLocalRewriteAuthenticatedRequest",
    "struct RuntimeLocalRewriteAdmittedRequest",
    "struct RuntimeLocalRewriteCapturedRequest",
    "struct RuntimeLocalRewritePreparedRequest",
    "struct RuntimeLocalRewriteGovernedRequest",
    "struct RuntimeLocalRewriteReservedRequest",
    "struct RuntimeLocalRewriteDispatchReadyRequest",
    "enum RuntimeLocalRewritePipelineExit",
    "Rejected(Box<RuntimeLocalRewritePipelineReply>)",
  ]) {
    requireText(
      errors,
      sources.pipeline,
      stageType,
      `${FILES.pipeline}: production stages must retain typed input/output/rejection state`,
    );
  }

  const canonical = functionBody(sources.pipeline, "runtime_local_rewrite_canonical_context");
  requireOrdered(
    errors,
    canonical ?? "",
    [
      "runtime_proxy_next_request_id(&shared.runtime_shared)",
      "let typed_request_id = RequestId::new();",
      "let started_at = Instant::now();",
      "Duration::from_millis(runtime_gateway_application_http_policy(shared).request_timeout_ms)",
      "ApplicationRequestDeadline::at(",
      "let header_request = request.header_request();",
      "runtime_gateway_application_request_context(",
    ],
    `${FILES.pipeline}: canonical stage must construct one typed request context from the request sequence, monotonic deadline, and original headers`,
  );
  requirePattern(
    errors,
    canonical ?? "",
    /runtime_gateway_application_request_context\(\s*target,\s*typed_request_id,\s*deadline,\s*&header_request\.headers,\s*\)/s,
    `${FILES.pipeline}: canonical request-context planning must receive the exact target, typed request ID, deadline, and original headers`,
  );
  requireCount(
    errors,
    canonical ?? "",
    "RequestId::new()",
    1,
    `${FILES.pipeline}: canonical stage must generate the typed request ID exactly once`,
  );
  forbidText(
    errors,
    canonical ?? "",
    "SystemTime",
    `${FILES.pipeline}: application deadline must use a monotonic clock`,
  );
  const directHandle = functionBody(sources.directRuntime, "handle") ?? "";
  requireOrdered(
    errors,
    directHandle,
    [
      "try_acquire_gateway_request_permit(",
      "let GatewayHandlerRequest {",
      "target,",
      "tokio::spawn(async move",
      "tokio::task::spawn_blocking(move",
      "run_runtime_local_rewrite_pipeline(request, target, &shared)",
    ],
    `${FILES.directRuntime}: direct requests must acquire bounded admission and move the front's exact target into the pipeline`,
  );
  forbidText(
    errors,
    sources.directRuntime,
    "CanonicalRequestTarget::parse",
    `${FILES.directRuntime}: direct requests must not reparse the canonical target`,
  );
  requireCount(
    errors,
    directHandle,
    "run_runtime_local_rewrite_pipeline(request, target, &shared)",
    1,
    `${FILES.directRuntime}: direct requests must enter the typed pipeline exactly once`,
  );
  const serve = functionBody(sources.serve, "run_enterprise_serve(") ?? "";
  requireOrdered(
    errors,
    serve,
    [
      "start_policy_gateway_application_for_mode(policy_mode)",
      "serve_with_handler(",
      "application.handle(request).await",
      "application.shutdown_and_drain(drain_timeout)",
    ],
    `${FILES.serve}: dedicated serving must dispatch in process and drain the owned application`,
  );
  for (const forbidden of [
    "start_policy_gateway_backend",
    "backend.listen_addr()",
    '"127.0.0.1:0"',
  ]) {
    forbidText(
      errors,
      serve,
      forbidden,
      `${FILES.serve}: dedicated serving must not restore loopback backend transport`,
    );
  }
  requireText(
    errors,
    sources.pipeline,
    "prodex_gateway_http::plan_gateway_http_error_response(&error)",
    `${FILES.pipeline}: request-context metadata failures must retain canonical gateway HTTP status mapping`,
  );
  const adminAuth = functionBody(sources.pipeline, "runtime_local_rewrite_preauthorize_admin") ?? "";
  requireBefore(
    errors,
    adminAuth,
    "runtime_gateway_admin_auth(",
    "runtime_gateway_admin_preauthorization(",
    `${FILES.pipeline}: verified admin credentials must enter application preauthorization`,
  );
  const dataAuth = functionBody(sources.pipeline, "runtime_local_rewrite_authorize_data_plane") ?? "";
  requireBefore(
    errors,
    dataAuth,
    "runtime_local_rewrite_verified_virtual_key(",
    "runtime_gateway_application_data_plane_authorization(",
    `${FILES.pipeline}: verified data credentials must enter application authorization`,
  );
  const capture = functionBody(sources.pipeline, "runtime_local_rewrite_capture_body") ?? "";
  requireText(
    errors,
    capture,
    "captured.path_and_query = state.context.target().path_and_query().to_string();",
    `${FILES.pipeline}: forwarding must reuse the canonical application target`,
  );
  const localAdmission =
    functionBody(sources.pipeline, "runtime_local_rewrite_bounded_admission") ?? "";
  requireBefore(
    errors,
    localAdmission,
    "runtime_gateway_application_local_admission(application, shared)",
    "acquire_runtime_proxy_active_request_slot_with_wait(",
    `${FILES.pipeline}: application execution planning must authorize local bounded admission`,
  );
  const control = functionBody(sources.governance, "runtime_local_rewrite_dispatch_control_plane") ?? "";
  requireText(
    errors,
    control,
    "&request.state.context",
    `${FILES.governance}: admin dispatch must receive the canonical application context`,
  );
  forbidText(
    errors,
    control,
    "request.state.context.clone()",
    `${FILES.governance}: admin dispatch must reuse rather than clone the canonical application context`,
  );
  requireText(
    errors,
    sources.governance,
    "runtime_gateway_virtual_key_admission(",
    `${FILES.governance}: typed pipeline must retain virtual-key reservation`,
  );
  const providerDispatch =
    functionBody(sources.dispatch, "runtime_local_rewrite_dispatch_provider") ?? "";
  requireOrdered(
    errors,
    providerDispatch,
    [
      "runtime_gateway_application_provider_dispatch(&request.application_admission)",
      "send_runtime_local_rewrite_upstream_request(",
      "&provider_dispatch",
    ],
    `${FILES.dispatch}: the concrete sender must consume the application provider dispatch plan`,
  );
  const providerPlan =
    functionBody(sources.dataPlaneAdapter, "runtime_gateway_application_provider_dispatch") ?? "";
  requirePattern(
    errors,
    providerPlan,
    /RuntimeGatewayApplicationAdmissionKind::TenantBound\(plan\)[\s\S]*?&plan\.admission\.provider_invocation/,
    `${FILES.dataPlaneAdapter}: tenant-bound provider dispatch must borrow the admitted application invocation`,
  );
  requirePattern(
    errors,
    sources.dataPlaneAdapter,
    /struct RuntimeGatewayApplicationAdmission\(RuntimeGatewayApplicationAdmissionKind\);[\s\S]*?CompatibilityAnonymous\(RuntimeGatewayCompatibilityProviderInvocation\)/,
    `${FILES.dataPlaneAdapter}: anonymous compatibility must remain explicit and non-forgeable outside the adapter`,
  );
  const providerSender =
    functionBody(sources.providerSender, "send_runtime_local_rewrite_upstream_request") ?? "";
  requireOrdered(
    errors,
    providerSender,
    [
      "let provider = dispatch.provider();",
      "let endpoint = dispatch.endpoint();",
      "let stream_mode = dispatch.stream_mode();",
      "match (provider, &shared.provider)",
    ],
    `${FILES.providerSender}: sender must select the configured adapter from the application provider, endpoint, and stream plan`,
  );
  forbidText(
    errors,
    providerSender,
    "runtime_provider_route_kind(",
    `${FILES.providerSender}: sender must not re-derive the planned endpoint from the raw path`,
  );
  requireText(
    errors,
    sources.dataPlaneAdapter,
    "plan_application_provider_retry(ApplicationProviderRetryRequest",
    `${FILES.dataPlaneAdapter}: provider retry decisions must enter the canonical application planner`,
  );
  for (const [source, file] of [
    [sources.providerSender, FILES.providerSender],
    [sources.providerCopilot, FILES.providerCopilot],
    [sources.providerDeepSeek, FILES.providerDeepSeek],
    [sources.providerGeminiOpenAi, FILES.providerGeminiOpenAi],
    [sources.providerGemini, FILES.providerGemini],
  ]) {
    requireText(
      errors,
      source,
      "runtime_gateway_application_provider_retry_precommit(",
      `${file}: provider retry loops must consume the application retry plan`,
    );
  }
  for (const forbidden of [
    "runtime_provider_should_retry_with_next_model",
    "runtime_provider_should_rotate_auth_after_response",
    "runtime_copilot_should_rotate_after_response",
  ]) {
    for (const [source, file] of [
      [sources.providerErrorPolicy, FILES.providerErrorPolicy],
      [sources.providerSender, FILES.providerSender],
      [sources.providerCopilot, FILES.providerCopilot],
      [sources.providerDeepSeek, FILES.providerDeepSeek],
      [sources.providerGeminiOpenAi, FILES.providerGeminiOpenAi],
      [sources.providerGemini, FILES.providerGemini],
    ]) {
      forbidText(
        errors,
        source,
        forbidden,
        `${file}: duplicate provider retry policy ${forbidden} is forbidden`,
      );
    }
  }
  for (const required of [
    "ProviderRetryCause::NextModel",
    "ProviderRetryCause::RotateCredential",
    "ProviderRetryDecision::DeniedNotRetryable",
  ]) {
    requireText(
      errors,
      sources.providerSpi,
      required,
      `${FILES.providerSpi}: canonical provider retry causes and denial must remain typed`,
    );
  }
  requireText(
    errors,
    sources.applicationProvider,
    "request.cause",
    `${FILES.applicationProvider}: application provider retry must forward the typed retry cause`,
  );

  const virtualKeyAdmission =
    functionBody(sources.keys, "runtime_gateway_virtual_key_admission") ?? "";
  const virtualKeyPlanner =
    functionBody(sources.keys, "runtime_gateway_application_virtual_key_admission") ?? "";
  requireText(
    errors,
    virtualKeyPlanner,
    "plan_application_virtual_key_admission(",
    `${FILES.keys}: virtual-key policy must enter the canonical application planner`,
  );
  requireOrdered(
    errors,
    virtualKeyAdmission,
    [
      "runtime_gateway_application_virtual_key_admission(",
      "runtime_gateway_application_data_plane_admission(",
      "runtime_gateway_distributed_rate_limit_admission(",
      "runtime_gateway_try_durable_reservation(",
      "apply_gateway_virtual_key_usage_update(",
    ],
    `${FILES.keys}: virtual-key policy planning must precede application admission, Redis/durable execution, and local usage writes`,
  );
  requireBefore(
    errors,
    virtualKeyAdmission,
    "runtime_gateway_application_data_plane_admission(",
    "runtime_gateway_try_durable_reservation(",
    `${FILES.keys}: application data-plane admission must precede durable reservation execution`,
  );
  for (const duplicate of [
    "runtime_proxy_crate::runtime_gateway_virtual_key_admission(",
    "runtime_proxy_crate::runtime_gateway_record_virtual_key_usage(",
  ]) {
    forbidText(
      errors,
      sources.keys,
      duplicate,
      `${FILES.keys}: production virtual-key admission must not bypass the canonical application policy`,
    );
  }
  const durableReservation =
    functionBody(sources.keys, "runtime_gateway_try_durable_reservation") ?? "";
  requireBefore(
    errors,
    durableReservation,
    "plan_application_atomic_reservation(",
    "runtime_gateway_sqlite_reserve_usage(",
    `${FILES.keys}: application storage planning must precede durable reservation execution`,
  );
  requireText(
    errors,
    durableReservation,
    "ApplicationAtomicReservationStoragePlan::Postgres(storage)",
    `${FILES.keys}: PostgreSQL durable reservation must consume the application storage plan`,
  );
  const durableReconciliation =
    functionBody(sources.ledger, "runtime_gateway_durable_reconcile_response") ?? "";
  requireBefore(
    errors,
    durableReconciliation,
    "runtime_gateway_application_usage_reconciliation(",
    "runtime_gateway_sqlite_reconcile_usage(",
    `${FILES.ledger}: application usage reconciliation must precede durable settlement`,
  );
  forbidText(
    errors,
    durableReconciliation,
    "plan_sqlite_usage_reconciliation(",
    `${FILES.ledger}: durable settlement must not bypass application storage planning`,
  );
  const reconciliationSchedule =
    functionBody(sources.reconciliationWorker, "schedule_runtime_gateway_billing_ledger_reconcile") ?? "";
  requireOrdered(
    errors,
    reconciliationSchedule,
    [
      "runtime_gateway_application_reconciliation_execution(",
      "spawn_blocking(move ||",
      "reconciliation.retry.attempts()",
      "runtime_gateway_reconciliation_retry_sleep(reconciliation, attempt)",
      "runtime_gateway_record_reconciliation_audit(",
    ],
    `${FILES.reconciliationWorker}: every backend must consume application reconciliation policy before retry and audit side effects`,
  );
  for (const duplicate of ["0..25", "Duration::from_millis(20)"]) {
    forbidText(
      errors,
      sources.reconciliationWorker,
      duplicate,
      `${FILES.reconciliationWorker}: app-local reconciliation retry policy '${duplicate}' is forbidden`,
    );
  }
  requireText(
    errors,
    reconciliationSchedule,
    "reconciliation.exhausted(storage_error_observed)",
    `${FILES.reconciliationWorker}: exhausted reconciliation schedules must receive an application failure classification`,
  );
  for (const failureSideEffect of [
    "ApplicationUsageReconciliationAuditOutcome::Failure",
    '"gateway_billing_ledger_reconcile_failed"',
  ]) {
    requireText(
      errors,
      reconciliationSchedule,
      failureSideEffect,
      `${FILES.reconciliationWorker}: exhausted pending reconciliation must emit failure audit and log side effects`,
    );
  }
  const reconciliationAudit =
    functionBody(sources.reconciliationAudit, "runtime_gateway_audit_usage_reconciliation") ?? "";
  for (const getter of [
    "plan.component()",
    "plan.action()",
    "plan.outcome()",
    "plan.backend()",
    "plan.reason()",
  ]) {
    requireText(
      errors,
      reconciliationAudit,
      getter,
      `${FILES.reconciliationAudit}: reconciliation audit serialization must consume the canonical application plan`,
    );
  }
  const reconciliationAuditCaller =
    functionBody(sources.reconciliationWorker, "runtime_gateway_record_reconciliation_audit") ?? "";
  requireText(
    errors,
    reconciliationAuditCaller,
    "runtime_gateway_audit_usage_reconciliation(runtime_shared, audit).is_err()",
    `${FILES.reconciliationWorker}: reconciliation audit write failures must be handled explicitly`,
  );
  requireText(
    errors,
    reconciliationAuditCaller,
    "gateway_usage_reconciliation_audit_failed",
    `${FILES.reconciliationWorker}: reconciliation audit write failures must emit a stable runtime log marker`,
  );
  for (const backend of [
    "ApplicationUsageReconciliationBackend::File",
    "ApplicationUsageReconciliationBackend::Sqlite",
    "ApplicationUsageReconciliationBackend::Postgres",
    "ApplicationUsageReconciliationBackend::Redis",
  ]) {
    requireText(
      errors,
      sources.dataPlaneAdapter,
      backend,
      `${FILES.dataPlaneAdapter}: every reconciliation backend must enter application execution planning`,
    );
  }
  requireText(
    errors,
    sources.dataPlaneAdapter,
    "plan_application_usage_reconciliation_execution(",
    `${FILES.dataPlaneAdapter}: compatibility settlement must enter application reconciliation execution planning`,
  );
  const streamCommit = functionBody(sources.responseSpend, "commit_stream") ?? "";
  requireText(
    errors,
    streamCommit,
    "runtime_gateway_application_provider_stage_is_committed(",
    `${FILES.responseSpend}: first-byte stream commit must enter application retry policy`,
  );
  const streamRead = functionBody(sources.responseSpend, "read") ?? "";
  requireBefore(
    errors,
    streamRead,
    "self.commit_stream()?",
    "self.observe_chunk(&buf[..read])",
    `${FILES.responseSpend}: stream commit must be fixed before observing provider bytes`,
  );
  requireText(
    errors,
    sources.responseSpend,
    "ReservationReconciliationReason::StreamInterrupted",
    `${FILES.responseSpend}: partial stream errors must retain typed reconciliation reason`,
  );
  requireText(
    errors,
    sources.responseSpend,
    "ReservationReconciliationReason::Cancelled",
    `${FILES.responseSpend}: cancelled streams must retain typed reconciliation reason`,
  );
  forbidText(
    errors,
    sources.observability,
    "schedule_runtime_gateway_durable_reconcile",
    `${FILES.observability}: response accounting must not schedule duplicate durable settlement`,
  );

  const admin = functionBody(sources.admin, "runtime_gateway_admin_response");
  if (!admin) {
    errors.push(`${FILES.admin}: production admin handler is missing`);
  } else {
    requireBefore(
      errors,
      admin,
      "let Some(preauthorized) = preauthorized",
      "runtime_gateway_admin_boundary_response(",
      `${FILES.admin}: application preauthorization must be present before admin use cases run`,
    );
    requireText(
      errors,
      admin,
      "preauthorized.control_plane_action()",
      `${FILES.admin}: admin dispatch must consume the exact application authorization plan`,
    );
    requireText(
      errors,
      admin,
      "let authorized_action = preauthorized.control_plane_action();",
      `${FILES.admin}: admin dispatch must retain the preauthorized action before mutation execution`,
    );
    requireText(
      errors,
      admin,
      "runtime_gateway_admin_create_key_response(",
      `${FILES.admin}: admin key create must route through production key mutation handlers`,
    );
    requireText(
      errors,
      admin,
      "runtime_gateway_admin_update_key_response(",
      `${FILES.admin}: admin key update must route through production key mutation handlers`,
    );
    requireText(
      errors,
      admin,
      "runtime_gateway_admin_delete_key_response(",
      `${FILES.admin}: admin key delete must route through production key mutation handlers`,
    );
    requireText(
      errors,
      admin,
      "runtime_gateway_admin_scim_create_user_response(",
      `${FILES.admin}: admin SCIM mutations must execute through the application-identity mutation execution path`,
    );
  }
  requireText(
    errors,
    sources.admin,
    "RuntimeGatewayAdminPreauthorization<'_>",
    `${FILES.admin}: admin dispatch must require the preauthorization wrapper`,
  );
  forbidText(
    errors,
    sources.admin,
    "runtime_gateway_admin_auth(",
    `${FILES.admin}: admin dispatch must not repeat legacy credential verification`,
  );
  forbidText(
    errors,
    sources.admin,
    "runtime_gateway_admin_write_authorized",
    `${FILES.admin}: duplicate legacy role policy must not be reintroduced`,
  );
  forbidText(
    errors,
    sources.admin,
    "let admin_write = (path == keys_path",
    `${FILES.admin}: legacy path/method mutation policy must not shadow the application operation`,
  );
  const adminExecution =
    functionBody(sources.adminExecution, "runtime_gateway_admin_mutation_execution") ?? "";
  requireText(
    errors,
    adminExecution,
    "runtime_gateway_admin_control_plane_action_for_operation(&http, admin_auth, operation)",
    `${FILES.adminExecution}: mutation execution must retain the control-plane base action and operation`,
  );
  requireText(
    errors,
    adminExecution,
    "plan_application_control_plane_audit_from_http(action.clone(), &http)",
    `${FILES.adminExecution}: mutation execution must run canonical control-plane audit planning`,
  );
  requireText(
    errors,
    adminExecution,
    "plan_application_control_plane_idempotency_from_http_digest(",
    `${FILES.adminExecution}: mutation execution must run canonical control-plane idempotency planning`,
  );
  requireText(
    errors,
    adminExecution,
    ".operation",
    `${FILES.adminExecution}: mutation execution must keep exact operation retention and failure-closed semantics`,
  );
  requireText(
    errors,
    adminExecution,
    "action.principal.id == base.audit_event.principal_id",
    `${FILES.adminExecution}: mutation execution must retain exact principal/tenant/resource identity when reusing the base action`,
  );
  requireText(
    errors,
    adminExecution,
    "action.resource.tenant_id == base.tenant.tenant_id",
    `${FILES.adminExecution}: mutation execution must retain exact principal/tenant/resource identity when reusing the base action`,
  );
  requireText(
    errors,
    adminExecution,
    "action.resource.id == base.audit_event.resource.id",
    `${FILES.adminExecution}: mutation execution must retain exact principal/tenant/resource identity when reusing the base action`,
  );
  forbidText(
    errors,
    sources.root,
    "gateway_admin_idempotency_keys",
    `${FILES.root}: gateway_admin_idempotency_keys preclaim must be removed for control-plane mutations`,
  );
  forbidText(
    errors,
    sources.admin,
    "BTreeSet",
    `${FILES.admin}: control-plane preclaim map must not use in-process BTreeSet`,
  );

  for (const [needle, message] of [
    [
      "plan_application_request_context(target, request_id, deadline, &headers)",
      `${FILES.adapter}: credential adapter must invoke application request-context planning`,
    ],
    [
      "plan_application_request_authentication_from_evidence(",
      `${FILES.adapter}: credential evidence must invoke application authentication`,
    ],
    [
      "plan_application_data_plane_authorization(",
      `${FILES.adapter}: credential evidence must invoke data-plane authorization`,
    ],
    [
      "plan_application_control_plane_authorization(",
      `${FILES.adapter}: credential evidence must invoke control-plane authorization`,
    ],
    [
      "RuntimeGatewayAdminPreauthorization",
      `${FILES.adapter}: credential adapter must carry control-plane preauthorization`,
    ],
    [
      "CredentialScope::DataPlane",
      `${FILES.adapter}: credential adapter must construct typed data-plane principals`,
    ],
    [
      "CredentialScope::ControlPlane",
      `${FILES.adapter}: credential adapter must construct typed control-plane principals`,
    ],
    [
      "VerifiedCredentialEvidence::Principal",
      `${FILES.adapter}: verified bearer, admin, and virtual-key principals must use typed evidence`,
    ],
    [
      "VerifiedCredentialEvidence::Oidc",
      `${FILES.adapter}: verified OIDC credentials must use typed evidence`,
    ],
    [
      "token.canonical_claims(principal_id",
      `${FILES.adapter}: OIDC evidence must derive canonical claims from verified identity material`,
    ],
    [
      'let oidc_name = format!("oidc:{subject_name}")',
      `${FILES.adapter}: OIDC principal identity must bind the verified subject name`,
    ],
    [
      "[resolved_tenant_text.as_bytes(), oidc_name.as_bytes()]",
      `${FILES.adapter}: OIDC principal identity must bind the resolved tenant mapping`,
    ],
    [
      ".map(runtime_gateway_control_plane_tenant_id_from_text)",
      `${FILES.adapter}: present OIDC tenant claims must use the canonical text-to-tenant mapping`,
    ],
    [
      '"prodex:gateway-admin-control-plane-tenant-text:v1"',
      `${FILES.adapter}: non-UUID tenant text must retain a stable independent namespace`,
    ],
    [
      "VerifiedOidcRoleEvidence::Claim(ExplicitRoleMapper::new",
      `${FILES.adapter}: present OIDC roles must carry an explicit canonical claim mapping`,
    ],
    [
      "VerifiedOidcRoleEvidence::TrustedMissingClaimFallback",
      `${FILES.adapter}: missing OIDC role fallback must remain explicit and typed`,
    ],
  ]) requireText(errors, sources.adapter, needle, message);
  for (const source of [sources.adapter, sources.pipeline]) {
    for (const forbidden of [
      "plan_application_request_authentication_from_compatibility",
      "plan_application_data_plane_authorization_from_compatibility",
      "plan_application_control_plane_authorization_from_compatibility",
      "CompatibilityAuthenticationRequest",
    ]) {
      forbidText(
        errors,
        source,
        forbidden,
        `${FILES.adapter}: production must not restore compatibility authentication authority`,
      );
    }
  }
  forbidText(
    errors,
    sources.adapter,
    "principal.credential_scope !=",
    `${FILES.adapter}: route credential scope must remain owned by application authentication`,
  );
  for (const forbidden of [
    "token.canonical_claims(principal.id",
    "principal_id: principal.id",
    'expect("admin principal has tenant")',
  ]) {
    forbidText(
      errors,
      sources.adapter,
      forbidden,
      `${FILES.adapter}: OIDC claim identity and tenant must not be copied or assumed from the resolved principal`,
    );
  }

  for (const [needle, message] of [
    [
      "decode_header(token)",
      `${FILES.oidcAdapter}: OIDC evidence must retain the verified JWT header`,
    ],
    [
      "decode::<BTreeMap<String, serde_json::Value>>",
      `${FILES.oidcAdapter}: OIDC evidence must come from verified JWT claims`,
    ],
    [
      ".gateway_oidc_jwks_snapshot",
      `${FILES.oidcAdapter}: OIDC request authentication must use the immutable JWKS snapshot`,
    ],
    [
      "snapshot.domain_snapshot_at(now, now_unix_ms)",
      `${FILES.oidcAdapter}: OIDC evidence must adapt the immutable JWKS snapshot into the canonical model`,
    ],
    [
      '.get("iss")',
      `${FILES.oidcAdapter}: canonical OIDC claims must use the verified issuer claim`,
    ],
    [
      "runtime_gateway_require_oidc_audience(&claims, &config.audience)",
      `${FILES.oidcAdapter}: canonical OIDC claims must bind the verified audience`,
    ],
    [
      'runtime_gateway_oidc_numeric_date_ms(&claims, "exp")',
      `${FILES.oidcAdapter}: canonical OIDC claims must retain the verified expiry`,
    ],
    [
      "JwtAlgorithm::Es384",
      `${FILES.oidcAdapter}: canonical OIDC algorithm mapping must cover the production verifier allowlist`,
    ],
    [
      "principal_id: PrincipalId",
      `${FILES.oidcAdapter}: canonical OIDC identity must be supplied independently of the resolved principal`,
    ],
  ]) requireText(errors, sources.oidcAdapter, needle, message);
  for (const forbidden of [
    "runtime_gateway_oidc_send(",
    "runtime_gateway_oidc_fetch_json(",
    "runtime_gateway_prefetch_oidc_cache(",
  ]) {
    forbidText(
      errors,
      sources.oidcAdapter,
      forbidden,
      `${FILES.oidcAdapter}: OIDC authentication must not perform request-path network refresh`,
    );
  }

  for (const [needle, message] of [
    [
      "plan_application_data_plane_execution(",
      `${FILES.dataPlaneAdapter}: local admission must enter application execution planning`,
    ],
    [
      "plan_application_data_plane(",
      `${FILES.dataPlaneAdapter}: tenant-bound admission must enter the application data-plane use case`,
    ],
    [
      "plan_application_usage_reconciliation(",
      `${FILES.dataPlaneAdapter}: durable settlement must enter application reconciliation`,
    ],
    [
      "RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous",
      `${FILES.dataPlaneAdapter}: intentionally open anonymous compatibility must remain explicit`,
    ],
    [
      "ProviderRetryStage::BeforeFirstByte",
      `${FILES.dataPlaneAdapter}: application retry authority must own precommit provider attempts`,
    ],
    [
      "ProviderRetryStage::AfterFirstByte | ProviderRetryStage::AfterCancellation",
      `${FILES.dataPlaneAdapter}: application retry authority must deny irreversible stages`,
    ],
  ]) requireText(errors, sources.dataPlaneAdapter, needle, message);
  for (const [needle, message] of [
    [
      "let request_id = authorized.request().request_id();",
      `${FILES.dataPlaneAdapter}: admission must reuse the canonical typed request ID`,
    ],
    [
      "authorized.request().trace_context()",
      `${FILES.dataPlaneAdapter}: admission must reuse the request context's parsed trace`,
    ],
  ]) requireText(errors, sources.dataPlaneAdapter, needle, message);
  forbidText(
    errors,
    sources.dataPlaneAdapter,
    "max_precommit_attempts: u8::MAX",
    `${FILES.dataPlaneAdapter}: synthetic unbounded precommit policy must not fake application authority`,
  );

  const providerInvocation =
    functionBody(sources.dataPlaneAdapter, "runtime_gateway_provider_invocation") ?? "";
  requireText(
    errors,
    providerInvocation,
    ".provider_credential",
    `${FILES.dataPlaneAdapter}: application provider invocation must receive the configured credential reference`,
  );
  requireText(
    errors,
    providerInvocation,
    "credential.reference()",
    `${FILES.dataPlaneAdapter}: application provider invocation must preserve the configured SecretRef`,
  );

  const providerConfig =
    functionBody(sources.providerConfig, "resolve_gateway_provider_credentials_with_resolver") ?? "";
  requireBefore(
    errors,
    providerConfig,
    "projected_provider_credential(",
    "gateway_projected_provider_options(",
    `${FILES.providerConfig}: projected provider credentials must remain references during planning`,
  );

  const providerAdapter =
    functionBody(
      sources.providerAdapter,
      "runtime_local_rewrite_with_projected_provider_secret",
    ) ?? "";
  requireOrdered(
    errors,
    providerAdapter,
    [
      ".provider()",
      ".resolve(",
      "SecretPurpose::ProviderCredential",
      "material.with_exposed_secret(",
    ],
    `${FILES.providerAdapter}: projected provider secrets must resolve and expose only inside the outgoing adapter`,
  );
  for (const forbidden of [
    "std::env",
    "env::var",
    "var_os(",
    ".to_owned()",
    ".to_string()",
    ".to_vec()",
    "String::from(",
  ]) {
    forbidText(
      errors,
      providerAdapter,
      forbidden,
      `${FILES.providerAdapter}: projected provider request path must not read the environment or clone raw secret material`,
    );
  }

  requireText(
    errors,
    sources.application,
    "classify_request_target(target)",
    `${FILES.application}: request context must derive its route from the canonical target`,
  );
  for (const [needle, message] of [
    [
      "request_id: RequestId",
      `${FILES.application}: immutable request context must carry a typed request ID`,
    ],
    [
      "deadline: ApplicationRequestDeadline",
      `${FILES.application}: immutable request context must carry a monotonic deadline`,
    ],
    [
      "trace_context: Option<TraceContext>",
      `${FILES.application}: immutable request context must own parsed trace context`,
    ],
    [
      "correlation: CorrelationContext",
      `${FILES.application}: immutable request context must own canonical correlation context`,
    ],
    [
      "metadata: ApplicationRequestMetadata",
      `${FILES.application}: immutable request context must carry bounded redacted metadata`,
    ],
    [
      "APPLICATION_REQUEST_METADATA_HEADER_LIMIT",
      `${FILES.application}: request metadata collection must remain explicitly bounded`,
    ],
    [
      "trace_context_from_headers(headers)",
      `${FILES.application}: request context must use the canonical trace parser`,
    ],
    [
      ".with_trace_id(trace.trace_id.clone())",
      `${FILES.application}: parsed trace identity must enter canonical correlation`,
    ],
    [
      ".with_tenant_id(tenant.tenant_id)",
      `${FILES.application}: authorized correlation must bind the canonical tenant`,
    ],
    [
      '.field("request_id", &"<redacted>")',
      `${FILES.application}: request-context Debug must redact typed request identity`,
    ],
    [
      '.field("deadline", &"<redacted>")',
      `${FILES.application}: request-context Debug must redact deadline internals`,
    ],
    [
      "control_plane_action: Option<ControlPlaneActionPlan>",
      `${FILES.application}: authorized context must retain the exact control-plane action plan`,
    ],
    [
      "pub fn control_plane_action(&self) -> Option<&ControlPlaneActionPlan>",
      `${FILES.application}: authorized context must expose the retained control-plane action plan`,
    ],
    [
      '&self.control_plane_action.as_ref().map(|_| "<redacted>")',
      `${FILES.application}: authorized-context Debug must redact the control-plane action plan`,
    ],
    [
      "Some(plan)",
      `${FILES.application}: control-plane authorization must retain the exact authorized plan`,
    ],
  ]) requireText(errors, sources.application, needle, message);
  requireText(
    errors,
    sources.application,
    "authenticate_verified_credential(VerifiedCredentialAuthenticationRequest",
    `${FILES.application}: application authentication must invoke typed prodex-authn evidence`,
  );
  requireText(
    errors,
    sources.application,
    "required_scope: request.required_credential_scope",
    `${FILES.application}: canonical request context must own route credential scope`,
  );
  for (const [needle, message] of [
    [
      "authorize_boundary_scope(boundary, principal)",
      `${FILES.application}: application must enforce canonical data-plane credential scope`,
    ],
    [
      "authorize_boundary_role(boundary, principal)",
      `${FILES.application}: application must enforce canonical data-plane roles`,
    ],
    [
      "decide_control_plane_action(action)",
      `${FILES.application}: application must enforce canonical control-plane authorization`,
    ],
    [
      ".tenant_context(TenantMode::SingleTenant)",
      `${FILES.application}: application authorization must resolve typed tenant context`,
    ],
  ]) requireText(errors, sources.application, needle, message);
  for (const field of [
    "pub target:",
    "pub request_id:",
    "pub deadline:",
    "pub route:",
    "pub plane:",
    "pub required_credential_scope:",
    "pub trace_context:",
    "pub correlation:",
    "pub metadata:",
  ]) {
    forbidText(
      errors,
      sources.application,
      field,
      `${FILES.application}: validated request-context fields must remain immutable outside the application boundary`,
    );
  }
  requireText(
    errors,
    sources.gatewayRoute,
    "pub fn trace_context_from_headers(",
    `${FILES.gatewayRoute}: application and HTTP planning must share one trace parser`,
  );
  requireText(
    errors,
    sources.keys,
    "let typed_request_id = authorized.request().request_id();",
    `${FILES.keys}: gateway accounting must reuse the canonical typed request ID`,
  );
  for (const lateId of [
    "let typed_request_id = RequestId::new();",
    "let request_id_typed = RequestId::new();",
  ]) {
    forbidText(
      errors,
      sources.keys,
      lateId,
      `${FILES.keys}: gateway admission must not generate a second typed request ID`,
    );
  }
  for (const needle of [
    "gateway_application_boundary_rejects_invalid_and_duplicate_trace_context_before_upstream",
    'header("traceparent", "private-invalid-traceparent")',
    "duplicate_headers.append(",
    '"invalid_trace_context"',
  ]) {
    requireText(
      errors,
      sources.applicationBoundaryTests,
      needle,
      `${FILES.applicationBoundaryTests}: production boundary must retain invalid and duplicate trace regression coverage`,
    );
  }
  requireText(
    errors,
    sources.authn,
    "principal.credential_scope != required",
    `${FILES.authn}: verified credential evidence must fail closed on scope mismatch`,
  );
  for (const [needle, message] of [
    [
      "validate_oidc_token_claims(",
      `${FILES.authn}: verified OIDC evidence must invoke canonical claim validation`,
    ],
    [
      "claims.principal_id == principal.id",
      `${FILES.authn}: verified OIDC evidence must bind the resolved principal identity`,
    ],
    [
      "claims.credential_scope == principal.credential_scope",
      `${FILES.authn}: verified OIDC evidence must bind principal credential scope`,
    ],
    [
      ".role_for_claim(Some(role_claim))",
      `${FILES.authn}: present OIDC role claims must be mapped canonically`,
    ],
    [
      "role == principal.role",
      `${FILES.authn}: mapped or trusted OIDC roles must bind the resolved principal role`,
    ],
    [
      ".tenant_id\n            .is_none_or",
      `${FILES.authn}: present OIDC tenant claims must bind the resolved tenant`,
    ],
  ]) requireText(errors, sources.authn, needle, message);
  requireText(
    errors,
    sources.modules,
    "mod local_rewrite_application_boundary;",
    `${FILES.modules}: production credential adapter module is not compiled`,
  );
  requireText(
    errors,
    sources.modules,
    "mod local_rewrite_pipeline;",
    `${FILES.modules}: production typed pipeline module is not compiled`,
  );
  requireText(
    errors,
    sources.modules,
    "mod local_rewrite_application_data_plane;",
    `${FILES.modules}: production data-plane application adapter is not compiled`,
  );
  requireText(
    errors,
    sources.modules,
    "mod local_rewrite_application_runtime;",
    `${FILES.modules}: production in-process application runtime is not compiled`,
  );
  requireText(
    errors,
    sources.modules,
    "mod local_rewrite_request;",
    `${FILES.modules}: production transport-neutral request adapter is not compiled`,
  );

  requireText(
    errors,
    sources.adminScope,
    "prodex_application::ApplicationControlPlaneGovernanceScope",
    `${FILES.adminScope}: gateway mutation scope checks must retain the application-owned governance matcher`,
  );
  requireText(
    errors,
    sources.adminExecution,
    "governance: runtime_gateway_admin_governance_scope(admin_auth)",
    `${FILES.adminExecution}: mutation execution must retain the exact application-owned governance scope`,
  );
  const keyUpdate =
    functionBody(sources.adminKeys, "runtime_gateway_admin_update_key_response") ?? "";
  const keyDelete =
    functionBody(sources.adminKeys, "runtime_gateway_admin_delete_key_response") ?? "";
  const keyCreate =
    functionBody(sources.adminKeys, "runtime_gateway_admin_create_key_response") ?? "";
  const legacyKeyPaths = keyCreate + keyUpdate + keyDelete;
  forbidText(
    errors,
    legacyKeyPaths,
    "runtime_gateway_mutate_admin_key_store(",
    `${FILES.adminKeys}: key mutation handlers must use atomic key store mutation`,
  );
  for (const source of [keyCreate, keyUpdate, keyDelete]) {
    requireText(
      errors,
      source,
      "runtime_gateway_admin_mutation_execution(",
      `${FILES.adminKeys}: key mutations must use application control-plane mutation execution`,
    );
    requireText(
      errors,
      source,
      "runtime_gateway_mutate_admin_key_store_atomic(",
      `${FILES.adminKeys}: key mutations must use atomic control-plane storage execution`,
    );
    requireText(
      errors,
      source,
      "plan_application_gateway_virtual_key_mutation(",
      `${FILES.adminKeys}: key mutations must use canonical virtual-key identity planner`,
    );
    requireText(
      errors,
      source,
      "runtime_gateway_apply_virtual_key_projection",
      `${FILES.adminKeys}: key mutations must apply canonical identity projections`,
    );
  }
  forbidText(
    errors,
    legacyKeyPaths,
    "runtime_gateway_audit_admin_key_event",
    `${FILES.adminKeys}: key mutation handlers must not call direct success audit events`,
  );
  forbidText(
    errors,
    legacyKeyPaths,
    "sha256:gateway-virtual-key-create",
    `${FILES.adminKeys}: key mutations must not use static lifecycle digests`,
  );
  forbidText(
    errors,
    legacyKeyPaths,
    "sha256:gateway-virtual-key-rotate",
    `${FILES.adminKeys}: key mutations must not use static lifecycle digests`,
  );
  const scimUpdate =
    functionBody(sources.adminScim, "runtime_gateway_admin_scim_update_user_response") ?? "";
  const scimCreate =
    functionBody(sources.adminScim, "runtime_gateway_admin_scim_create_user_response") ?? "";
  const scimDelete =
    functionBody(sources.adminScim, "runtime_gateway_admin_scim_delete_user_response") ?? "";
  const scimPlan =
    functionBody(sources.adminScim, "runtime_gateway_plan_and_apply_scim_mutation") ?? "";
  const scimExecute =
    functionBody(sources.adminScim, "runtime_gateway_execute_scim_mutation") ?? "";
  const scimMutation = scimCreate + scimUpdate + scimDelete;
  forbidText(
    errors,
    scimMutation,
    "runtime_gateway_mutate_admin_key_store(",
    `${FILES.adminScim}: SCIM mutations must use atomic key store mutation`,
  );
  for (const source of [scimCreate, scimUpdate, scimDelete]) {
    requireText(
      errors,
      source,
      "runtime_gateway_admin_mutation_execution(",
      `${FILES.adminScim}: SCIM mutations must use application control-plane mutation execution`,
    );
    requireText(
      errors,
      source,
      "runtime_gateway_execute_scim_mutation(",
      `${FILES.adminScim}: SCIM mutations must route through shared execution helper`,
    );
  }
  requireText(
    errors,
    scimExecute,
    "runtime_gateway_mutate_admin_key_store_atomic(",
    `${FILES.adminScim}: SCIM mutations must write through atomic control-plane storage`,
  );
  requireText(
    errors,
    scimPlan,
    "plan_application_gateway_scim_user_mutation(",
    `${FILES.adminScim}: SCIM mutation helper must use the canonical SCIM identity planner`,
  );
  requireText(
    errors,
    scimPlan,
    "runtime_gateway_apply_scim_user_projection(",
    `${FILES.adminScim}: SCIM mutation helper must apply canonical projections`,
  );
  requireText(
    errors,
    scimPlan,
    "now_unix_ms: authorized_action.audit_event.occurred_at_unix_ms",
    `${FILES.adminScim}: SCIM planner should stamp from the retained authorized action`,
  );
  forbidText(
    errors,
    scimMutation,
    "runtime_gateway_audit_admin_scim_user_event",
    `${FILES.adminScim}: SCIM mutations must not call direct success audit events`,
  );
  forbidText(
    errors,
    scimMutation,
    "plan_application_user_lifecycle(",
    `${FILES.adminScim}: SCIM mutations must use application-identity planner`,
  );
  forbidText(
    errors,
    scimMutation,
    "sha256:scim-user-",
    `${FILES.adminScim}: SCIM mutations must not use static lifecycle digests`,
  );
  for (const regression of [
    "gateway_file_admin_mutations_reject_foreign_records_replaced_by_second_proxy",
    "gateway_sqlite_admin_mutations_reject_foreign_records_replaced_by_second_proxy",
    "let proxy_a = start_toctou_proxy(",
    "let proxy_b = start_toctou_proxy(",
    "assert_scope_forbidden(stale_update);",
    "assert_scope_forbidden(stale_delete);",
    "assert_scope_forbidden(stale_scim_update);",
  ]) {
    requireText(
      errors,
      sources.adminToctouTests,
      regression,
      `${FILES.adminToctouTests}: file and SQLite two-proxy foreign-replacement regressions must remain compiled`,
    );
  }
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`production-boundary-guard self-test failed: ${message}`);
}

function runSelfTest() {
  const valid = {
    root: `fn handle_runtime_local_rewrite_proxy_request() {
      run_runtime_local_rewrite_pipeline(RuntimeLocalRewriteRequest::tiny(request), target, shared);
    }`,
    pipeline: `struct RuntimeLocalRewriteCanonicalRequest;
    struct RuntimeLocalRewriteAuthenticatedRequest;
    struct RuntimeLocalRewriteAdmittedRequest;
    struct RuntimeLocalRewriteCapturedRequest;
    struct RuntimeLocalRewritePreparedRequest;
    struct RuntimeLocalRewriteGovernedRequest;
    struct RuntimeLocalRewriteReservedRequest;
    struct RuntimeLocalRewriteDispatchReadyRequest;
    enum RuntimeLocalRewritePipelineExit { Rejected(Box<RuntimeLocalRewritePipelineReply>) }
    fn try_run_runtime_local_rewrite_pipeline() {
      runtime_local_rewrite_canonical_context(request, target, shared);
      runtime_local_rewrite_authenticate(canonical, shared);
      runtime_local_rewrite_bounded_admission(authenticated, shared);
      runtime_local_rewrite_dispatch_websocket(admitted, shared);
      runtime_local_rewrite_capture_body(admitted, shared);
      runtime_local_rewrite_prepare_constraints(captured, shared);
      runtime_local_rewrite_dispatch_control_plane(prepared, shared);
      runtime_local_rewrite_pre_reservation_governance(prepared, shared);
      runtime_local_rewrite_reserve_virtual_key(governed, shared);
      runtime_local_rewrite_post_reservation_governance(reserved, shared);
      runtime_local_rewrite_apply_constraints(reserved);
      runtime_local_rewrite_dispatch_provider(ready, shared);
    }
    fn runtime_local_rewrite_canonical_context() {
      runtime_proxy_next_request_id(&shared.runtime_shared);
      let typed_request_id = RequestId::new();
      let started_at = Instant::now();
      Duration::from_millis(runtime_gateway_application_http_policy(shared).request_timeout_ms);
      ApplicationRequestDeadline::at(started_at.checked_add(timeout));
      let header_request = request.header_request();
      runtime_gateway_application_request_context(
        target,
        typed_request_id,
        deadline,
        &header_request.headers,
      );
    }
    fn runtime_local_rewrite_application_context_rejection() {
      prodex_gateway_http::plan_gateway_http_error_response(&error);
    }
    fn runtime_local_rewrite_preauthorize_admin() {
      runtime_gateway_admin_auth();
      runtime_gateway_admin_preauthorization();
    }
    fn runtime_local_rewrite_authorize_data_plane() {
      runtime_local_rewrite_verified_virtual_key();
      runtime_gateway_application_data_plane_authorization();
    }
    fn runtime_local_rewrite_capture_body() {
      captured.path_and_query = state.context.target().path_and_query().to_string();
    }
    fn runtime_local_rewrite_bounded_admission() {
      runtime_gateway_application_local_admission(application, shared);
      acquire_runtime_proxy_active_request_slot_with_wait();
    }`,
    directRuntime: `async fn handle() {
      try_acquire_gateway_request_permit();
      let GatewayHandlerRequest {
        target,
        request,
      } = handler_request;
      tokio::spawn(async move {});
      tokio::task::spawn_blocking(move || {
        run_runtime_local_rewrite_pipeline(request, target, &shared);
      });
    }`,
    serve: `fn run_enterprise_serve() {
      start_policy_gateway_application_for_mode(policy_mode);
      serve_with_handler(config, move |request| async move {
        application.handle(request).await
      });
      application.shutdown_and_drain(drain_timeout);
    }`,
    governance: `fn runtime_local_rewrite_dispatch_control_plane() {
      runtime_gateway_admin_response(&request.state.context);
    }
    fn runtime_local_rewrite_reserve_virtual_key() {
      runtime_gateway_virtual_key_admission();
    }`,
    dispatch: `fn runtime_local_rewrite_dispatch_provider() {
      let provider_dispatch = runtime_gateway_application_provider_dispatch(&request.application_admission);
      send_runtime_local_rewrite_upstream_request(request, &provider_dispatch);
    }`,
    providerSender: `fn send_runtime_local_rewrite_upstream_request() {
      let provider = dispatch.provider();
      let endpoint = dispatch.endpoint();
      let stream_mode = dispatch.stream_mode();
      runtime_gateway_application_provider_retry_precommit();
      match (provider, &shared.provider) {}
    }`,
    providerErrorPolicy: "",
    providerCopilot: "runtime_gateway_application_provider_retry_precommit();",
    providerDeepSeek: "runtime_gateway_application_provider_retry_precommit();",
    providerGeminiOpenAi: "runtime_gateway_application_provider_retry_precommit();",
    providerGemini: "runtime_gateway_application_provider_retry_precommit();",
    applicationProvider: "fn retry() { request.cause; }",
    providerSpi: `fn retry() {
      ProviderRetryCause::NextModel;
      ProviderRetryCause::RotateCredential;
      ProviderRetryDecision::DeniedNotRetryable;
    }`,
    keys: `fn runtime_gateway_application_virtual_key_admission() {
      plan_application_virtual_key_admission();
    }
    fn runtime_gateway_virtual_key_admission() {
      let typed_request_id = authorized.request().request_id();
      runtime_gateway_application_virtual_key_admission();
      runtime_gateway_application_data_plane_admission();
      runtime_gateway_distributed_rate_limit_admission();
      runtime_gateway_try_durable_reservation();
      apply_gateway_virtual_key_usage_update();
    }
    fn runtime_gateway_try_durable_reservation() {
      plan_application_atomic_reservation();
      ApplicationAtomicReservationStoragePlan::Postgres(storage);
      runtime_gateway_sqlite_reserve_usage();
    }`,
    reconciliationWorker: `fn schedule_runtime_gateway_billing_ledger_reconcile() {
      runtime_gateway_application_reconciliation_execution();
      spawn_blocking(move || {
        let storage_error_observed = false;
        for attempt in reconciliation.retry.attempts() {
          runtime_gateway_reconciliation_retry_sleep(reconciliation, attempt);
          runtime_gateway_record_reconciliation_audit();
        }
        ApplicationUsageReconciliationAuditOutcome::Failure;
        "gateway_billing_ledger_reconcile_failed";
        reconciliation.exhausted(storage_error_observed);
      });
    }
    fn runtime_gateway_record_reconciliation_audit() {
      if runtime_gateway_audit_usage_reconciliation(runtime_shared, audit).is_err() {
        gateway_usage_reconciliation_audit_failed;
      }
    }`,
    ledger: `fn runtime_gateway_durable_reconcile_response() {
      runtime_gateway_application_usage_reconciliation();
      runtime_gateway_sqlite_reconcile_usage();
    }`,
    reconciliationAudit: `fn runtime_gateway_audit_usage_reconciliation() {
      append_audit_event(
        plan.component(),
        plan.action(),
        plan.outcome(),
        plan.backend(),
        plan.reason(),
      );
    }`,
    responseSpend: `fn commit_stream() {
      runtime_gateway_application_provider_stage_is_committed();
    }
    fn read() {
      self.commit_stream()?;
      self.observe_chunk(&buf[..read]);
    }
    ReservationReconciliationReason::StreamInterrupted;
    ReservationReconciliationReason::Cancelled;`,
    observability: "schedule_runtime_gateway_billing_ledger_reconcile();",
    admin: `fn runtime_gateway_admin_response(preauthorized: Option<RuntimeGatewayAdminPreauthorization<'_>>) {
      let Some(preauthorized) = preauthorized;
      let authorized_action = preauthorized.control_plane_action();
      runtime_gateway_admin_boundary_response();
      runtime_gateway_admin_create_key_response();
      runtime_gateway_admin_update_key_response();
      runtime_gateway_admin_delete_key_response();
      runtime_gateway_admin_scim_create_user_response();
    }`,
    adminExecution: `fn runtime_gateway_admin_mutation_execution() {
      let alias = matches!(
        (base.operation, operation),
        (ControlPlaneOperation::VirtualKeyUpdate, ControlPlaneOperation::VirtualKeyRotateSecret)
      );
      let action = runtime_gateway_admin_control_plane_action_for_operation(&http, admin_auth, operation)
        .filter(|action| {
          action.principal.id == base.audit_event.principal_id &&
          action.resource.tenant_id == base.tenant.tenant_id &&
          action.resource.id == base.audit_event.resource.id
        })
        .ok_or_else(|| {
          runtime_gateway_admin_execution_error(400, "control_plane_route_invalid", "control-plane route is invalid");
        })?;
      let operation = plan_application_control_plane_idempotency_from_http_digest(
        action,
        &http,
        runtime_gateway_request_body_sha256(&captured.body),
      )?.operation.ok_or_else(|| {
        runtime_gateway_admin_execution_error(500, "control_plane_idempotency_plan_invalid", "control-plane idempotency planning failed");
      })?;
      plan_application_control_plane_audit_from_http(action.clone(), &http);
      governance: runtime_gateway_admin_governance_scope(admin_auth);
    }`,
    adminAuth: `fn can_access_stored_key() {
      self.governance_scope().matches();
      self.can_access_key(&key.name);
    }`,
    adminKeys: `fn runtime_gateway_admin_create_key_response() {
      runtime_gateway_admin_mutation_execution(captured, path, admin_auth, base_action, ControlPlaneOperation::VirtualKeyCreate);
      runtime_gateway_mutate_admin_key_store_atomic(shared, atomic_write, |store| {
        let records = application_key_records(store)?;
        let plan = plan_application_gateway_virtual_key_mutation(ApplicationGatewayVirtualKeyMutationRequest {
          authorized_action: &authorized_action,
          governance: &governance,
          current_records: &records,
          mutation: runtime_gateway_virtual_key_create_mutation(new_id, &body, fingerprint),
          now_unix_ms,
        });
        runtime_gateway_apply_virtual_key_projection(store, &plan)?;
      });
    }
    fn runtime_gateway_admin_update_key_response() {
      runtime_gateway_admin_mutation_execution(captured, path, admin_auth, base_action, requested_operation);
      runtime_gateway_mutate_admin_key_store_atomic(shared, atomic_write, |store| {
        let records = application_key_records(store)?;
        let plan = plan_application_gateway_virtual_key_mutation(ApplicationGatewayVirtualKeyMutationRequest {
          authorized_action: &authorized_action,
          governance: &governance,
          current_records: &records,
          mutation: runtime_gateway_virtual_key_update_mutation(id, &body, fingerprint),
          now_unix_ms,
        });
        runtime_gateway_apply_virtual_key_projection(store, &plan)?;
      });
    }
    fn runtime_gateway_admin_delete_key_response() {
      runtime_gateway_admin_mutation_execution(captured, path, admin_auth, base_action, ControlPlaneOperation::VirtualKeyDelete);
      runtime_gateway_mutate_admin_key_store_atomic(shared, atomic_write, |store| {
        let records = application_key_records(store)?;
        let plan = plan_application_gateway_virtual_key_mutation(ApplicationGatewayVirtualKeyMutationRequest {
          authorized_action: &authorized_action,
          governance: &governance,
          current_records: &records,
          mutation: runtime_gateway_virtual_key_delete_mutation(id),
          now_unix_ms,
        });
        runtime_gateway_apply_virtual_key_projection(store, &plan)?;
      });
    }`,
    adminScim: `fn runtime_gateway_admin_scim_create_user_response() {
      runtime_gateway_admin_mutation_execution(captured, &path, admin_auth, base_action, ControlPlaneOperation::ScimUserCreate);
      runtime_gateway_execute_scim_mutation(shared, execution, mutation, &mut committed);
    }
    fn runtime_gateway_admin_scim_update_user_response() {
      runtime_gateway_admin_mutation_execution(captured, &path, admin_auth, base_action, ControlPlaneOperation::ScimUserUpdate);
      runtime_gateway_execute_scim_mutation(shared, execution, mutation, &mut committed);
    }
    fn runtime_gateway_admin_scim_delete_user_response() {
      runtime_gateway_admin_mutation_execution(captured, &path, admin_auth, base_action, ControlPlaneOperation::ScimUserDelete);
      runtime_gateway_execute_scim_mutation(shared, execution, mutation, &mut committed);
    }
    fn runtime_gateway_execute_scim_mutation() {
      runtime_gateway_mutate_admin_key_store_atomic(shared, atomic_write, |store| {
        let user = runtime_gateway_plan_and_apply_scim_mutation(store, &authorized_action, &governance, mutation)?;
      });
    }
    fn runtime_gateway_plan_and_apply_scim_mutation() {
      let plan = plan_application_gateway_scim_user_mutation(ApplicationGatewayScimUserMutationRequest {
        authorized_action: &authorized_action,
        governance: &governance,
        current_records: &current_records,
        mutation,
        now_unix_ms: authorized_action.audit_event.occurred_at_unix_ms,
      });
      runtime_gateway_apply_scim_user_projection(store, &plan)?;
    }
    fn scim_in_transaction_update_rejects_foreign_authoritative_preimage_without_write() {}`,
    adminScope:
      "pub(super) use prodex_application::ApplicationControlPlaneGovernanceScope;",
    adminToctouTests: `fn gateway_file_admin_mutations_reject_foreign_records_replaced_by_second_proxy() {}
      fn gateway_sqlite_admin_mutations_reject_foreign_records_replaced_by_second_proxy() {}
      let proxy_a = start_toctou_proxy();
      let proxy_b = start_toctou_proxy();
      assert_scope_forbidden(stale_update);
      assert_scope_forbidden(stale_delete);
      assert_scope_forbidden(stale_scim_update);`,
    adapter:
      `plan_application_request_context(target, request_id, deadline, &headers); plan_application_request_authentication_from_evidence(); plan_application_data_plane_authorization(); plan_application_control_plane_authorization(); RuntimeGatewayAdminPreauthorization; CredentialScope::DataPlane; CredentialScope::ControlPlane; VerifiedCredentialEvidence::Principal; VerifiedCredentialEvidence::Oidc; token.canonical_claims(principal_id); VerifiedOidcRoleEvidence::Claim(ExplicitRoleMapper::new([])); VerifiedOidcRoleEvidence::TrustedMissingClaimFallback;
      let oidc_name = format!("oidc:{subject_name}");
      stable(&[resolved_tenant_text.as_bytes(), oidc_name.as_bytes()]);
      claimed_tenant_id.map(runtime_gateway_control_plane_tenant_id_from_text);
      "prodex:gateway-admin-control-plane-tenant-text:v1";`,
    dataPlaneAdapter:
      `struct RuntimeGatewayApplicationAdmission(RuntimeGatewayApplicationAdmissionKind);
      enum RuntimeGatewayApplicationAdmissionKind { TenantBound, CompatibilityAnonymous(RuntimeGatewayCompatibilityProviderInvocation) }
      plan_application_data_plane_execution(); plan_application_data_plane(); plan_application_usage_reconciliation(); plan_application_usage_reconciliation_execution(); RuntimeGatewayApplicationAdmissionKind::CompatibilityAnonymous; ProviderRetryStage::BeforeFirstByte; ProviderRetryStage::AfterFirstByte | ProviderRetryStage::AfterCancellation;
      ApplicationUsageReconciliationBackend::File;
      ApplicationUsageReconciliationBackend::Sqlite;
      ApplicationUsageReconciliationBackend::Postgres;
      ApplicationUsageReconciliationBackend::Redis;
      let request_id = authorized.request().request_id();
      authorized.request().trace_context();
      fn runtime_gateway_application_provider_dispatch() {
        match admission { RuntimeGatewayApplicationAdmissionKind::TenantBound(plan) => &plan.admission.provider_invocation }
      }
      fn runtime_gateway_application_provider_retry_precommit() {
        plan_application_provider_retry(ApplicationProviderRetryRequest {});
      }
      fn runtime_gateway_provider_invocation() {
        shared.provider_credential.as_ref().map(|credential| credential.reference());
      }`,
    providerConfig: `fn resolve_gateway_provider_credentials_with_resolver() {
      resolver.projected_provider_credential();
      gateway_projected_provider_options();
    }`,
    providerAdapter: `fn runtime_local_rewrite_with_projected_provider_secret() {
      credential.provider().resolve(SecretPurpose::ProviderCredential);
      material.with_exposed_secret(|bytes| expose(bytes));
    }`,
    application:
      `struct ApplicationRequestContext {
        target: CanonicalRequestTarget,
        request_id: RequestId,
        deadline: ApplicationRequestDeadline,
        route: GatewayHttpRouteKind,
        plane: GatewayHttpRoutePlane,
        required_credential_scope: CredentialScope,
        trace_context: Option<TraceContext>,
        correlation: CorrelationContext,
        metadata: ApplicationRequestMetadata,
      }
      struct ApplicationAuthorizedRequestContext {
        control_plane_action: Option<ControlPlaneActionPlan>,
      }
      impl ApplicationAuthorizedRequestContext {
        pub fn control_plane_action(&self) -> Option<&ControlPlaneActionPlan> {
          self.control_plane_action.as_ref()
        }
      }
      APPLICATION_REQUEST_METADATA_HEADER_LIMIT;
      classify_request_target(target);
      trace_context_from_headers(headers);
      CorrelationContext::new(request_id).with_trace_id(trace.trace_id.clone());
      correlation.with_tenant_id(tenant.tenant_id);
      debug.field("request_id", &"<redacted>");
      debug.field("deadline", &"<redacted>");
      debug.field("control_plane_action", &self.control_plane_action.as_ref().map(|_| "<redacted>"));
      authenticate_verified_credential(VerifiedCredentialAuthenticationRequest { required_scope: request.required_credential_scope });
      authorize_boundary_scope(boundary, principal);
      authorize_boundary_role(boundary, principal);
      decide_control_plane_action(action);
      Some(plan);
      principal.tenant_context(TenantMode::SingleTenant);`,
    gatewayRoute: "pub fn trace_context_from_headers() {}",
    applicationBoundaryTests:
      `fn gateway_application_boundary_rejects_invalid_and_duplicate_trace_context_before_upstream() {
        client.header("traceparent", "private-invalid-traceparent");
        duplicate_headers.append(value);
        "invalid_trace_context";
      }`,
    authn: `if principal.credential_scope != required {}
      validate_oidc_token_claims();
      claims.principal_id == principal.id;
      claims.credential_scope == principal.credential_scope;
      mapper.role_for_claim(Some(role_claim));
      role == principal.role;
      claims
            .tenant_id
            .is_none_or();`,
    oidcAdapter: `decode_header(token);
      decode::<BTreeMap<String, serde_json::Value>>();
      shared.gateway_oidc_jwks_snapshot.load_full();
      snapshot.domain_snapshot_at(now, now_unix_ms);
      claims.get("iss");
      runtime_gateway_require_oidc_audience(&claims, &config.audience);
      runtime_gateway_oidc_numeric_date_ms(&claims, "exp");
      JwtAlgorithm::Es384;
      fn canonical_claims(principal_id: PrincipalId) {}`,
    modules:
      "mod local_rewrite_application_boundary; mod local_rewrite_application_data_plane; mod local_rewrite_application_runtime; mod local_rewrite_pipeline; mod local_rewrite_request;",
  };
  const validErrors = validateProductionBoundary(valid);
  assertSelfTest(
    validErrors.length === 0,
    `valid wiring rejected: ${validErrors.join("; ")}`,
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      dataPlaneAdapter: valid.dataPlaneAdapter.replace("credential.reference()", "credential"),
    }).some((error) => error.includes("preserve the configured SecretRef")),
    "synthetic provider credential reference accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      providerAdapter: valid.providerAdapter.replace(
        "material.with_exposed_secret(|bytes| expose(bytes));",
        "let copied = material.to_vec();",
      ),
    }).some((error) => error.includes("resolve and expose only inside")),
    "projected provider material exposed outside the scoped adapter accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      providerAdapter: valid.providerAdapter.replace(
        "credential.provider()",
        "std::env::var(\"PROVIDER_KEY\"); credential.provider()",
      ),
    }).some((error) => error.includes("must not read the environment")),
    "request-path environment credential read accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      pipeline: valid.pipeline.replace("runtime_gateway_application_data_plane_authorization();", ""),
    }).some((error) => error.includes("data credentials")),
    "data-plane authorization bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      pipeline: valid.pipeline.replace("deadline,\n        &header_request.headers", "fresh_deadline(),\n        &header_request.headers"),
    }).some((error) => error.includes("exact target, typed request ID, deadline")),
    "request-context deadline replacement accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      pipeline: valid.pipeline.replace("&header_request.headers,", "&[],"),
    }).some((error) => error.includes("original headers")),
    "request-context original-header bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      pipeline: valid.pipeline.replace("let started_at = Instant::now();", "let started_at = SystemTime::now();"),
    }).some((error) => error.includes("monotonic clock")),
    "wall-clock request deadline accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      keys: `${valid.keys}\nlet request_id_typed = RequestId::new();`,
    }).some((error) => error.includes("second typed request ID")),
    "late gateway typed request ID accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      pipeline: valid.pipeline.replace("runtime_gateway_admin_preauthorization();", ""),
    }).some((error) => error.includes("application preauthorization")),
    "control-plane preauthorization bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      pipeline: valid.pipeline.replace(
        "runtime_local_rewrite_bounded_admission(authenticated, shared);\n      runtime_local_rewrite_dispatch_websocket(admitted, shared);",
        "runtime_local_rewrite_dispatch_websocket(admitted, shared);\n      runtime_local_rewrite_bounded_admission(authenticated, shared);",
      ),
    }).some((error) => error.includes("stages must remain ordered")),
    "pipeline stage reorder accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      pipeline: valid.pipeline.replace("struct RuntimeLocalRewriteReservedRequest;", ""),
    }).some((error) => error.includes("typed input/output/rejection")),
    "untyped pipeline stage accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      application: valid.application.replace("authenticate_verified_credential", "shadow_authenticate"),
    }).some((error) => error.includes("typed prodex-authn evidence")),
    "shadow application authentication accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      application: valid.application.replace(
        "control_plane_action: Option<ControlPlaneActionPlan>,",
        "",
      ),
    }).some((error) => error.includes("retain the exact control-plane action plan")),
    "authorized context without the exact control-plane action plan accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adapter: `${valid.adapter} plan_application_request_authentication_from_compatibility();`,
    }).some((error) => error.includes("compatibility authentication authority")),
    "compatibility production authentication accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      oidcAdapter: valid.oidcAdapter.replace(
        "snapshot.domain_snapshot_at(now, now_unix_ms);",
        "synthetic_snapshot;",
      ),
    }).some((error) => error.includes("immutable JWKS snapshot into the canonical model")),
    "synthetic OIDC JWKS evidence accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      oidcAdapter: `${valid.oidcAdapter} runtime_gateway_oidc_send();`,
    }).some((error) => error.includes("request-path network refresh")),
    "OIDC request-path network refresh accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      authn: valid.authn.replace("claims.principal_id == principal.id;", "true;"),
    }).some((error) => error.includes("bind the resolved principal identity")),
    "unbound resolved OIDC principal accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adapter: valid.adapter.replace(
        'let oidc_name = format!("oidc:{subject_name}");',
        "let oidc_name = principal.name;",
      ),
    }).some((error) => error.includes("verified subject name")),
    "OIDC principal identity detached from verified subject accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      authn: valid.authn.replace("mapper.role_for_claim(Some(role_claim));", "trusted_role;"),
    }).some((error) => error.includes("role claims must be mapped canonically")),
    "unmapped OIDC role claim accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      application: `${valid.application} struct Context { pub target: String }`,
    }).some((error) => error.includes("must remain immutable")),
    "externally mutable validated context accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      admin: `${valid.admin} fn runtime_gateway_admin_write_authorized() {}`,
    }).some((error) => error.includes("duplicate legacy role policy")),
    "duplicate legacy role policy accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      admin: valid.admin.replace("runtime_gateway_admin_create_key_response();", ""),
    }).some((error) => error.includes("admin key create must route through production key mutation handlers")),
    "admin SCIM/key mutation without key routing accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      admin: valid.admin.replace("let authorized_action = preauthorized.control_plane_action();", ""),
    }).some((error) => error.includes("must consume the exact application authorization plan")),
    "admin handlers without the exact preauthorization accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adminKeys: valid.adminKeys.replace("runtime_gateway_mutate_admin_key_store_atomic(", "runtime_gateway_mutate_admin_key_store("),
    }).some((error) => error.includes("atomic key store mutation")),
    "legacy key mutation storage accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adminExecution: valid.adminExecution.replace(
        `.filter(|action| {\n          action.principal.id == base.audit_event.principal_id &&\n          action.resource.tenant_id == base.tenant.tenant_id &&\n          action.resource.id == base.audit_event.resource.id\n        })`,
        "",
      ),
    }).some((error) => error.includes("exact principal/tenant/resource identity")),
    "admin execution without request context accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adminExecution: valid.adminExecution.replace(
        "governance: runtime_gateway_admin_governance_scope(admin_auth);",
        "",
      ),
    }).some((error) => error.includes("exact application-owned governance scope")),
    "admin mutation execution without application governance scope accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adminExecution: valid.adminExecution.replace("plan_application_control_plane_audit_from_http(action.clone(), &http);", ""),
    }).some((error) => error.includes("canonical control-plane audit planning")),
    "admin mutation execution without canonical audit planning accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adminExecution: valid.adminExecution.replace("plan_application_control_plane_idempotency_from_http_digest(", "runtime_gateway_admin_idempotency_response("),
    }).some((error) => error.includes("canonical control-plane idempotency planning")),
    "admin mutation execution without canonical idempotency planning accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adminKeys: valid.adminKeys.replace(
        "let records = application_key_records(store)?;",
        "runtime_gateway_audit_admin_key_event(shared, key);\nlet records = application_key_records(store)?;",
      ),
    }).some((error) => error.includes("direct success audit events")),
    "admin key mutation with direct success audit accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adminScim: valid.adminScim.replace(
        "runtime_gateway_execute_scim_mutation(shared, execution, mutation, &mut committed);",
        "runtime_gateway_audit_admin_scim_user_event(shared);\nruntime_gateway_execute_scim_mutation(shared, execution, mutation, &mut committed);",
      ),
    }).some((error) => error.includes("direct success audit events")),
    "admin SCIM mutation with direct success audit accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adminScim: valid.adminScim.replace(
        "plan_application_gateway_scim_user_mutation(ApplicationGatewayScimUserMutationRequest {",
        "plan_application_user_lifecycle(ApplicationGatewayScimUserMutationRequest {",
      ),
    }).some((error) => error.includes("SCIM mutation helper must use the canonical SCIM identity planner")),
    "SCIM mutation without application planner accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adminScim: valid.adminScim.replace(
        "runtime_gateway_mutate_admin_key_store_atomic(shared, atomic_write, |store| {",
        "runtime_gateway_mutate_admin_key_store(shared, |store| {",
      ),
    }).some((error) => error.includes("SCIM mutations must write through atomic")),
    "SCIM mutation with legacy key storage accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      adminScim: valid.adminScim
        .split("runtime_gateway_execute_scim_mutation(shared, execution, mutation, &mut committed);")
        .join("let _ = execution;"),
    }).some((error) => error.includes("SCIM mutations must route through shared execution helper")),
    "SCIM mutation without shared execution path accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      admin: `${valid.admin} fn legacy() { let admin_write = (path == keys_path); }`,
    }).some((error) => error.includes("path/method mutation policy")),
    "legacy admin mutation classifier accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      dispatch: valid.dispatch.replace(
        "runtime_gateway_application_provider_dispatch(&request.application_admission)",
        "provider_bypass",
      ),
    }).some((error) => error.includes("concrete sender")),
    "provider dispatch application bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      providerSender: valid.providerSender.replace("let endpoint = dispatch.endpoint();", ""),
    }).some((error) => error.includes("application provider, endpoint, and stream plan")),
    "provider sender endpoint re-derivation accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      providerCopilot: `${valid.providerCopilot}\nfn runtime_copilot_should_rotate_after_response() {}`,
    }).some((error) => error.includes("duplicate provider retry policy")),
    "duplicate Copilot retry policy accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      keys: valid.keys.replace("runtime_gateway_application_data_plane_admission();", ""),
    }).some((error) => error.includes("data-plane admission")),
    "durable reservation application admission bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      keys: valid.keys.replace("plan_application_virtual_key_admission();", ""),
    }).some((error) => error.includes("canonical application planner")),
    "virtual-key application policy bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      keys: valid.keys.replace(
        "runtime_gateway_application_virtual_key_admission();\n      runtime_gateway_application_data_plane_admission();",
        "runtime_gateway_application_data_plane_admission();\n      runtime_gateway_application_virtual_key_admission();",
      ),
    }).some((error) => error.includes("local usage writes")),
    "virtual-key policy planning after application admission accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      ledger: valid.ledger.replace("runtime_gateway_application_usage_reconciliation();", ""),
    }).some((error) => error.includes("usage reconciliation")),
    "durable reconciliation application bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      reconciliationWorker: valid.reconciliationWorker.replace(
        "runtime_gateway_application_reconciliation_execution();",
        "",
      ),
    }).some((error) => error.includes("before retry and audit side effects")),
    "compatibility reconciliation policy bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      reconciliationWorker: valid.reconciliationWorker.replace(
        "reconciliation.exhausted(storage_error_observed);",
        "",
      ),
    }).some((error) => error.includes("failure classification")),
    "silent reconciliation retry exhaustion accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      reconciliationWorker: valid.reconciliationWorker.replace(
        "ApplicationUsageReconciliationAuditOutcome::Failure;",
        "",
      ),
    }).some((error) => error.includes("failure audit and log side effects")),
    "silent pending reconciliation failure accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      reconciliationWorker: valid.reconciliationWorker.replace(
        "runtime_gateway_audit_usage_reconciliation(runtime_shared, audit).is_err()",
        "false",
      ),
    }).some((error) => error.includes("write failures must be handled")),
    "reconciliation audit write failure bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      responseSpend: valid.responseSpend.replace(
        "runtime_gateway_application_provider_stage_is_committed();",
        "",
      ),
    }).some((error) => error.includes("first-byte stream commit")),
    "stream commit application bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      observability: `${valid.observability} schedule_runtime_gateway_durable_reconcile();`,
    }).some((error) => error.includes("duplicate durable settlement")),
    "duplicate durable settlement scheduling accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  const entries = await Promise.all(
    Object.entries(FILES).map(async ([key, file]) => [
      key,
      await fs.readFile(path.join(repoRoot, file), "utf8"),
    ]),
  );
  const errors = validateProductionBoundary(Object.fromEntries(entries));
  for (const error of errors) process.stderr.write(`${error}\n`);
  if (errors.length > 0) process.exitCode = 1;
}

main().catch((error) => {
  process.stderr.write(`production-boundary-guard: ${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
