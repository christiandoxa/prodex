#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const FILES = Object.freeze({
  root: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite.rs",
  admin: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_router.rs",
  adapter:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_boundary.rs",
  dataPlaneAdapter:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_data_plane.rs",
  application: "crates/prodex-application/src/request_context.rs",
  authn: "crates/prodex-authn/src/compatibility.rs",
  pipeline:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline.rs",
  governance:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline_governance.rs",
  dispatch:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline_dispatch.rs",
  keys: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_keys.rs",
  ledger: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_ledger.rs",
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

export function validateProductionBoundary(sources) {
  const errors = [];
  const root = functionBody(sources.root, "handle_runtime_local_rewrite_proxy_request");
  if (!root) {
    errors.push(`${FILES.root}: production gateway handler is missing`);
  } else {
    requireText(
      errors,
      root,
      "run_runtime_local_rewrite_pipeline(request, shared);",
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
        "runtime_local_rewrite_canonical_target(request)",
        "runtime_local_rewrite_canonical_context(request, &target, shared)",
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
  requireText(
    errors,
    canonical ?? "",
    "runtime_gateway_application_request_context(target)",
    `${FILES.pipeline}: canonical stage must invoke application request-context planning`,
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
    "request.state.context",
    `${FILES.governance}: admin dispatch must receive the canonical application context`,
  );
  requireText(
    errors,
    sources.governance,
    "runtime_gateway_virtual_key_admission(",
    `${FILES.governance}: typed pipeline must retain virtual-key reservation`,
  );
  const providerDispatch =
    functionBody(sources.dispatch, "runtime_local_rewrite_dispatch_provider") ?? "";
  requireBefore(
    errors,
    providerDispatch,
    "runtime_gateway_application_provider_dispatch(",
    "send_runtime_local_rewrite_upstream_request(",
    `${FILES.dispatch}: application provider validation must precede provider dispatch`,
  );

  const virtualKeyAdmission =
    functionBody(sources.keys, "runtime_gateway_virtual_key_admission") ?? "";
  requireBefore(
    errors,
    virtualKeyAdmission,
    "runtime_gateway_application_data_plane_admission(",
    "runtime_gateway_try_durable_reservation(",
    `${FILES.keys}: application data-plane admission must precede durable reservation execution`,
  );
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
      "application.tenant_context()",
      `${FILES.admin}: admin dispatch must carry the typed application authorization context`,
    );
    requireText(
      errors,
      admin,
      "action.operation.requires_idempotency()",
      `${FILES.admin}: application operation policy must classify admin mutations`,
    );
    requireOrdered(
      errors,
      admin,
      [
        "runtime_gateway_admin_control_plane_action(&admin_http, admin_auth)",
        "runtime_gateway_admin_audit_boundary_response(",
        "runtime_gateway_admin_idempotency_response(",
        "runtime_gateway_admin_create_key_response(",
      ],
      `${FILES.admin}: canonical action, audit, and idempotency planning must precede admin mutation handlers`,
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
  const adminAudit = functionBody(sources.admin, "runtime_gateway_admin_audit_boundary_response") ?? "";
  requireText(
    errors,
    adminAudit,
    "plan_application_control_plane_audit_from_http(action, http)",
    `${FILES.admin}: production admin mutations must enter application audit routing`,
  );
  const adminIdempotency = functionBody(sources.admin, "runtime_gateway_admin_idempotency_response") ?? "";
  requireText(
    errors,
    adminIdempotency,
    "plan_application_control_plane_idempotency_from_http_digest(",
    `${FILES.admin}: production admin mutations must enter application idempotency planning`,
  );
  requireText(
    errors,
    adminIdempotency,
    "ApplicationControlPlaneIdempotencyError::IdempotencyKeyRequired) => return None",
    `${FILES.admin}: compatibility adapter must preserve optional legacy idempotency after canonical planning`,
  );
  requireText(
    errors,
    adminIdempotency,
    "runtime_gateway_admin_idempotency_replay_decision(",
    `${FILES.admin}: production admin duplicate handling must enter application replay planning`,
  );
  forbidText(
    errors,
    adminIdempotency,
    "if !keys.insert(cache_key)",
    `${FILES.admin}: direct set insertion must not own duplicate idempotency policy`,
  );
  forbidText(
    errors,
    adminIdempotency,
    "idempotency_key_from_headers",
    `${FILES.admin}: gateway header parsing must not shadow application idempotency planning`,
  );
  const adminReplay = functionBody(
    sources.admin,
    "runtime_gateway_admin_idempotency_replay_decision",
  ) ?? "";
  requireText(
    errors,
    adminReplay,
    "plan_application_control_plane_idempotency_replay(operation, existing.as_ref())",
    `${FILES.admin}: application replay planner must own duplicate idempotency semantics`,
  );

  for (const [needle, message] of [
    [
      "plan_application_request_context(target)",
      `${FILES.adapter}: compatibility adapter must invoke application request-context planning`,
    ],
    [
      "plan_application_request_authentication_from_compatibility(",
      `${FILES.adapter}: compatibility adapter must invoke application authentication`,
    ],
    [
      "plan_application_data_plane_authorization_from_compatibility(",
      `${FILES.adapter}: compatibility adapter must invoke data-plane authorization`,
    ],
    [
      "plan_application_control_plane_authorization_from_compatibility(",
      `${FILES.adapter}: compatibility adapter must invoke control-plane authorization`,
    ],
    [
      "RuntimeGatewayAdminPreauthorization",
      `${FILES.adapter}: compatibility adapter must carry control-plane preauthorization`,
    ],
    [
      "CredentialScope::DataPlane",
      `${FILES.adapter}: compatibility adapter must construct typed data-plane principals`,
    ],
    [
      "CredentialScope::ControlPlane",
      `${FILES.adapter}: compatibility adapter must construct typed control-plane principals`,
    ],
  ]) requireText(errors, sources.adapter, needle, message);

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
      "RuntimeGatewayApplicationAdmission::CompatibilityAnonymous",
      `${FILES.dataPlaneAdapter}: intentionally open anonymous compatibility must remain explicit`,
    ],
    [
      "ProviderRetryStage::AfterFirstByte | ProviderRetryStage::AfterCancellation",
      `${FILES.dataPlaneAdapter}: application retry authority must be limited to irreversible stages`,
    ],
  ]) requireText(errors, sources.dataPlaneAdapter, needle, message);
  forbidText(
    errors,
    sources.dataPlaneAdapter,
    "max_precommit_attempts: u8::MAX",
    `${FILES.dataPlaneAdapter}: synthetic unbounded precommit policy must not fake application authority`,
  );

  requireText(
    errors,
    sources.application,
    "classify_request_target(target)",
    `${FILES.application}: request context must derive its route from the canonical target`,
  );
  requireText(
    errors,
    sources.application,
    "authenticate_compatibility_request(CompatibilityAuthenticationRequest",
    `${FILES.application}: application authentication must invoke prodex-authn`,
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
  for (const field of ["pub target:", "pub route:", "pub plane:", "pub required_credential_scope:"]) {
    forbidText(
      errors,
      sources.application,
      field,
      `${FILES.application}: validated request-context fields must remain immutable outside the application boundary`,
    );
  }
  requireText(
    errors,
    sources.authn,
    "principal.credential_scope != required",
    `${FILES.authn}: compatibility authentication must fail closed on scope mismatch`,
  );
  requireText(
    errors,
    sources.modules,
    "mod local_rewrite_application_boundary;",
    `${FILES.modules}: production compatibility adapter module is not compiled`,
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
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`production-boundary-guard self-test failed: ${message}`);
}

function runSelfTest() {
  const valid = {
    root: `fn handle_runtime_local_rewrite_proxy_request() {
      run_runtime_local_rewrite_pipeline(request, shared);
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
      runtime_local_rewrite_canonical_target(request);
      runtime_local_rewrite_canonical_context(request, &target, shared);
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
      runtime_gateway_application_request_context(target);
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
    governance: `fn runtime_local_rewrite_dispatch_control_plane() {
      runtime_gateway_admin_response(request.state.context);
    }
    fn runtime_local_rewrite_reserve_virtual_key() {
      runtime_gateway_virtual_key_admission();
    }`,
    dispatch: `fn runtime_local_rewrite_dispatch_provider() {
      runtime_gateway_application_provider_dispatch();
      send_runtime_local_rewrite_upstream_request();
    }`,
    keys: `fn runtime_gateway_virtual_key_admission() {
      runtime_gateway_application_data_plane_admission();
      runtime_gateway_try_durable_reservation();
    }
    fn runtime_gateway_try_durable_reservation() {
      plan_application_atomic_reservation();
      ApplicationAtomicReservationStoragePlan::Postgres(storage);
      runtime_gateway_sqlite_reserve_usage();
    }`,
    ledger: `fn runtime_gateway_durable_reconcile_response() {
      runtime_gateway_application_usage_reconciliation();
      runtime_gateway_sqlite_reconcile_usage();
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
      application.tenant_context();
      runtime_gateway_admin_boundary_response();
      runtime_gateway_admin_control_plane_action(&admin_http, admin_auth);
      action.operation.requires_idempotency();
      runtime_gateway_admin_audit_boundary_response();
      runtime_gateway_admin_idempotency_response();
      runtime_gateway_admin_create_key_response();
    }
    fn runtime_gateway_admin_audit_boundary_response() {
      plan_application_control_plane_audit_from_http(action, http);
    }
    fn runtime_gateway_admin_idempotency_response() {
      plan_application_control_plane_idempotency_from_http_digest();
      ApplicationControlPlaneIdempotencyError::IdempotencyKeyRequired) => return None;
      runtime_gateway_admin_idempotency_replay_decision(&operation, existing_fingerprint.as_deref());
    }
    fn runtime_gateway_admin_idempotency_replay_decision() {
      plan_application_control_plane_idempotency_replay(operation, existing.as_ref());
    }`,
    adapter:
      "plan_application_request_context(target); plan_application_request_authentication_from_compatibility(); plan_application_data_plane_authorization_from_compatibility(); plan_application_control_plane_authorization_from_compatibility(); RuntimeGatewayAdminPreauthorization; CredentialScope::DataPlane; CredentialScope::ControlPlane;",
    dataPlaneAdapter:
      "plan_application_data_plane_execution(); plan_application_data_plane(); plan_application_usage_reconciliation(); RuntimeGatewayApplicationAdmission::CompatibilityAnonymous; ProviderRetryStage::AfterFirstByte | ProviderRetryStage::AfterCancellation;",
    application:
      "classify_request_target(target); authenticate_compatibility_request(CompatibilityAuthenticationRequest {}); authorize_boundary_scope(boundary, principal); authorize_boundary_role(boundary, principal); decide_control_plane_action(action); principal.tenant_context(TenantMode::SingleTenant);",
    authn: "if principal.credential_scope != required {}",
    modules:
      "mod local_rewrite_application_boundary; mod local_rewrite_application_data_plane; mod local_rewrite_pipeline;",
  };
  assertSelfTest(validateProductionBoundary(valid).length === 0, "valid wiring rejected");
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
      application: valid.application.replace("authenticate_compatibility_request", "shadow_authenticate"),
    }).some((error) => error.includes("invoke prodex-authn")),
    "shadow application authentication accepted",
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
      admin: valid.admin.replace("runtime_gateway_admin_audit_boundary_response();", ""),
    }).some((error) => error.includes("audit, and idempotency planning")),
    "admin mutation audit bypass accepted",
  );
  assertSelfTest(
    validateProductionBoundary({
      ...valid,
      admin: valid.admin.replace(
        "plan_application_control_plane_idempotency_replay(operation, existing.as_ref());",
        "already_seen",
      ),
    }).some((error) => error.includes("replay planner")),
    "shadow admin duplicate policy accepted",
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
      dispatch: valid.dispatch.replace("runtime_gateway_application_provider_dispatch();", ""),
    }).some((error) => error.includes("provider validation")),
    "provider dispatch application bypass accepted",
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
      ledger: valid.ledger.replace("runtime_gateway_application_usage_reconciliation();", ""),
    }).some((error) => error.includes("usage reconciliation")),
    "durable reconciliation application bypass accepted",
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
