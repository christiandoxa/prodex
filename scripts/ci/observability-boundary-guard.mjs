#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseDependencySections } from "./boundary-guard-utils.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const OBS_MANIFEST = "crates/prodex-observability/Cargo.toml";
const OBS_SRC_DIR = "crates/prodex-observability/src";
const ALLOWED_DEPENDENCIES = new Set(["prodex_domain"]);
const ALLOWED_DEV_DEPENDENCIES = new Set([]);
const FORBIDDEN_DEPENDENCIES = new Set([
  "anyhow",
  "axum",
  "clap",
  "hyper",
  "opentelemetry",
  "opentelemetry-otlp",
  "opentelemetry_sdk",
  "prometheus",
  "reqwest",
  "rusqlite",
  "sqlx",
  "tokio",
  "tower",
  "tracing",
  "tracing-opentelemetry",
  "tracing-subscriber",
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
  { name: "otel sdk", pattern: /\b(opentelemetry|opentelemetry_sdk|opentelemetry_otlp)\s*::/u },
  { name: "metrics backend", pattern: /\bprometheus\s*::/u },
  { name: "tracing backend", pattern: /\b(tracing|tracing_subscriber|tracing_opentelemetry)\s*::/u },
  { name: "transport", pattern: /\btungstenite\s*::/u },
]);
const REQUIRED_SOURCE_SNIPPETS = Object.freeze([
  "pub enum TracePropagationCarrier",
  "pub enum TracePropagationResult",
  "pub struct TracePropagationMetricPlan",
  "pub fn plan_trace_propagation_metric(",
  "metric_name: \"prodex_trace_propagation_events_total\"",
  "\"trace_carrier\"",
  "\"trace_propagation_result\"",
  "trace_propagation_carrier_label(carrier)",
  "trace_propagation_result_label(result)",
  "pub struct StructuredLogCorrelationPlan",
  "pub fn plan_structured_log_correlation(",
  "\"request_id\"",
  "\"call_id\"",
  "\"trace_id\"",
  "\"tenant_id\"",
  "\"audit_event_id\"",
  "pub enum EnterpriseIdKind",
  "pub enum EnterpriseIdResult",
  "pub struct EnterpriseIdMetricPlan",
  "pub fn plan_enterprise_id_metric(",
  "metric_name: \"prodex_enterprise_id_events_total\"",
  "\"enterprise_id_kind\"",
  "\"enterprise_id_result\"",
  "enterprise_id_kind_label(kind)",
  "enterprise_id_result_label(result)",
  "pub struct JwksCacheAgeMetricPlan",
  "pub fn plan_jwks_cache_age_metric(",
  "metric_name: \"prodex_jwks_cache_age_ms\"",
  "TelemetryAttribute::metric_label(\"jwks_cache_state\"",
  "evaluate_jwks_refresh(snapshot, now_unix_ms)",
  "jwks_refresh_decision_label(decision)",
  "pub struct PolicySnapshotAgeMetricPlan",
  "pub fn plan_policy_snapshot_age_metric",
  "metric_name: \"prodex_policy_snapshot_age_ms\"",
  "\"policy_cache_state\"",
  "evaluate_policy_refresh(status, now_unix_ms)",
  "policy_refresh_decision_label(decision)",
  "pub enum JwksRefreshOutcome",
  "pub struct JwksRefreshOutcomeMetricPlan",
  "pub fn plan_jwks_refresh_outcome_metric(",
  "metric_name: \"prodex_jwks_refresh_total\"",
  "\"jwks_refresh_result\"",
  "jwks_refresh_outcome_label(outcome)",
  "pub enum OidcRefreshOperation",
  "pub enum OidcRefreshResult",
  "pub struct OidcRefreshMetricPlan",
  "pub fn plan_oidc_refresh_metric(",
  "metric_name: \"prodex_oidc_refresh_events_total\"",
  "\"oidc_refresh_operation\"",
  "\"oidc_refresh_result\"",
  "oidc_refresh_operation_label(operation)",
  "oidc_refresh_result_label(result)",
  "pub enum PolicyRefreshOutcome",
  "pub struct PolicyRefreshOutcomeMetricPlan",
  "pub fn plan_policy_refresh_outcome_metric(",
  "metric_name: \"prodex_policy_refresh_total\"",
  "\"policy_refresh_result\"",
  "policy_refresh_outcome_label(outcome)",
  "pub enum PolicyRollbackOperation",
  "pub enum PolicyRollbackResult",
  "pub struct PolicyRollbackMetricPlan",
  "pub fn plan_policy_rollback_metric(",
  "metric_name: \"prodex_policy_rollback_events_total\"",
  "\"policy_rollback_operation\"",
  "\"policy_rollback_result\"",
  "policy_rollback_operation_label(operation)",
  "policy_rollback_result_label(result)",
  "pub enum ConfigActivationSource",
  "pub enum ConfigActivationResult",
  "pub struct ConfigActivationMetricPlan",
  "pub fn plan_config_activation_metric(",
  "metric_name: \"prodex_config_activation_events_total\"",
  "\"config_activation_source\"",
  "\"config_activation_result\"",
  "config_activation_source_label(source)",
  "config_activation_result_label(result)",
  "pub enum ConfigPublicationDeliveryTarget",
  "pub enum ConfigPublicationDeliveryResult",
  "pub struct ConfigPublicationDeliveryMetricPlan",
  "pub fn plan_config_publication_delivery_metric(",
  "metric_name: \"prodex_config_publication_delivery_total\"",
  "\"config_publication_target\"",
  "\"config_publication_result\"",
  "config_publication_delivery_target_label(target)",
  "config_publication_delivery_result_label(result)",
  "pub enum ConfigCacheInvalidationTarget",
  "pub enum ConfigCacheInvalidationResult",
  "pub struct ConfigCacheInvalidationMetricPlan",
  "pub fn plan_config_cache_invalidation_metric(",
  "metric_name: \"prodex_config_cache_invalidation_events_total\"",
  "\"config_invalidation_target\"",
  "\"config_invalidation_result\"",
  "config_cache_invalidation_target_label(target)",
  "config_cache_invalidation_result_label(result)",
  "pub enum TelemetryDropReason",
  "pub struct DroppedTelemetryMetricPlan",
  "pub fn plan_dropped_telemetry_metric(",
  "metric_name: \"prodex_telemetry_dropped_total\"",
  "\"telemetry_drop_reason\"",
  "telemetry_drop_reason_label(reason)",
  "pub enum QueueDepthKind",
  "pub struct QueueDepthMetricPlan",
  "pub fn plan_queue_depth_metric(",
  "metric_name: \"prodex_queue_depth\"",
  "\"queue_kind\"",
  "queue_depth_kind_label(kind)",
  "pub enum ConnectionPoolKind",
  "pub struct ConnectionPoolSaturationMetricPlan",
  "pub fn plan_connection_pool_saturation_metric(",
  "metric_name: \"prodex_connection_pool_in_use\"",
  "\"pool_kind\"",
  "connection_pool_kind_label(kind)",
  "pub enum ApiRouteKind",
  "pub enum ApiStatusClass",
  "pub struct ApiRedMetricPlan",
  "pub fn plan_api_red_metric(",
  "request_count_metric_name: \"prodex_api_requests_total\"",
  "duration_metric_name: \"prodex_api_request_duration_ms\"",
  "\"api_route\"",
  "\"status_class\"",
  "api_route_kind_label(route)",
  "api_status_class_label(status_class)",
  "pub enum ApiAdmissionResult",
  "pub struct ApiAdmissionMetricPlan",
  "pub fn plan_api_admission_metric(",
  "metric_name: \"prodex_api_admission_decisions_total\"",
  "\"api_admission_route\"",
  "\"api_admission_result\"",
  "api_admission_result_label(result)",
  "pub enum ApiSchemaSurface",
  "pub enum ApiSchemaValidationResult",
  "pub struct ApiSchemaValidationMetricPlan",
  "pub fn plan_api_schema_validation_metric(",
  "metric_name: \"prodex_api_schema_validation_total\"",
  "\"api_schema_surface\"",
  "\"api_schema_result\"",
  "api_schema_surface_label(surface)",
  "api_schema_validation_result_label(result)",
  "pub enum ApiDeprecationSurface",
  "pub enum ApiDeprecationSignal",
  "pub struct ApiDeprecationMetricPlan",
  "pub fn plan_api_deprecation_metric(",
  "metric_name: \"prodex_api_deprecation_events_total\"",
  "\"api_deprecation_surface\"",
  "\"api_deprecation_signal\"",
  "api_deprecation_surface_label(surface)",
  "api_deprecation_signal_label(signal)",
  "pub enum ApiPaginationSurface",
  "pub enum ApiPaginationResult",
  "pub struct ApiPaginationMetricPlan",
  "pub fn plan_api_pagination_metric(",
  "metric_name: \"prodex_api_pagination_events_total\"",
  "\"api_pagination_surface\"",
  "\"api_pagination_result\"",
  "api_pagination_surface_label(surface)",
  "api_pagination_result_label(result)",
  "pub enum ApiPreconditionSurface",
  "pub enum ApiPreconditionResult",
  "pub struct ApiPreconditionMetricPlan",
  "pub fn plan_api_precondition_metric(",
  "metric_name: \"prodex_api_precondition_events_total\"",
  "\"api_precondition_surface\"",
  "\"api_precondition_result\"",
  "api_precondition_surface_label(surface)",
  "api_precondition_result_label(result)",
  "pub enum ApiIdempotencySurface",
  "pub enum ApiIdempotencyResult",
  "pub struct ApiIdempotencyMetricPlan",
  "pub fn plan_api_idempotency_metric(",
  "metric_name: \"prodex_api_idempotency_events_total\"",
  "\"api_idempotency_surface\"",
  "\"api_idempotency_result\"",
  "api_idempotency_surface_label(surface)",
  "api_idempotency_result_label(result)",
  "pub enum IdempotencyRecordBackend",
  "pub enum IdempotencyRecordOperation",
  "pub enum IdempotencyRecordResult",
  "pub struct IdempotencyRecordMetricPlan",
  "pub fn plan_idempotency_record_metric(",
  "metric_name: \"prodex_idempotency_record_events_total\"",
  "\"idempotency_record_backend\"",
  "\"idempotency_record_operation\"",
  "\"idempotency_record_result\"",
  "idempotency_record_backend_label(backend)",
  "idempotency_record_operation_label(operation)",
  "idempotency_record_result_label(result)",
  "pub enum ApiCompatibilitySurface",
  "pub enum ApiCompatibilityResult",
  "pub struct ApiCompatibilityMetricPlan",
  "pub fn plan_api_compatibility_metric(",
  "metric_name: \"prodex_api_compatibility_events_total\"",
  "\"api_compatibility_surface\"",
  "\"api_compatibility_result\"",
  "api_compatibility_surface_label(surface)",
  "api_compatibility_result_label(result)",
  "pub enum ApiMutationAuditSurface",
  "pub enum ApiMutationAuditResult",
  "pub struct ApiMutationAuditMetricPlan",
  "pub fn plan_api_mutation_audit_metric(",
  "metric_name: \"prodex_api_mutation_audit_events_total\"",
  "\"api_mutation_audit_surface\"",
  "\"api_mutation_audit_result\"",
  "api_mutation_audit_surface_label(surface)",
  "api_mutation_audit_result_label(result)",
  "pub enum ApiVersionSurface",
  "pub enum ApiVersionResult",
  "pub struct ApiVersionMetricPlan",
  "pub fn plan_api_version_metric(",
  "metric_name: \"prodex_api_version_negotiation_events_total\"",
  "\"api_version_surface\"",
  "\"api_version_result\"",
  "api_version_surface_label(surface)",
  "api_version_result_label(result)",
  "pub enum ApiSpecSurface",
  "pub enum ApiSpecPublicationResult",
  "pub struct ApiSpecPublicationMetricPlan",
  "pub fn plan_api_spec_publication_metric(",
  "metric_name: \"prodex_api_spec_publication_events_total\"",
  "\"api_spec_surface\"",
  "\"api_spec_publication_result\"",
  "api_spec_surface_label(surface)",
  "api_spec_publication_result_label(result)",
  "pub enum ApiErrorEnvelopeSurface",
  "pub enum ApiErrorEnvelopeResult",
  "pub struct ApiErrorEnvelopeMetricPlan",
  "pub fn plan_api_error_envelope_metric(",
  "metric_name: \"prodex_api_error_envelope_events_total\"",
  "\"api_error_envelope_surface\"",
  "\"api_error_envelope_result\"",
  "api_error_envelope_surface_label(surface)",
  "api_error_envelope_result_label(result)",
  "pub enum ApiBodyLimitSurface",
  "pub enum ApiBodyLimitResult",
  "pub struct ApiBodyLimitMetricPlan",
  "pub fn plan_api_body_limit_metric(",
  "metric_name: \"prodex_api_body_limit_events_total\"",
  "\"api_body_limit_surface\"",
  "\"api_body_limit_result\"",
  "api_body_limit_surface_label(surface)",
  "api_body_limit_result_label(result)",
  "pub enum ApiTimeoutBudgetSurface",
  "pub enum ApiTimeoutBudgetResult",
  "pub struct ApiTimeoutBudgetMetricPlan",
  "pub fn plan_api_timeout_budget_metric(",
  "metric_name: \"prodex_api_timeout_budget_events_total\"",
  "\"api_timeout_budget_surface\"",
  "\"api_timeout_budget_result\"",
  "api_timeout_budget_surface_label(surface)",
  "api_timeout_budget_result_label(result)",
  "pub enum ApiCancellationSurface",
  "pub enum ApiCancellationSource",
  "pub struct ApiCancellationMetricPlan",
  "pub fn plan_api_cancellation_metric(",
  "metric_name: \"prodex_api_cancellation_events_total\"",
  "\"api_cancellation_surface\"",
  "\"api_cancellation_source\"",
  "api_cancellation_surface_label(surface)",
  "api_cancellation_source_label(source)",
  "pub enum ApiStreamBackpressureSurface",
  "pub enum ApiStreamBackpressureState",
  "pub struct ApiStreamBackpressureMetricPlan",
  "pub fn plan_api_stream_backpressure_metric(",
  "metric_name: \"prodex_api_stream_backpressure_events_total\"",
  "\"api_stream_backpressure_surface\"",
  "\"api_stream_backpressure_state\"",
  "api_stream_backpressure_surface_label(surface)",
  "api_stream_backpressure_state_label(state)",
  "pub enum ProviderKind",
  "pub enum ProviderResultClass",
  "pub struct ProviderMetricPlan",
  "pub fn plan_provider_metric(",
  "request_count_metric_name: \"prodex_provider_requests_total\"",
  "duration_metric_name: \"prodex_provider_request_duration_ms\"",
  "\"provider\"",
  "\"provider_result\"",
  "provider_kind_label(provider)",
  "provider_result_class_label(result)",
  "pub enum ProviderCapabilityKind",
  "pub enum ProviderCapabilityResult",
  "pub struct ProviderCapabilityNegotiationMetricPlan",
  "pub fn plan_provider_capability_negotiation_metric(",
  "metric_name: \"prodex_provider_capability_negotiation_events_total\"",
  "\"provider_capability\"",
  "\"provider_capability_result\"",
  "provider_capability_kind_label(capability)",
  "provider_capability_result_label(result)",
  "pub enum ProviderRetryAttemptStage",
  "pub enum ProviderRetryOutcome",
  "pub struct ProviderRetryMetricPlan",
  "pub fn plan_provider_retry_metric(",
  "metric_name: \"prodex_provider_retry_events_total\"",
  "\"provider_retry_stage\"",
  "\"provider_retry_outcome\"",
  "provider_retry_attempt_stage_label(stage)",
  "provider_retry_outcome_label(outcome)",
  "pub enum ProviderCircuitBreakerDecision",
  "pub enum ProviderCircuitBreakerEvent",
  "pub struct ProviderCircuitBreakerMetricPlan",
  "pub fn plan_provider_circuit_breaker_metric(",
  "metric_name: \"prodex_provider_circuit_breaker_events_total\"",
  "\"provider_circuit_breaker_decision\"",
  "\"provider_circuit_breaker_event\"",
  "provider_circuit_breaker_decision_label(decision)",
  "provider_circuit_breaker_event_label(event)",
  "pub enum ProviderDegradationSignal",
  "pub enum ProviderDegradationSeverity",
  "pub struct ProviderDegradationMetricPlan",
  "pub fn plan_provider_degradation_metric(",
  "metric_name: \"prodex_provider_degradation_events_total\"",
  "\"provider_degradation_signal\"",
  "\"provider_degradation_severity\"",
  "provider_degradation_signal_label(signal)",
  "provider_degradation_severity_label(severity)",
  "pub enum StreamTransportKind",
  "pub enum StreamOutcome",
  "pub struct StreamingLifecycleMetricPlan",
  "pub fn plan_streaming_lifecycle_metric(",
  "event_count_metric_name: \"prodex_streaming_lifecycle_total\"",
  "duration_metric_name: \"prodex_streaming_lifecycle_duration_ms\"",
  "\"stream_transport\"",
  "\"stream_outcome\"",
  "stream_transport_kind_label(transport)",
  "stream_outcome_label(outcome)",
  "pub enum RoutingLaneKind",
  "pub enum RoutingDecisionOutcome",
  "pub struct RoutingDecisionMetricPlan",
  "pub fn plan_routing_decision_metric(",
  "metric_name: \"prodex_routing_decisions_total\"",
  "\"routing_lane\"",
  "\"routing_outcome\"",
  "routing_lane_kind_label(lane)",
  "routing_decision_outcome_label(outcome)",
  "pub enum AuditOperation",
  "pub enum AuditResult",
  "pub struct AuditMetricPlan",
  "pub fn plan_audit_metric(",
  "metric_name: \"prodex_audit_events_total\"",
  "\"audit_operation\"",
  "\"audit_result\"",
  "audit_operation_label(operation)",
  "audit_result_label(result)",
  "pub enum AuditQueryLifecycleOperation",
  "pub enum AuditQueryLifecycleResult",
  "pub struct AuditQueryLifecycleMetricPlan",
  "pub fn plan_audit_query_lifecycle_metric(",
  "metric_name: \"prodex_audit_query_lifecycle_events_total\"",
  "\"audit_query_operation\"",
  "\"audit_query_result\"",
  "audit_query_lifecycle_operation_label(operation)",
  "audit_query_lifecycle_result_label(result)",
  "pub enum AuditChainOperation",
  "pub enum AuditChainResult",
  "pub struct AuditChainMetricPlan",
  "pub fn plan_audit_chain_metric(",
  "metric_name: \"prodex_audit_chain_events_total\"",
  "\"audit_chain_operation\"",
  "\"audit_chain_result\"",
  "audit_chain_operation_label(operation)",
  "audit_chain_result_label(result)",
  "pub enum AuditRetentionPurgeOperation",
  "pub enum AuditRetentionPurgeResult",
  "pub struct AuditRetentionPurgeMetricPlan",
  "pub fn plan_audit_retention_purge_metric(",
  "metric_name: \"prodex_audit_retention_purge_events_total\"",
  "\"audit_retention_operation\"",
  "\"audit_retention_result\"",
  "audit_retention_purge_operation_label(operation)",
  "audit_retention_purge_result_label(result)",
  "pub enum SecurityDecisionKind",
  "pub enum SecurityDecisionResult",
  "pub struct SecurityDecisionMetricPlan",
  "pub fn plan_security_decision_metric(",
  "metric_name: \"prodex_security_decisions_total\"",
  "\"security_decision\"",
  "\"security_result\"",
  "security_decision_kind_label(decision)",
  "security_decision_result_label(result)",
  "pub enum InspectionStage",
  "pub enum InspectionCoverageClass",
  "pub enum InspectionFindingCategory",
  "pub enum InspectionMaskingAction",
  "pub enum InspectionOutcome",
  "pub struct InspectionMetricPlan",
  "pub fn plan_inspection_metric(",
  "event_metric_name: \"prodex_inspection_events_total\"",
  "duration_metric_name: \"prodex_inspection_duration_microseconds\"",
  "\"inspection_stage\"",
  "\"inspection_coverage\"",
  "\"inspection_finding_category\"",
  "\"inspection_masking_action\"",
  "\"inspection_outcome\"",
  "inspection_stage_label(stage)",
  "inspection_coverage_label(coverage)",
  "inspection_finding_category_label(finding_category)",
  "inspection_masking_action_label(masking_action)",
  "inspection_outcome_label(outcome)",
  "pub enum AuthnTokenValidationStage",
  "pub enum AuthnTokenValidationResult",
  "pub struct AuthnTokenValidationMetricPlan",
  "pub fn plan_authn_token_validation_metric(",
  "metric_name: \"prodex_authn_token_validation_events_total\"",
  "\"authn_validation_stage\"",
  "\"authn_validation_result\"",
  "authn_token_validation_stage_label(stage)",
  "authn_token_validation_result_label(result)",
  "pub enum AuthzBoundaryKind",
  "pub enum AuthzDecisionResult",
  "pub struct AuthzDecisionMetricPlan",
  "pub fn plan_authz_decision_metric(",
  "metric_name: \"prodex_authz_decisions_total\"",
  "\"authz_boundary\"",
  "\"authz_result\"",
  "authz_boundary_kind_label(boundary)",
  "authz_decision_result_label(result)",
  "pub enum CredentialScopeMismatchDirection",
  "pub enum CredentialScopeMismatchResult",
  "pub struct CredentialScopeMismatchMetricPlan",
  "pub fn plan_credential_scope_mismatch_metric(",
  "metric_name: \"prodex_credential_scope_mismatch_events_total\"",
  "\"credential_scope_direction\"",
  "\"credential_scope_result\"",
  "credential_scope_mismatch_direction_label(direction)",
  "credential_scope_mismatch_result_label(result)",
  "pub enum TenantIsolationSurface",
  "pub enum TenantIsolationResult",
  "pub struct TenantIsolationMetricPlan",
  "pub fn plan_tenant_isolation_metric(",
  "metric_name: \"prodex_tenant_isolation_events_total\"",
  "\"tenant_isolation_surface\"",
  "\"tenant_isolation_result\"",
  "tenant_isolation_surface_label(surface)",
  "tenant_isolation_result_label(result)",
  "pub enum PostgresTenantContextOperation",
  "pub enum PostgresTenantContextResult",
  "pub struct PostgresTenantContextMetricPlan",
  "pub fn plan_postgres_tenant_context_metric(",
  "metric_name: \"prodex_postgres_tenant_context_events_total\"",
  "\"postgres_tenant_context_operation\"",
  "\"postgres_tenant_context_result\"",
  "postgres_tenant_context_operation_label(operation)",
  "postgres_tenant_context_result_label(result)",
  "pub enum IdentityContextSurface",
  "pub enum IdentityContextResult",
  "pub struct IdentityContextMetricPlan",
  "pub fn plan_identity_context_metric(",
  "metric_name: \"prodex_identity_context_events_total\"",
  "\"identity_context_surface\"",
  "\"identity_context_result\"",
  "identity_context_surface_label(surface)",
  "identity_context_result_label(result)",
  "pub enum BreakGlassLifecycleOperation",
  "pub enum BreakGlassLifecycleResult",
  "pub struct BreakGlassLifecycleMetricPlan",
  "pub fn plan_break_glass_lifecycle_metric(",
  "metric_name: \"prodex_break_glass_lifecycle_events_total\"",
  "\"break_glass_operation\"",
  "\"break_glass_result\"",
  "break_glass_lifecycle_operation_label(operation)",
  "break_glass_lifecycle_result_label(result)",
  "pub enum UserLifecycleOperation",
  "pub enum UserLifecycleResult",
  "pub struct UserLifecycleMetricPlan",
  "pub fn plan_user_lifecycle_metric(",
  "metric_name: \"prodex_user_lifecycle_events_total\"",
  "\"user_lifecycle_operation\"",
  "\"user_lifecycle_result\"",
  "user_lifecycle_operation_label(operation)",
  "user_lifecycle_result_label(result)",
  "pub enum ServiceIdentityLifecycleOperation",
  "pub enum ServiceIdentityLifecycleResult",
  "pub struct ServiceIdentityLifecycleMetricPlan",
  "pub fn plan_service_identity_lifecycle_metric(",
  "metric_name: \"prodex_service_identity_lifecycle_events_total\"",
  "\"service_identity_operation\"",
  "\"service_identity_result\"",
  "service_identity_lifecycle_operation_label(operation)",
  "service_identity_lifecycle_result_label(result)",
  "pub enum RoleBindingLifecycleOperation",
  "pub enum RoleBindingLifecycleResult",
  "pub struct RoleBindingLifecycleMetricPlan",
  "pub fn plan_role_binding_lifecycle_metric(",
  "metric_name: \"prodex_role_binding_lifecycle_events_total\"",
  "\"role_binding_operation\"",
  "\"role_binding_result\"",
  "role_binding_lifecycle_operation_label(operation)",
  "role_binding_lifecycle_result_label(result)",
  "pub enum ProviderCredentialLifecycleOperation",
  "pub enum ProviderCredentialLifecycleResult",
  "pub struct ProviderCredentialLifecycleMetricPlan",
  "pub fn plan_provider_credential_lifecycle_metric(",
  "metric_name: \"prodex_provider_credential_lifecycle_events_total\"",
  "\"provider_credential_operation\"",
  "\"provider_credential_result\"",
  "provider_credential_lifecycle_operation_label(operation)",
  "provider_credential_lifecycle_result_label(result)",
  "pub enum VirtualKeyLifecycleOperation",
  "pub enum VirtualKeyLifecycleResult",
  "pub struct VirtualKeyLifecycleMetricPlan",
  "pub fn plan_virtual_key_lifecycle_metric(",
  "metric_name: \"prodex_virtual_key_lifecycle_events_total\"",
  "\"credential_lifecycle_operation\"",
  "\"credential_lifecycle_result\"",
  "virtual_key_lifecycle_operation_label(operation)",
  "virtual_key_lifecycle_result_label(result)",
  "pub enum BudgetPolicyLifecycleOperation",
  "pub enum BudgetPolicyLifecycleResult",
  "pub struct BudgetPolicyLifecycleMetricPlan",
  "pub fn plan_budget_policy_lifecycle_metric(",
  "metric_name: \"prodex_budget_policy_lifecycle_events_total\"",
  "\"budget_policy_operation\"",
  "\"budget_policy_result\"",
  "budget_policy_lifecycle_operation_label(operation)",
  "budget_policy_lifecycle_result_label(result)",
  "pub enum PolicyLifecycleOperation",
  "pub enum PolicyLifecycleResult",
  "pub struct PolicyLifecycleMetricPlan",
  "pub fn plan_policy_lifecycle_metric(",
  "metric_name: \"prodex_policy_lifecycle_events_total\"",
  "\"policy_lifecycle_operation\"",
  "\"policy_lifecycle_result\"",
  "policy_lifecycle_operation_label(operation)",
  "policy_lifecycle_result_label(result)",
  "pub enum TenantLifecycleOperation",
  "pub enum TenantLifecycleResult",
  "pub struct TenantLifecycleMetricPlan",
  "pub fn plan_tenant_lifecycle_metric(",
  "metric_name: \"prodex_tenant_lifecycle_events_total\"",
  "\"account_lifecycle_operation\"",
  "\"account_lifecycle_result\"",
  "tenant_lifecycle_operation_label(operation)",
  "tenant_lifecycle_result_label(result)",
  "pub enum ReservationRecoveryOperation",
  "pub enum ReservationRecoveryResult",
  "pub struct ReservationRecoveryMetricPlan",
  "pub fn plan_reservation_recovery_metric(",
  "metric_name: \"prodex_reservation_recovery_events_total\"",
  "\"reservation_recovery_operation\"",
  "\"reservation_recovery_result\"",
  "reservation_recovery_operation_label(operation)",
  "reservation_recovery_result_label(result)",
  "pub enum AccountingOperation",
  "pub enum AccountingResult",
  "pub struct AccountingMetricPlan",
  "pub fn plan_accounting_metric(",
  "metric_name: \"prodex_accounting_events_total\"",
  "\"accounting_operation\"",
  "\"accounting_result\"",
  "accounting_operation_label(operation)",
  "accounting_result_label(result)",
  "pub enum BillingLedgerOperation",
  "pub enum BillingLedgerResult",
  "pub struct BillingLedgerMetricPlan",
  "pub fn plan_billing_ledger_metric(",
  "metric_name: \"prodex_billing_ledger_events_total\"",
  "\"billing_ledger_operation\"",
  "\"billing_ledger_result\"",
  "billing_ledger_operation_label(operation)",
  "billing_ledger_result_label(result)",
  "pub enum BudgetRejectionReason",
  "pub struct BudgetRejectionMetricPlan",
  "pub fn plan_budget_rejection_metric(",
  "metric_name: \"prodex_budget_rejections_total\"",
  "\"budget_rejection_reason\"",
  "budget_rejection_reason_label(reason)",
  "pub enum RateLimitScope",
  "pub enum RateLimitDecision",
  "pub struct RateLimitDecisionMetricPlan",
  "pub fn plan_rate_limit_decision_metric(",
  "metric_name: \"prodex_rate_limit_decisions_total\"",
  "\"rate_limit_scope\"",
  "\"rate_limit_decision\"",
  "rate_limit_scope_label(scope)",
  "rate_limit_decision_label(decision)",
  "pub enum RedisCoordinationOperation",
  "pub enum RedisCoordinationResult",
  "pub struct RedisCoordinationMetricPlan",
  "pub fn plan_redis_coordination_metric(",
  "metric_name: \"prodex_redis_coordination_events_total\"",
  "\"redis_coordination_operation\"",
  "\"redis_coordination_result\"",
  "redis_coordination_operation_label(operation)",
  "redis_coordination_result_label(result)",
  "pub enum QuotaCorrectnessEvent",
  "pub struct QuotaCorrectnessMetricPlan",
  "pub fn plan_quota_correctness_metric(",
  "metric_name: \"prodex_quota_correctness_events_total\"",
  "\"quota_correctness_event\"",
  "quota_correctness_event_label(event)",
  "pub enum SloAlertSli",
  "pub enum SloAlertSeverity",
  "pub struct SloAlertMetricPlan",
  "pub fn plan_slo_alert_metric(",
  "metric_name: \"prodex_slo_alert_events_total\"",
  "\"slo_sli\"",
  "\"slo_severity\"",
  "slo_alert_sli_label(sli)",
  "slo_alert_severity_label(severity)",
  "pub enum ShutdownLifecycleEvent",
  "pub enum ShutdownLifecycleResult",
  "pub struct ShutdownLifecycleMetricPlan",
  "pub fn plan_shutdown_lifecycle_metric(",
  "metric_name: \"prodex_shutdown_lifecycle_total\"",
  "\"shutdown_event\"",
  "\"shutdown_result\"",
  "shutdown_lifecycle_event_label(event)",
  "shutdown_lifecycle_result_label(result)",
  "pub enum HealthProbeKind",
  "pub enum HealthProbeResult",
  "pub struct HealthProbeMetricPlan",
  "pub fn plan_health_probe_metric(",
  "metric_name: \"prodex_health_probe_results_total\"",
  "\"health_probe\"",
  "\"health_result\"",
  "health_probe_kind_label(probe)",
  "health_probe_result_label(result)",
  "pub enum SecretProviderBackend",
  "pub enum SecretProviderOperation",
  "pub enum SecretProviderResult",
  "pub struct SecretProviderMetricPlan",
  "pub fn plan_secret_provider_metric(",
  "metric_name: \"prodex_secret_provider_operations_total\"",
  "\"secret_backend\"",
  "\"secret_operation\"",
  "\"secret_result\"",
  "secret_provider_backend_label(backend)",
  "secret_provider_operation_label(operation)",
  "secret_provider_result_label(result)",
  "pub enum SecretRotationScope",
  "pub enum SecretRotationResult",
  "pub struct SecretRotationMetricPlan",
  "pub fn plan_secret_rotation_metric(",
  "metric_name: \"prodex_secret_rotation_events_total\"",
  "\"secret_scope\"",
  "\"secret_rotation_result\"",
  "secret_rotation_scope_label(scope)",
  "secret_rotation_result_label(result)",
  "pub enum BackupRestoreOperation",
  "pub enum BackupRestoreResult",
  "pub struct BackupRestoreMetricPlan",
  "pub fn plan_backup_restore_metric(",
  "metric_name: \"prodex_backup_restore_events_total\"",
  "\"backup_restore_operation\"",
  "\"backup_restore_result\"",
  "backup_restore_operation_label(operation)",
  "backup_restore_result_label(result)",
  "pub enum DeploymentRolloutOperation",
  "pub enum DeploymentRolloutResult",
  "pub struct DeploymentRolloutMetricPlan",
  "pub fn plan_deployment_rollout_metric(",
  "metric_name: \"prodex_deployment_rollout_events_total\"",
  "\"deployment_rollout_operation\"",
  "\"deployment_rollout_result\"",
  "deployment_rollout_operation_label(operation)",
  "deployment_rollout_result_label(result)",
  "pub enum LoadSoakScenarioKind",
  "pub enum LoadSoakResult",
  "pub struct LoadSoakMetricPlan",
  "pub fn plan_load_soak_metric(",
  "event_count_metric_name: \"prodex_load_soak_events_total\"",
  "duration_metric_name: \"prodex_load_soak_duration_ms\"",
  "\"load_soak_scenario\"",
  "\"load_soak_result\"",
  "load_soak_scenario_label(scenario)",
  "load_soak_result_label(result)",
  "pub enum FaultInjectionTarget",
  "pub enum FaultInjectionResult",
  "pub struct FaultInjectionMetricPlan",
  "pub fn plan_fault_injection_metric(",
  "metric_name: \"prodex_fault_injection_events_total\"",
  "\"fault_injection_target\"",
  "\"fault_injection_result\"",
  "fault_injection_target_label(target)",
  "fault_injection_result_label(result)",
  "pub enum MigrationLifecycleOperation",
  "pub enum MigrationLifecycleResult",
  "pub struct MigrationLifecycleMetricPlan",
  "pub fn plan_migration_lifecycle_metric(",
  "metric_name: \"prodex_migration_lifecycle_events_total\"",
  "\"migration_operation\"",
  "\"migration_result\"",
  "migration_lifecycle_operation_label(operation)",
  "migration_lifecycle_result_label(result)",
  "pub enum PersistenceOperation",
  "pub enum PersistenceResult",
  "pub struct PersistenceMetricPlan",
  "pub fn plan_persistence_metric(",
  "metric_name: \"prodex_persistence_operations_total\"",
  "\"persistence_operation\"",
  "\"persistence_result\"",
  "persistence_operation_label(operation)",
  "persistence_result_label(result)",
]);

function sorted(values) {
  return [...values].sort((left, right) => left.localeCompare(right));
}

export function validateObservabilityManifest(tomlText, manifestPath = OBS_MANIFEST) {
  const sections = parseDependencySections(tomlText);
  const errors = [];
  const dependencies = sections.get("dependencies") ?? new Set();
  const devDependencies = sections.get("dev-dependencies") ?? new Set();
  for (const dep of dependencies) {
    if (!ALLOWED_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dependencies] must stay boundary-only; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-observability cannot depend on forbidden telemetry/runtime/framework crate '${dep}'`);
    }
  }
  for (const dep of devDependencies) {
    if (!ALLOWED_DEV_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: [dev-dependencies] must stay empty; unexpected dependency '${dep}'`);
    }
    if (FORBIDDEN_DEPENDENCIES.has(dep)) {
      errors.push(`${manifestPath}: prodex-observability tests cannot depend on forbidden telemetry/runtime/framework crate '${dep}'`);
    }
  }
  for (const sectionName of sorted(sections.keys())) {
    if (sectionName.startsWith("target.")) {
      errors.push(`${manifestPath}: target-specific dependencies are not allowed in prodex-observability`);
    }
  }
  return errors;
}

export function validateObservabilitySource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_SOURCE_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(`${sourcePath}:${index + 1}: prodex-observability cannot use ${name} boundary '${line.trim()}'`);
      }
    }
  });
  return errors;
}

export function validateRequiredObservabilityContracts(sourceText, sourcePath = "source.rs") {
  const errors = [];
  for (const snippet of REQUIRED_SOURCE_SNIPPETS) {
    if (!sourceText.includes(snippet)) {
      errors.push(`${sourcePath}: missing required observability contract '${snippet}'`);
    }
  }
  return errors;
}

export function validateObservabilityCrateRoot(sourceText, sourcePath = "source.rs") {
  if (!sourceText.includes("#![forbid(unsafe_code)]")) {
    return [`${sourcePath}: prodex-observability crate root must forbid unsafe code`];
  }
  return [];
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
  const files = await rustFilesUnder(path.join(repoRoot, OBS_SRC_DIR));
  const errors = [];
  const crateSource = [];
  for (const file of files) {
    const source = await fs.readFile(file, "utf8");
    crateSource.push(source);
    errors.push(...validateObservabilitySource(source, path.relative(repoRoot, file)));
    if (path.relative(repoRoot, file) === "crates/prodex-observability/src/lib.rs") {
      errors.push(...validateObservabilityCrateRoot(source, path.relative(repoRoot, file)));
    }
  }
  errors.push(...validateRequiredObservabilityContracts(crateSource.join("\n"), OBS_SRC_DIR));
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
const valid = `
[package]
name = "prodex-observability"

[dependencies]
prodex_domain = { workspace = true }
`;
  assertSelfTest(validateObservabilityManifest(valid, "valid/Cargo.toml").length === 0, "valid manifest rejected");
  assertSelfTest(
    validateObservabilityManifest(`${valid}\nopentelemetry = "0.27"\n`, "invalid/Cargo.toml").some((error) => error.includes("opentelemetry")),
    "forbidden opentelemetry dependency accepted",
  );
  assertSelfTest(
    validateObservabilityManifest(`${valid}\ntracing = "0.1"\n`, "invalid-tracing/Cargo.toml").some((error) => error.includes("tracing")),
    "forbidden tracing dependency accepted",
  );
  assertSelfTest(
    validateObservabilityManifest(`${valid}\nserde_json = "1"\n`, "invalid-extra/Cargo.toml").some((error) => error.includes("serde_json")),
    "extra dependency accepted",
  );
  assertSelfTest(
    validateObservabilityManifest(`${valid}\n[target.'cfg(unix)'.dependencies]\ntokio = "1"\n`, "invalid-target/Cargo.toml").some((error) => error.includes("target-specific")),
    "target-specific dependency accepted",
  );
  assertSelfTest(
    validateObservabilitySource("use opentelemetry::trace::Tracer;", "bad.rs").some((error) => error.includes("otel sdk")),
    "otel sdk source boundary accepted",
  );
  assertSelfTest(
    validateObservabilitySource("tracing::info!(\"hello\");", "bad.rs").some((error) => error.includes("tracing backend")),
    "tracing backend source boundary accepted",
  );
  assertSelfTest(
    validateObservabilitySource("use std::fmt;\nuse prodex_domain::TraceId;", "good.rs").length === 0,
    "safe source rejected",
  );
  assertSelfTest(
    validateObservabilityCrateRoot("#![forbid(unsafe_code)]\npub struct TraceContext;", "good.rs").length === 0,
    "unsafe-forbidden crate root rejected",
  );
  assertSelfTest(
    validateObservabilityCrateRoot("pub struct TraceContext;", "bad.rs").some((error) => error.includes("forbid unsafe code")),
    "crate root without unsafe forbid accepted",
  );
  assertSelfTest(
    validateRequiredObservabilityContracts(
      `
pub struct JwksCacheAgeMetricPlan;
pub struct StructuredLogCorrelationPlan;
pub struct PolicySnapshotAgeMetricPlan;
pub enum JwksRefreshOutcome {}
pub struct JwksRefreshOutcomeMetricPlan;
pub enum OidcRefreshOperation {}
pub enum OidcRefreshResult {}
pub struct OidcRefreshMetricPlan;
pub enum PolicyRefreshOutcome {}
pub struct PolicyRefreshOutcomeMetricPlan;
pub enum PolicyRollbackOperation {}
pub enum PolicyRollbackResult {}
pub struct PolicyRollbackMetricPlan;
pub enum ConfigActivationSource {}
pub enum ConfigActivationResult {}
pub struct ConfigActivationMetricPlan;
pub enum ConfigPublicationDeliveryTarget {}
pub enum ConfigPublicationDeliveryResult {}
pub struct ConfigPublicationDeliveryMetricPlan;
pub enum ConfigCacheInvalidationTarget {}
pub enum ConfigCacheInvalidationResult {}
pub struct ConfigCacheInvalidationMetricPlan;
pub enum TelemetryDropReason {}
pub struct DroppedTelemetryMetricPlan;
pub enum QueueDepthKind {}
pub struct QueueDepthMetricPlan;
pub enum ConnectionPoolKind {}
pub struct ConnectionPoolSaturationMetricPlan;
pub enum ApiRouteKind {}
pub enum ApiStatusClass {}
pub struct ApiRedMetricPlan;
pub enum ApiAdmissionResult {}
pub struct ApiAdmissionMetricPlan;
pub enum ApiSchemaSurface {}
pub enum ApiSchemaValidationResult {}
pub struct ApiSchemaValidationMetricPlan;
pub enum ApiDeprecationSurface {}
pub enum ApiDeprecationSignal {}
pub struct ApiDeprecationMetricPlan;
pub enum ApiPaginationSurface {}
pub enum ApiPaginationResult {}
pub struct ApiPaginationMetricPlan;
pub enum ApiPreconditionSurface {}
pub enum ApiPreconditionResult {}
pub struct ApiPreconditionMetricPlan;
pub enum ApiIdempotencySurface {}
pub enum ApiIdempotencyResult {}
pub struct ApiIdempotencyMetricPlan;
pub enum IdempotencyRecordBackend {}
pub enum IdempotencyRecordOperation {}
pub enum IdempotencyRecordResult {}
pub struct IdempotencyRecordMetricPlan;
pub enum ApiCompatibilitySurface {}
pub enum ApiCompatibilityResult {}
pub struct ApiCompatibilityMetricPlan;
pub enum ApiMutationAuditSurface {}
pub enum ApiMutationAuditResult {}
pub struct ApiMutationAuditMetricPlan;
pub enum ApiVersionSurface {}
pub enum ApiVersionResult {}
pub struct ApiVersionMetricPlan;
pub enum ApiSpecSurface {}
pub enum ApiSpecPublicationResult {}
pub struct ApiSpecPublicationMetricPlan;
pub enum ApiErrorEnvelopeSurface {}
pub enum ApiErrorEnvelopeResult {}
pub struct ApiErrorEnvelopeMetricPlan;
pub enum ApiBodyLimitSurface {}
pub enum ApiBodyLimitResult {}
pub struct ApiBodyLimitMetricPlan;
pub enum ApiTimeoutBudgetSurface {}
pub enum ApiTimeoutBudgetResult {}
pub struct ApiTimeoutBudgetMetricPlan;
pub enum ApiCancellationSurface {}
pub enum ApiCancellationSource {}
pub struct ApiCancellationMetricPlan;
pub enum ProviderKind {}
pub enum ProviderResultClass {}
pub struct ProviderMetricPlan;
pub enum ProviderDegradationSignal {}
pub enum ProviderDegradationSeverity {}
pub struct ProviderDegradationMetricPlan;
pub enum StreamTransportKind {}
pub enum StreamOutcome {}
pub struct StreamingLifecycleMetricPlan;
pub enum RoutingLaneKind {}
pub enum RoutingDecisionOutcome {}
pub struct RoutingDecisionMetricPlan;
pub enum AuditOperation {}
pub enum AuditResult {}
pub struct AuditMetricPlan;
pub enum AuditQueryLifecycleOperation {}
pub enum AuditQueryLifecycleResult {}
pub struct AuditQueryLifecycleMetricPlan;
pub enum AuditChainOperation {}
pub enum AuditChainResult {}
pub struct AuditChainMetricPlan;
pub enum AuditRetentionPurgeOperation {}
pub enum AuditRetentionPurgeResult {}
pub struct AuditRetentionPurgeMetricPlan;
pub enum SecurityDecisionKind {}
pub enum SecurityDecisionResult {}
pub struct SecurityDecisionMetricPlan;
pub enum InspectionStage {}
pub enum InspectionCoverageClass {}
pub enum InspectionFindingCategory {}
pub enum InspectionMaskingAction {}
pub enum InspectionOutcome {}
pub struct InspectionMetricPlan;
pub enum AuthnTokenValidationStage {}
pub enum AuthnTokenValidationResult {}
pub struct AuthnTokenValidationMetricPlan;
pub enum AuthzBoundaryKind {}
pub enum AuthzDecisionResult {}
pub struct AuthzDecisionMetricPlan;
pub enum CredentialScopeMismatchDirection {}
pub enum CredentialScopeMismatchResult {}
pub struct CredentialScopeMismatchMetricPlan;
pub enum TenantIsolationSurface {}
pub enum TenantIsolationResult {}
pub struct TenantIsolationMetricPlan;
pub enum PostgresTenantContextOperation {}
pub enum PostgresTenantContextResult {}
pub struct PostgresTenantContextMetricPlan;
pub enum IdentityContextSurface {}
pub enum IdentityContextResult {}
pub struct IdentityContextMetricPlan;
pub enum BreakGlassLifecycleOperation {}
pub enum BreakGlassLifecycleResult {}
pub struct BreakGlassLifecycleMetricPlan;
pub enum UserLifecycleOperation {}
pub enum UserLifecycleResult {}
pub struct UserLifecycleMetricPlan;
pub enum ServiceIdentityLifecycleOperation {}
pub enum ServiceIdentityLifecycleResult {}
pub struct ServiceIdentityLifecycleMetricPlan;
pub enum RoleBindingLifecycleOperation {}
pub enum RoleBindingLifecycleResult {}
pub struct RoleBindingLifecycleMetricPlan;
pub enum ProviderCredentialLifecycleOperation {}
pub enum ProviderCredentialLifecycleResult {}
pub struct ProviderCredentialLifecycleMetricPlan;
pub enum VirtualKeyLifecycleOperation {}
pub enum VirtualKeyLifecycleResult {}
pub struct VirtualKeyLifecycleMetricPlan;
pub enum BudgetPolicyLifecycleOperation {}
pub enum BudgetPolicyLifecycleResult {}
pub struct BudgetPolicyLifecycleMetricPlan;
pub enum PolicyLifecycleOperation {}
pub enum PolicyLifecycleResult {}
pub struct PolicyLifecycleMetricPlan;
pub enum TenantLifecycleOperation {}
pub enum TenantLifecycleResult {}
pub struct TenantLifecycleMetricPlan;
pub enum ReservationRecoveryOperation {}
pub enum ReservationRecoveryResult {}
pub struct ReservationRecoveryMetricPlan;
pub enum AccountingOperation {}
pub enum AccountingResult {}
pub struct AccountingMetricPlan;
pub enum BillingLedgerOperation {}
pub enum BillingLedgerResult {}
pub struct BillingLedgerMetricPlan;
pub enum BudgetRejectionReason {}
pub struct BudgetRejectionMetricPlan;
pub enum RateLimitScope {}
pub enum RateLimitDecision {}
pub struct RateLimitDecisionMetricPlan;
pub enum RedisCoordinationOperation {}
pub enum RedisCoordinationResult {}
pub struct RedisCoordinationMetricPlan;
pub enum QuotaCorrectnessEvent {}
pub struct QuotaCorrectnessMetricPlan;
pub enum SloAlertSli {}
pub enum SloAlertSeverity {}
pub struct SloAlertMetricPlan;
pub enum ShutdownLifecycleEvent {}
pub enum ShutdownLifecycleResult {}
pub struct ShutdownLifecycleMetricPlan;
pub enum HealthProbeKind {}
pub enum HealthProbeResult {}
pub struct HealthProbeMetricPlan;
pub enum SecretProviderBackend {}
pub enum SecretProviderOperation {}
pub enum SecretProviderResult {}
pub struct SecretProviderMetricPlan;
pub enum SecretRotationScope {}
pub enum SecretRotationResult {}
pub struct SecretRotationMetricPlan;
pub enum BackupRestoreOperation {}
pub enum BackupRestoreResult {}
pub struct BackupRestoreMetricPlan;
pub enum DeploymentRolloutOperation {}
pub enum DeploymentRolloutResult {}
pub struct DeploymentRolloutMetricPlan;
pub enum LoadSoakScenarioKind {}
pub enum LoadSoakResult {}
pub struct LoadSoakMetricPlan;
pub enum FaultInjectionTarget {}
pub enum FaultInjectionResult {}
pub struct FaultInjectionMetricPlan;
pub enum MigrationLifecycleOperation {}
pub enum MigrationLifecycleResult {}
pub struct MigrationLifecycleMetricPlan;
pub enum PersistenceOperation {}
pub enum PersistenceResult {}
pub struct PersistenceMetricPlan;
pub enum TracePropagationCarrier {}
pub enum TracePropagationResult {}
pub struct TracePropagationMetricPlan;
pub enum EnterpriseIdKind {}
pub enum EnterpriseIdResult {}
pub struct EnterpriseIdMetricPlan;
pub fn plan_trace_propagation_metric() {
    let carrier = TelemetryAttribute::metric_label("trace_carrier", trace_propagation_carrier_label(carrier));
    let result = TelemetryAttribute::metric_label("trace_propagation_result", trace_propagation_result_label(result));
    metric_name: "prodex_trace_propagation_events_total";
}
fn trace_propagation_carrier_label(carrier: TracePropagationCarrier) {}
fn trace_propagation_result_label(result: TracePropagationResult) {}
pub fn plan_structured_log_correlation() {
    TelemetryAttribute::trace_only("request_id", request_id);
    TelemetryAttribute::trace_only("call_id", call_id);
    TelemetryAttribute::trace_only("trace_id", trace_id);
    TelemetryAttribute::trace_only("tenant_id", tenant_id);
    TelemetryAttribute::trace_only("audit_event_id", audit_event_id);
}
pub fn plan_enterprise_id_metric() {
    let kind = TelemetryAttribute::metric_label("enterprise_id_kind", enterprise_id_kind_label(kind));
    let result = TelemetryAttribute::metric_label("enterprise_id_result", enterprise_id_result_label(result));
    metric_name: "prodex_enterprise_id_events_total";
}
fn enterprise_id_kind_label(kind: EnterpriseIdKind) {}
fn enterprise_id_result_label(result: EnterpriseIdResult) {}
pub fn plan_jwks_cache_age_metric() {
    let decision = evaluate_jwks_refresh(snapshot, now_unix_ms);
    let state = TelemetryAttribute::metric_label("jwks_cache_state", jwks_refresh_decision_label(decision));
    metric_name: "prodex_jwks_cache_age_ms";
}
fn jwks_refresh_decision_label(decision: JwksRefreshDecision) {}
pub fn plan_policy_snapshot_age_metric() {
    let decision = evaluate_policy_refresh(status, now_unix_ms);
    let state = TelemetryAttribute::metric_label("policy_cache_state", policy_refresh_decision_label(decision));
    metric_name: "prodex_policy_snapshot_age_ms";
}
fn policy_refresh_decision_label(decision: PolicyRefreshDecision) {}
pub fn plan_jwks_refresh_outcome_metric() {
    let label = TelemetryAttribute::metric_label("jwks_refresh_result", jwks_refresh_outcome_label(outcome));
    metric_name: "prodex_jwks_refresh_total";
}
fn jwks_refresh_outcome_label(outcome: JwksRefreshOutcome) {}
pub fn plan_oidc_refresh_metric() {
    let operation = TelemetryAttribute::metric_label("oidc_refresh_operation", oidc_refresh_operation_label(operation));
    let result = TelemetryAttribute::metric_label("oidc_refresh_result", oidc_refresh_result_label(result));
    metric_name: "prodex_oidc_refresh_events_total";
}
fn oidc_refresh_operation_label(operation: OidcRefreshOperation) {}
fn oidc_refresh_result_label(result: OidcRefreshResult) {}
pub fn plan_policy_refresh_outcome_metric() {
    let label = TelemetryAttribute::metric_label("policy_refresh_result", policy_refresh_outcome_label(outcome));
    metric_name: "prodex_policy_refresh_total";
}
fn policy_refresh_outcome_label(outcome: PolicyRefreshOutcome) {}
pub fn plan_policy_rollback_metric() {
    let operation = TelemetryAttribute::metric_label("policy_rollback_operation", policy_rollback_operation_label(operation));
    let result = TelemetryAttribute::metric_label("policy_rollback_result", policy_rollback_result_label(result));
    metric_name: "prodex_policy_rollback_events_total";
}
fn policy_rollback_operation_label(operation: PolicyRollbackOperation) {}
fn policy_rollback_result_label(result: PolicyRollbackResult) {}
pub fn plan_config_activation_metric() {
    let source = TelemetryAttribute::metric_label("config_activation_source", config_activation_source_label(source));
    let result = TelemetryAttribute::metric_label("config_activation_result", config_activation_result_label(result));
    metric_name: "prodex_config_activation_events_total";
}
fn config_activation_source_label(source: ConfigActivationSource) {}
fn config_activation_result_label(result: ConfigActivationResult) {}
pub fn plan_config_publication_delivery_metric() {
    let target = TelemetryAttribute::metric_label("config_publication_target", config_publication_delivery_target_label(target));
    let result = TelemetryAttribute::metric_label("config_publication_result", config_publication_delivery_result_label(result));
    metric_name: "prodex_config_publication_delivery_total";
}
fn config_publication_delivery_target_label(target: ConfigPublicationDeliveryTarget) {}
fn config_publication_delivery_result_label(result: ConfigPublicationDeliveryResult) {}
pub fn plan_config_cache_invalidation_metric() {
    let target = TelemetryAttribute::metric_label("config_invalidation_target", config_cache_invalidation_target_label(target));
    let result = TelemetryAttribute::metric_label("config_invalidation_result", config_cache_invalidation_result_label(result));
    metric_name: "prodex_config_cache_invalidation_events_total";
}
fn config_cache_invalidation_target_label(target: ConfigCacheInvalidationTarget) {}
fn config_cache_invalidation_result_label(result: ConfigCacheInvalidationResult) {}
pub fn plan_dropped_telemetry_metric() {
    let label = TelemetryAttribute::metric_label("telemetry_drop_reason", telemetry_drop_reason_label(reason));
    metric_name: "prodex_telemetry_dropped_total";
}
fn telemetry_drop_reason_label(reason: TelemetryDropReason) {}
pub fn plan_queue_depth_metric() {
    let label = TelemetryAttribute::metric_label("queue_kind", queue_depth_kind_label(kind));
    metric_name: "prodex_queue_depth";
}
fn queue_depth_kind_label(kind: QueueDepthKind) {}
pub fn plan_connection_pool_saturation_metric() {
    let label = TelemetryAttribute::metric_label("pool_kind", connection_pool_kind_label(kind));
    metric_name: "prodex_connection_pool_in_use";
}
fn connection_pool_kind_label(kind: ConnectionPoolKind) {}
pub fn plan_api_red_metric() {
    let route = TelemetryAttribute::metric_label("api_route", api_route_kind_label(route));
    let status = TelemetryAttribute::metric_label("status_class", api_status_class_label(status_class));
    request_count_metric_name: "prodex_api_requests_total";
    duration_metric_name: "prodex_api_request_duration_ms";
}
fn api_route_kind_label(route: ApiRouteKind) {}
fn api_status_class_label(status_class: ApiStatusClass) {}
pub fn plan_api_admission_metric() {
    let route = TelemetryAttribute::metric_label("api_admission_route", api_route_kind_label(route));
    let result = TelemetryAttribute::metric_label("api_admission_result", api_admission_result_label(result));
    metric_name: "prodex_api_admission_decisions_total";
}
fn api_admission_result_label(result: ApiAdmissionResult) {}
pub fn plan_api_schema_validation_metric() {
    let surface = TelemetryAttribute::metric_label("api_schema_surface", api_schema_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_schema_result", api_schema_validation_result_label(result));
    metric_name: "prodex_api_schema_validation_total";
}
fn api_schema_surface_label(surface: ApiSchemaSurface) {}
fn api_schema_validation_result_label(result: ApiSchemaValidationResult) {}
pub fn plan_api_deprecation_metric() {
    let surface = TelemetryAttribute::metric_label("api_deprecation_surface", api_deprecation_surface_label(surface));
    let signal = TelemetryAttribute::metric_label("api_deprecation_signal", api_deprecation_signal_label(signal));
    metric_name: "prodex_api_deprecation_events_total";
}
fn api_deprecation_surface_label(surface: ApiDeprecationSurface) {}
fn api_deprecation_signal_label(signal: ApiDeprecationSignal) {}
pub fn plan_api_pagination_metric() {
    let surface = TelemetryAttribute::metric_label("api_pagination_surface", api_pagination_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_pagination_result", api_pagination_result_label(result));
    metric_name: "prodex_api_pagination_events_total";
}
fn api_pagination_surface_label(surface: ApiPaginationSurface) {}
fn api_pagination_result_label(result: ApiPaginationResult) {}
pub fn plan_api_precondition_metric() {
    let surface = TelemetryAttribute::metric_label("api_precondition_surface", api_precondition_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_precondition_result", api_precondition_result_label(result));
    metric_name: "prodex_api_precondition_events_total";
}
fn api_precondition_surface_label(surface: ApiPreconditionSurface) {}
fn api_precondition_result_label(result: ApiPreconditionResult) {}
pub fn plan_api_idempotency_metric() {
    let surface = TelemetryAttribute::metric_label("api_idempotency_surface", api_idempotency_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_idempotency_result", api_idempotency_result_label(result));
    metric_name: "prodex_api_idempotency_events_total";
}
fn api_idempotency_surface_label(surface: ApiIdempotencySurface) {}
fn api_idempotency_result_label(result: ApiIdempotencyResult) {}
pub fn plan_idempotency_record_metric() {
    let backend = TelemetryAttribute::metric_label("idempotency_record_backend", idempotency_record_backend_label(backend));
    let operation = TelemetryAttribute::metric_label("idempotency_record_operation", idempotency_record_operation_label(operation));
    let result = TelemetryAttribute::metric_label("idempotency_record_result", idempotency_record_result_label(result));
    metric_name: "prodex_idempotency_record_events_total";
}
fn idempotency_record_backend_label(backend: IdempotencyRecordBackend) {}
fn idempotency_record_operation_label(operation: IdempotencyRecordOperation) {}
fn idempotency_record_result_label(result: IdempotencyRecordResult) {}
pub fn plan_api_compatibility_metric() {
    let surface = TelemetryAttribute::metric_label("api_compatibility_surface", api_compatibility_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_compatibility_result", api_compatibility_result_label(result));
    metric_name: "prodex_api_compatibility_events_total";
}
fn api_compatibility_surface_label(surface: ApiCompatibilitySurface) {}
fn api_compatibility_result_label(result: ApiCompatibilityResult) {}
pub fn plan_api_mutation_audit_metric() {
    let surface = TelemetryAttribute::metric_label("api_mutation_audit_surface", api_mutation_audit_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_mutation_audit_result", api_mutation_audit_result_label(result));
    metric_name: "prodex_api_mutation_audit_events_total";
}
fn api_mutation_audit_surface_label(surface: ApiMutationAuditSurface) {}
fn api_mutation_audit_result_label(result: ApiMutationAuditResult) {}
pub fn plan_api_version_metric() {
    let surface = TelemetryAttribute::metric_label("api_version_surface", api_version_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_version_result", api_version_result_label(result));
    metric_name: "prodex_api_version_negotiation_events_total";
}
fn api_version_surface_label(surface: ApiVersionSurface) {}
fn api_version_result_label(result: ApiVersionResult) {}
pub fn plan_api_spec_publication_metric() {
    let surface = TelemetryAttribute::metric_label("api_spec_surface", api_spec_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_spec_publication_result", api_spec_publication_result_label(result));
    metric_name: "prodex_api_spec_publication_events_total";
}
fn api_spec_surface_label(surface: ApiSpecSurface) {}
fn api_spec_publication_result_label(result: ApiSpecPublicationResult) {}
pub fn plan_api_error_envelope_metric() {
    let surface = TelemetryAttribute::metric_label("api_error_envelope_surface", api_error_envelope_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_error_envelope_result", api_error_envelope_result_label(result));
    metric_name: "prodex_api_error_envelope_events_total";
}
fn api_error_envelope_surface_label(surface: ApiErrorEnvelopeSurface) {}
fn api_error_envelope_result_label(result: ApiErrorEnvelopeResult) {}
pub fn plan_api_body_limit_metric() {
    let surface = TelemetryAttribute::metric_label("api_body_limit_surface", api_body_limit_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_body_limit_result", api_body_limit_result_label(result));
    metric_name: "prodex_api_body_limit_events_total";
}
fn api_body_limit_surface_label(surface: ApiBodyLimitSurface) {}
fn api_body_limit_result_label(result: ApiBodyLimitResult) {}
pub fn plan_api_timeout_budget_metric() {
    let surface = TelemetryAttribute::metric_label("api_timeout_budget_surface", api_timeout_budget_surface_label(surface));
    let result = TelemetryAttribute::metric_label("api_timeout_budget_result", api_timeout_budget_result_label(result));
    metric_name: "prodex_api_timeout_budget_events_total";
}
fn api_timeout_budget_surface_label(surface: ApiTimeoutBudgetSurface) {}
fn api_timeout_budget_result_label(result: ApiTimeoutBudgetResult) {}
pub fn plan_api_cancellation_metric() {
    let surface = TelemetryAttribute::metric_label("api_cancellation_surface", api_cancellation_surface_label(surface));
    let source = TelemetryAttribute::metric_label("api_cancellation_source", api_cancellation_source_label(source));
    metric_name: "prodex_api_cancellation_events_total";
}
fn api_cancellation_surface_label(surface: ApiCancellationSurface) {}
fn api_cancellation_source_label(source: ApiCancellationSource) {}
pub enum ApiStreamBackpressureSurface {}
pub enum ApiStreamBackpressureState {}
pub struct ApiStreamBackpressureMetricPlan;
pub fn plan_api_stream_backpressure_metric() {
    let surface = TelemetryAttribute::metric_label("api_stream_backpressure_surface", api_stream_backpressure_surface_label(surface));
    let state = TelemetryAttribute::metric_label("api_stream_backpressure_state", api_stream_backpressure_state_label(state));
    metric_name: "prodex_api_stream_backpressure_events_total";
}
fn api_stream_backpressure_surface_label(surface: ApiStreamBackpressureSurface) {}
fn api_stream_backpressure_state_label(state: ApiStreamBackpressureState) {}
pub fn plan_provider_metric() {
    let provider = TelemetryAttribute::metric_label("provider", provider_kind_label(provider));
    let result = TelemetryAttribute::metric_label("provider_result", provider_result_class_label(result));
    request_count_metric_name: "prodex_provider_requests_total";
    duration_metric_name: "prodex_provider_request_duration_ms";
}
fn provider_kind_label(provider: ProviderKind) {}
fn provider_result_class_label(result: ProviderResultClass) {}
pub enum ProviderCapabilityKind {}
pub enum ProviderCapabilityResult {}
pub struct ProviderCapabilityNegotiationMetricPlan;
pub fn plan_provider_capability_negotiation_metric() {
    let provider = TelemetryAttribute::metric_label("provider", provider_kind_label(provider));
    let capability = TelemetryAttribute::metric_label("provider_capability", provider_capability_kind_label(capability));
    let result = TelemetryAttribute::metric_label("provider_capability_result", provider_capability_result_label(result));
    metric_name: "prodex_provider_capability_negotiation_events_total";
}
fn provider_capability_kind_label(capability: ProviderCapabilityKind) {}
fn provider_capability_result_label(result: ProviderCapabilityResult) {}
pub enum ProviderRetryAttemptStage {}
pub enum ProviderRetryOutcome {}
pub struct ProviderRetryMetricPlan;
pub fn plan_provider_retry_metric() {
    let provider = TelemetryAttribute::metric_label("provider", provider_kind_label(provider));
    let stage = TelemetryAttribute::metric_label("provider_retry_stage", provider_retry_attempt_stage_label(stage));
    let outcome = TelemetryAttribute::metric_label("provider_retry_outcome", provider_retry_outcome_label(outcome));
    metric_name: "prodex_provider_retry_events_total";
}
fn provider_retry_attempt_stage_label(stage: ProviderRetryAttemptStage) {}
fn provider_retry_outcome_label(outcome: ProviderRetryOutcome) {}
pub enum ProviderCircuitBreakerDecision {}
pub enum ProviderCircuitBreakerEvent {}
pub struct ProviderCircuitBreakerMetricPlan;
pub fn plan_provider_circuit_breaker_metric() {
    let provider = TelemetryAttribute::metric_label("provider", provider_kind_label(provider));
    let decision = TelemetryAttribute::metric_label("provider_circuit_breaker_decision", provider_circuit_breaker_decision_label(decision));
    let event = TelemetryAttribute::metric_label("provider_circuit_breaker_event", provider_circuit_breaker_event_label(event));
    metric_name: "prodex_provider_circuit_breaker_events_total";
}
fn provider_circuit_breaker_decision_label(decision: ProviderCircuitBreakerDecision) {}
fn provider_circuit_breaker_event_label(event: ProviderCircuitBreakerEvent) {}
pub fn plan_provider_degradation_metric() {
    let provider = TelemetryAttribute::metric_label("provider", provider_kind_label(provider));
    let signal = TelemetryAttribute::metric_label("provider_degradation_signal", provider_degradation_signal_label(signal));
    let severity = TelemetryAttribute::metric_label("provider_degradation_severity", provider_degradation_severity_label(severity));
    metric_name: "prodex_provider_degradation_events_total";
}
fn provider_degradation_signal_label(signal: ProviderDegradationSignal) {}
fn provider_degradation_severity_label(severity: ProviderDegradationSeverity) {}
pub fn plan_streaming_lifecycle_metric() {
    let transport = TelemetryAttribute::metric_label("stream_transport", stream_transport_kind_label(transport));
    let outcome = TelemetryAttribute::metric_label("stream_outcome", stream_outcome_label(outcome));
    event_count_metric_name: "prodex_streaming_lifecycle_total";
    duration_metric_name: "prodex_streaming_lifecycle_duration_ms";
}
fn stream_transport_kind_label(transport: StreamTransportKind) {}
fn stream_outcome_label(outcome: StreamOutcome) {}
pub fn plan_routing_decision_metric() {
    let lane = TelemetryAttribute::metric_label("routing_lane", routing_lane_kind_label(lane));
    let outcome = TelemetryAttribute::metric_label("routing_outcome", routing_decision_outcome_label(outcome));
    metric_name: "prodex_routing_decisions_total";
}
fn routing_lane_kind_label(lane: RoutingLaneKind) {}
fn routing_decision_outcome_label(outcome: RoutingDecisionOutcome) {}
pub fn plan_audit_metric() {
    let operation = TelemetryAttribute::metric_label("audit_operation", audit_operation_label(operation));
    let result = TelemetryAttribute::metric_label("audit_result", audit_result_label(result));
    metric_name: "prodex_audit_events_total";
}
fn audit_operation_label(operation: AuditOperation) {}
fn audit_result_label(result: AuditResult) {}
pub fn plan_audit_query_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("audit_query_operation", audit_query_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("audit_query_result", audit_query_lifecycle_result_label(result));
    metric_name: "prodex_audit_query_lifecycle_events_total";
}
fn audit_query_lifecycle_operation_label(operation: AuditQueryLifecycleOperation) {}
fn audit_query_lifecycle_result_label(result: AuditQueryLifecycleResult) {}
pub fn plan_audit_chain_metric() {
    let operation = TelemetryAttribute::metric_label("audit_chain_operation", audit_chain_operation_label(operation));
    let result = TelemetryAttribute::metric_label("audit_chain_result", audit_chain_result_label(result));
    metric_name: "prodex_audit_chain_events_total";
}
fn audit_chain_operation_label(operation: AuditChainOperation) {}
fn audit_chain_result_label(result: AuditChainResult) {}
pub fn plan_audit_retention_purge_metric() {
    let operation = TelemetryAttribute::metric_label("audit_retention_operation", audit_retention_purge_operation_label(operation));
    let result = TelemetryAttribute::metric_label("audit_retention_result", audit_retention_purge_result_label(result));
    metric_name: "prodex_audit_retention_purge_events_total";
}
fn audit_retention_purge_operation_label(operation: AuditRetentionPurgeOperation) {}
fn audit_retention_purge_result_label(result: AuditRetentionPurgeResult) {}
pub fn plan_security_decision_metric() {
    let decision = TelemetryAttribute::metric_label("security_decision", security_decision_kind_label(decision));
    let result = TelemetryAttribute::metric_label("security_result", security_decision_result_label(result));
    metric_name: "prodex_security_decisions_total";
}
fn security_decision_kind_label(decision: SecurityDecisionKind) {}
fn security_decision_result_label(result: SecurityDecisionResult) {}
pub fn plan_inspection_metric() {
    let stage = TelemetryAttribute::metric_label("inspection_stage", inspection_stage_label(stage));
    let coverage = TelemetryAttribute::metric_label("inspection_coverage", inspection_coverage_label(coverage));
    let finding = TelemetryAttribute::metric_label("inspection_finding_category", inspection_finding_category_label(finding_category));
    let masking = TelemetryAttribute::metric_label("inspection_masking_action", inspection_masking_action_label(masking_action));
    let outcome = TelemetryAttribute::metric_label("inspection_outcome", inspection_outcome_label(outcome));
    event_metric_name: "prodex_inspection_events_total";
    duration_metric_name: "prodex_inspection_duration_microseconds";
}
fn inspection_stage_label(stage: InspectionStage) {}
fn inspection_coverage_label(coverage: InspectionCoverageClass) {}
fn inspection_finding_category_label(finding_category: InspectionFindingCategory) {}
fn inspection_masking_action_label(masking_action: InspectionMaskingAction) {}
fn inspection_outcome_label(outcome: InspectionOutcome) {}
pub fn plan_authn_token_validation_metric() {
    let stage = TelemetryAttribute::metric_label("authn_validation_stage", authn_token_validation_stage_label(stage));
    let result = TelemetryAttribute::metric_label("authn_validation_result", authn_token_validation_result_label(result));
    metric_name: "prodex_authn_token_validation_events_total";
}
fn authn_token_validation_stage_label(stage: AuthnTokenValidationStage) {}
fn authn_token_validation_result_label(result: AuthnTokenValidationResult) {}
pub fn plan_authz_decision_metric() {
    let boundary = TelemetryAttribute::metric_label("authz_boundary", authz_boundary_kind_label(boundary));
    let result = TelemetryAttribute::metric_label("authz_result", authz_decision_result_label(result));
    metric_name: "prodex_authz_decisions_total";
}
fn authz_boundary_kind_label(boundary: AuthzBoundaryKind) {}
fn authz_decision_result_label(result: AuthzDecisionResult) {}
pub fn plan_credential_scope_mismatch_metric() {
    let direction = TelemetryAttribute::metric_label("credential_scope_direction", credential_scope_mismatch_direction_label(direction));
    let result = TelemetryAttribute::metric_label("credential_scope_result", credential_scope_mismatch_result_label(result));
    metric_name: "prodex_credential_scope_mismatch_events_total";
}
fn credential_scope_mismatch_direction_label(direction: CredentialScopeMismatchDirection) {}
fn credential_scope_mismatch_result_label(result: CredentialScopeMismatchResult) {}
pub fn plan_tenant_isolation_metric() {
    let surface = TelemetryAttribute::metric_label("tenant_isolation_surface", tenant_isolation_surface_label(surface));
    let result = TelemetryAttribute::metric_label("tenant_isolation_result", tenant_isolation_result_label(result));
    metric_name: "prodex_tenant_isolation_events_total";
}
fn tenant_isolation_surface_label(surface: TenantIsolationSurface) {}
fn tenant_isolation_result_label(result: TenantIsolationResult) {}
pub fn plan_postgres_tenant_context_metric() {
    let operation = TelemetryAttribute::metric_label("postgres_tenant_context_operation", postgres_tenant_context_operation_label(operation));
    let result = TelemetryAttribute::metric_label("postgres_tenant_context_result", postgres_tenant_context_result_label(result));
    metric_name: "prodex_postgres_tenant_context_events_total";
}
fn postgres_tenant_context_operation_label(operation: PostgresTenantContextOperation) {}
fn postgres_tenant_context_result_label(result: PostgresTenantContextResult) {}
pub fn plan_identity_context_metric() {
    let surface = TelemetryAttribute::metric_label("identity_context_surface", identity_context_surface_label(surface));
    let result = TelemetryAttribute::metric_label("identity_context_result", identity_context_result_label(result));
    metric_name: "prodex_identity_context_events_total";
}
fn identity_context_surface_label(surface: IdentityContextSurface) {}
fn identity_context_result_label(result: IdentityContextResult) {}
pub fn plan_break_glass_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("break_glass_operation", break_glass_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("break_glass_result", break_glass_lifecycle_result_label(result));
    metric_name: "prodex_break_glass_lifecycle_events_total";
}
fn break_glass_lifecycle_operation_label(operation: BreakGlassLifecycleOperation) {}
fn break_glass_lifecycle_result_label(result: BreakGlassLifecycleResult) {}
pub fn plan_user_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("user_lifecycle_operation", user_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("user_lifecycle_result", user_lifecycle_result_label(result));
    metric_name: "prodex_user_lifecycle_events_total";
}
fn user_lifecycle_operation_label(operation: UserLifecycleOperation) {}
fn user_lifecycle_result_label(result: UserLifecycleResult) {}
pub fn plan_service_identity_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("service_identity_operation", service_identity_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("service_identity_result", service_identity_lifecycle_result_label(result));
    metric_name: "prodex_service_identity_lifecycle_events_total";
}
fn service_identity_lifecycle_operation_label(operation: ServiceIdentityLifecycleOperation) {}
fn service_identity_lifecycle_result_label(result: ServiceIdentityLifecycleResult) {}
pub fn plan_role_binding_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("role_binding_operation", role_binding_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("role_binding_result", role_binding_lifecycle_result_label(result));
    metric_name: "prodex_role_binding_lifecycle_events_total";
}
fn role_binding_lifecycle_operation_label(operation: RoleBindingLifecycleOperation) {}
fn role_binding_lifecycle_result_label(result: RoleBindingLifecycleResult) {}
pub fn plan_provider_credential_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("provider_credential_operation", provider_credential_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("provider_credential_result", provider_credential_lifecycle_result_label(result));
    metric_name: "prodex_provider_credential_lifecycle_events_total";
}
fn provider_credential_lifecycle_operation_label(operation: ProviderCredentialLifecycleOperation) {}
fn provider_credential_lifecycle_result_label(result: ProviderCredentialLifecycleResult) {}
pub fn plan_virtual_key_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("credential_lifecycle_operation", virtual_key_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("credential_lifecycle_result", virtual_key_lifecycle_result_label(result));
    metric_name: "prodex_virtual_key_lifecycle_events_total";
}
fn virtual_key_lifecycle_operation_label(operation: VirtualKeyLifecycleOperation) {}
fn virtual_key_lifecycle_result_label(result: VirtualKeyLifecycleResult) {}
pub fn plan_budget_policy_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("budget_policy_operation", budget_policy_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("budget_policy_result", budget_policy_lifecycle_result_label(result));
    metric_name: "prodex_budget_policy_lifecycle_events_total";
}
fn budget_policy_lifecycle_operation_label(operation: BudgetPolicyLifecycleOperation) {}
fn budget_policy_lifecycle_result_label(result: BudgetPolicyLifecycleResult) {}
pub fn plan_policy_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("policy_lifecycle_operation", policy_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("policy_lifecycle_result", policy_lifecycle_result_label(result));
    metric_name: "prodex_policy_lifecycle_events_total";
}
fn policy_lifecycle_operation_label(operation: PolicyLifecycleOperation) {}
fn policy_lifecycle_result_label(result: PolicyLifecycleResult) {}
pub fn plan_tenant_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("account_lifecycle_operation", tenant_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("account_lifecycle_result", tenant_lifecycle_result_label(result));
    metric_name: "prodex_tenant_lifecycle_events_total";
}
fn tenant_lifecycle_operation_label(operation: TenantLifecycleOperation) {}
fn tenant_lifecycle_result_label(result: TenantLifecycleResult) {}
pub fn plan_reservation_recovery_metric() {
    let operation = TelemetryAttribute::metric_label("reservation_recovery_operation", reservation_recovery_operation_label(operation));
    let result = TelemetryAttribute::metric_label("reservation_recovery_result", reservation_recovery_result_label(result));
    metric_name: "prodex_reservation_recovery_events_total";
}
fn reservation_recovery_operation_label(operation: ReservationRecoveryOperation) {}
fn reservation_recovery_result_label(result: ReservationRecoveryResult) {}
pub fn plan_accounting_metric() {
    let operation = TelemetryAttribute::metric_label("accounting_operation", accounting_operation_label(operation));
    let result = TelemetryAttribute::metric_label("accounting_result", accounting_result_label(result));
    metric_name: "prodex_accounting_events_total";
}
fn accounting_operation_label(operation: AccountingOperation) {}
fn accounting_result_label(result: AccountingResult) {}
pub fn plan_billing_ledger_metric() {
    let operation = TelemetryAttribute::metric_label("billing_ledger_operation", billing_ledger_operation_label(operation));
    let result = TelemetryAttribute::metric_label("billing_ledger_result", billing_ledger_result_label(result));
    metric_name: "prodex_billing_ledger_events_total";
}
fn billing_ledger_operation_label(operation: BillingLedgerOperation) {}
fn billing_ledger_result_label(result: BillingLedgerResult) {}
pub fn plan_budget_rejection_metric() {
    let reason = TelemetryAttribute::metric_label("budget_rejection_reason", budget_rejection_reason_label(reason));
    metric_name: "prodex_budget_rejections_total";
}
fn budget_rejection_reason_label(reason: BudgetRejectionReason) {}
pub fn plan_rate_limit_decision_metric() {
    let scope = TelemetryAttribute::metric_label("rate_limit_scope", rate_limit_scope_label(scope));
    let decision = TelemetryAttribute::metric_label("rate_limit_decision", rate_limit_decision_label(decision));
    metric_name: "prodex_rate_limit_decisions_total";
}
fn rate_limit_scope_label(scope: RateLimitScope) {}
fn rate_limit_decision_label(decision: RateLimitDecision) {}
pub fn plan_redis_coordination_metric() {
    let operation = TelemetryAttribute::metric_label("redis_coordination_operation", redis_coordination_operation_label(operation));
    let result = TelemetryAttribute::metric_label("redis_coordination_result", redis_coordination_result_label(result));
    metric_name: "prodex_redis_coordination_events_total";
}
fn redis_coordination_operation_label(operation: RedisCoordinationOperation) {}
fn redis_coordination_result_label(result: RedisCoordinationResult) {}
pub fn plan_quota_correctness_metric() {
    let event = TelemetryAttribute::metric_label("quota_correctness_event", quota_correctness_event_label(event));
    metric_name: "prodex_quota_correctness_events_total";
}
fn quota_correctness_event_label(event: QuotaCorrectnessEvent) {}
pub fn plan_slo_alert_metric() {
    let sli = TelemetryAttribute::metric_label("slo_sli", slo_alert_sli_label(sli));
    let severity = TelemetryAttribute::metric_label("slo_severity", slo_alert_severity_label(severity));
    metric_name: "prodex_slo_alert_events_total";
}
fn slo_alert_sli_label(sli: SloAlertSli) {}
fn slo_alert_severity_label(severity: SloAlertSeverity) {}
pub fn plan_shutdown_lifecycle_metric() {
    let event = TelemetryAttribute::metric_label("shutdown_event", shutdown_lifecycle_event_label(event));
    let result = TelemetryAttribute::metric_label("shutdown_result", shutdown_lifecycle_result_label(result));
    metric_name: "prodex_shutdown_lifecycle_total";
}
fn shutdown_lifecycle_event_label(event: ShutdownLifecycleEvent) {}
fn shutdown_lifecycle_result_label(result: ShutdownLifecycleResult) {}
pub fn plan_health_probe_metric() {
    let probe = TelemetryAttribute::metric_label("health_probe", health_probe_kind_label(probe));
    let result = TelemetryAttribute::metric_label("health_result", health_probe_result_label(result));
    metric_name: "prodex_health_probe_results_total";
}
fn health_probe_kind_label(probe: HealthProbeKind) {}
fn health_probe_result_label(result: HealthProbeResult) {}
pub fn plan_secret_provider_metric() {
    let backend = TelemetryAttribute::metric_label("secret_backend", secret_provider_backend_label(backend));
    let operation = TelemetryAttribute::metric_label("secret_operation", secret_provider_operation_label(operation));
    let result = TelemetryAttribute::metric_label("secret_result", secret_provider_result_label(result));
    metric_name: "prodex_secret_provider_operations_total";
}
fn secret_provider_backend_label(backend: SecretProviderBackend) {}
fn secret_provider_operation_label(operation: SecretProviderOperation) {}
fn secret_provider_result_label(result: SecretProviderResult) {}
pub fn plan_secret_rotation_metric() {
    let scope = TelemetryAttribute::metric_label("secret_scope", secret_rotation_scope_label(scope));
    let result = TelemetryAttribute::metric_label("secret_rotation_result", secret_rotation_result_label(result));
    metric_name: "prodex_secret_rotation_events_total";
}
fn secret_rotation_scope_label(scope: SecretRotationScope) {}
fn secret_rotation_result_label(result: SecretRotationResult) {}
pub fn plan_backup_restore_metric() {
    let operation = TelemetryAttribute::metric_label("backup_restore_operation", backup_restore_operation_label(operation));
    let result = TelemetryAttribute::metric_label("backup_restore_result", backup_restore_result_label(result));
    metric_name: "prodex_backup_restore_events_total";
}
fn backup_restore_operation_label(operation: BackupRestoreOperation) {}
fn backup_restore_result_label(result: BackupRestoreResult) {}
pub fn plan_deployment_rollout_metric() {
    let operation = TelemetryAttribute::metric_label("deployment_rollout_operation", deployment_rollout_operation_label(operation));
    let result = TelemetryAttribute::metric_label("deployment_rollout_result", deployment_rollout_result_label(result));
    metric_name: "prodex_deployment_rollout_events_total";
}
fn deployment_rollout_operation_label(operation: DeploymentRolloutOperation) {}
fn deployment_rollout_result_label(result: DeploymentRolloutResult) {}
pub fn plan_load_soak_metric() {
    let scenario = TelemetryAttribute::metric_label("load_soak_scenario", load_soak_scenario_label(scenario));
    let result = TelemetryAttribute::metric_label("load_soak_result", load_soak_result_label(result));
    event_count_metric_name: "prodex_load_soak_events_total";
    duration_metric_name: "prodex_load_soak_duration_ms";
}
fn load_soak_scenario_label(scenario: LoadSoakScenarioKind) {}
fn load_soak_result_label(result: LoadSoakResult) {}
pub fn plan_fault_injection_metric() {
    let target = TelemetryAttribute::metric_label("fault_injection_target", fault_injection_target_label(target));
    let result = TelemetryAttribute::metric_label("fault_injection_result", fault_injection_result_label(result));
    metric_name: "prodex_fault_injection_events_total";
}
fn fault_injection_target_label(target: FaultInjectionTarget) {}
fn fault_injection_result_label(result: FaultInjectionResult) {}
pub fn plan_migration_lifecycle_metric() {
    let operation = TelemetryAttribute::metric_label("migration_operation", migration_lifecycle_operation_label(operation));
    let result = TelemetryAttribute::metric_label("migration_result", migration_lifecycle_result_label(result));
    metric_name: "prodex_migration_lifecycle_events_total";
}
fn migration_lifecycle_operation_label(operation: MigrationLifecycleOperation) {}
fn migration_lifecycle_result_label(result: MigrationLifecycleResult) {}
pub fn plan_persistence_metric() {
    let operation = TelemetryAttribute::metric_label("persistence_operation", persistence_operation_label(operation));
    let result = TelemetryAttribute::metric_label("persistence_result", persistence_result_label(result));
    metric_name: "prodex_persistence_operations_total";
}
fn persistence_operation_label(operation: PersistenceOperation) {}
fn persistence_result_label(result: PersistenceResult) {}
`,
      "crates/prodex-observability/src/lib.rs",
    ).length === 0,
    "valid JWKS age metric contract rejected",
  );
  assertSelfTest(
    validateRequiredObservabilityContracts("pub fn plan_jwks_cache_age_metric() {}", "crates/prodex-observability/src/lib.rs").some((error) =>
      error.includes("jwks_cache_state"),
    ),
    "missing JWKS state label contract accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  const manifest = await fs.readFile(path.join(repoRoot, OBS_MANIFEST), "utf8");
  const errors = [...validateObservabilityManifest(manifest), ...(await validateSources())];
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`observability-boundary-guard: ${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
