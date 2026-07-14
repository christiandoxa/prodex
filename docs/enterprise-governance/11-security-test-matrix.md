# Security Test Matrix

The authoritative machine-readable inventory is
[`test-matrix.json`](test-matrix.json). Every row names a threat, control,
mode, phase, test type, expected result and implementation status. `planned`
means required evidence is not yet present; it is not a passing result.

The matrix also uses `tested` only when it names the focused test or guard,
`implemented` when production behavior exists but a broader gate remains, and
`pending_validation` only when the required backend execution evidence is not
available.

## Candidate evidence register

| Control | Status | Named evidence |
| --- | --- | --- |
| Identity/header trust | tested | `production_edge_security_uses_peer_trust_and_rejects_host_origin_csrf_spoofing`; `validated_peer_metadata_maps_to_low_cardinality_governance_zone`; `principal_evidence_enforces_required_scope_and_anonymous_policy`; auth boundary guard |
| Bounded inspection/classification | tested | `inspection_result_is_bounded_deterministic_and_content_free`; schema walker/local detector bounds; application inspection tests |
| PDP and obligations | tested | `explicit_deny_wins_and_drops_obligations`; `policy_compilation_is_bounded_and_rejects_duplicate_ids`; governance obligation matrix |
| Execution approval | tested | `execution_fingerprint_is_stable_and_bound_to_the_request_context`; `execution_approval_is_policy_selected_quorum_gated_and_one_use`; `gateway_execution_approval_http_lists_shows_and_reviews_without_payloads` |
| Provider routing | tested | `governed_routing_runtime_signals_change_selection_and_keep_only_eligible_fallbacks`; `provider_registry_resolves_selected_heterogeneous_projected_adapter`; precommit-only retry and revocation suites |
| Channel boundary | tested | `provider_route_mapping_covers_forwarded_data_plane_routes`; production/application boundary guards |
| Upstream transparency/no postcommit retry | tested | generic 429/content-policy pass-through and precommit-only error-policy regressions |
| Bank startup configuration | tested | `bank_governance_deployment_matrix_fails_closed`; listener guard; deployment-security guard |
| PostgreSQL/RLS and SIEM | tested | live disposable PostgreSQL/TLS/RLS lifecycle and outbox proof passed |
| Session revocation epoch | tested | `cross_replica_revocation_epoch_invalidates_cached_sessions_promptly`; deployed two-gateway chaos remains external evidence |
| Low-cardinality metrics | tested | `prometheus_text_aggregates_keys_without_high_cardinality_labels`; live alert/pager/SLO acceptance remains external evidence |
| Encrypted restore | tested | AES-256-GCM backup/isolated restore drill; final synthetic RPO 2.004 s and RTO 1.442 s with governance links intact |
| Performance | implemented | maximum-bound and disabled-path Criterion budgets, fuzz, load and stress passed; external multi-replica soak remains deployment evidence |

## Declared residuals

- Dynamic governed routing resolves eligible heterogeneous projected adapters
  and permits bounded eligible-set fallback only before response commitment.
- Mandatory governed data-plane audit is synchronous precommit I/O.
- Policy-selected Presidio and guardrail-webhook calls are bounded request-path
  network dependencies.
- Shared-authority revocation epochs trigger a 250-millisecond poll path; a
  deployed two-gateway chaos run, Redis restart, managed-database failover and
  external SIEM outage exercises remain deployment acceptance work.
- Configured `gateway.workload_identity` or `mtls_required` fails startup as
  unsupported until the runtime verifies the workload token and mTLS peer.
  Trusted external termination remains an unverified acceptance contract.

## Required suites

| Area | Minimum evidence |
| --- | --- |
| Tenant and identity | forged headers, confused deputy, cross-tenant/RLS, session fixation/revocation |
| Inspection | Unicode byte ranges, nested tool arguments, depth/size limits, unsupported modality, fail-open/closed modes |
| Policy | golden effects/reasons, missing attributes, obligation conflict, snapshot invalidation/LKG |
| Approval | self/stale/replayed approval, quorum races, expiry, use count, break-glass overreach |
| Routing | hard-filter before score, fixed-point determinism, pinning/revocation, eligible fallback, no postcommit retry |
| Audit/SIEM | atomic rollback, chain tamper/fork, outbox duplicate/lease/backlog, content canary |
| Secrets | raw-secret rejection, namespace authorization, cache expiry, rotation/revocation and Vault outage |
| Storage/HA | migrations, PostgreSQL RLS, restore, multi-replica activation, Redis loss and DB failover |
| Deployment | non-root/read-only, private bind, deny-by-default egress, image/provenance scanning |
| Performance | disabled overhead, stage p99 budgets, load/soak and bounded cardinality |

## Adversarial invariants

Tests must prove that no caller-controlled field becomes trusted identity; no
classification can be lowered by an untrusted input; no soft score can restore
an ineligible provider; no approval can authorize outside the active policy; no
continuation survives explicit revocation; no mandatory mutation succeeds
without audit/outbox; no secret enters logs/evidence; and no retry or rotation
occurs after response commitment.

## CI policy

Unit, golden and property suites run on every change. Architecture guards,
format, lint, full workspace tests, docs/JSON validation and secret/PII scans are
required merge gates. Integration, PostgreSQL/RLS, multi-replica, restore,
container/Kubernetes security and benchmark jobs run in their declared CI
environments. Fuzz, load, soak and chaos jobs retain seeds and raw artifacts and
run on scheduled/release gates according to cost.

Exceptions are time-bounded, owner-approved and auditable. They identify the
exact missing control/test, affected modes, compensating control and expiry;
they never convert a planned row to passed.
