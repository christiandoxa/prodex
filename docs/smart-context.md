# Smart Context

Smart Context keeps protocol and continuation metadata exact while allowing independent context payload segments to be rewritten, rehydrated, or left untouched according to a local safety envelope. Explicit exact mode remains full request pass-through.

## Safety Model

- Control-plane fields stay exact: `previous_response_id`, turn state, session identifiers, function/tool-call IDs, ordering, protocol fields, and explicit exact-mode fields.
- Payload segments are classified independently as protocol exact, continuation exact, critical exact, rehydratable exact, lossless transformable, condensable, or droppable duplicate.
- Missing artifact references are segment-local failures. The dependent segment is preserved and recorded as unresolved; unrelated segments may still be optimized when validation passes.
- Hash-backed rehydration requires valid content hashes and requested line ranges.
- Validation checks JSON structure, continuation/tool integrity, mandatory references, critical-signal recall, nonempty mandatory payloads, duplicated appendices, and segment allocations before the rewritten request can be sent upstream.
- Whole-request fallback is reserved for explicit exact mode and failures that can affect protocol, continuation, or global structural correctness.

## Budgeting

Budgeting is model-relative. Pressure is computed as:

```text
effective_used_tokens / max(1, context_window - output_reserve)
```

Context-window selection is ordered as explicit launch configuration, versioned provider/model registry, observed runtime token accounting, then conservative fallback. Runtime telemetry records the pressure band, estimator confidence, usable window, and absolute safety floor so operators can distinguish low-confidence estimates from real context pressure.

## Rollout

- `PRODEX_SMART_CONTEXT_SHADOW=1` computes Smart Context decisions and telemetry while sending the original request upstream.
- `PRODEX_SMART_CONTEXT_CANARY_PERCENT=N` applies rewriting only for a deterministic percentage of requests. Canary-out requests pass through unchanged and log `rollout_canary_out`.
- Explicit exact mode bypasses rollout and remains full pass-through.

## Telemetry

Smart Context runtime logs use structured fields and avoid source contents by default. Important fields include:

- Size and token estimates: `body_bytes_before`, `body_bytes_after`, `estimated_tokens_before`, `estimated_tokens_after`, `body_bytes_saved`, `rewrite_ratio_percent`.
- Budget and pressure: `model_context_window_tokens`, `model_context_window_source`, `observed_context_tokens`, `available_tokens`, `pressure_basis_points`, `pressure_band`, `estimator_confidence`, `effective_usable_context_tokens`, `absolute_safety_floor_tokens`, `budget_mode`, `policy_reasons`.
- Transform counters: `artifacts_stored`, `tool_outputs_condensed`, `duplicate_texts`, `cross_turn_duplicate_texts`, `repeat_tool_output_refs`, `blob_outputs_condensed`, `rehydrated_refs`, `rehydration_token_cost`, `static_context_deltas`, `repo_state_facts`, `transformed_segment_categories`.
- Candidate/repair counters: `candidate_count`, `selected_candidate_count`, `rejected_candidate_count`, `selected_candidate_utility_points`, `segment_rollback_count`, `full_request_fallback_count`, `artifact_hash_failures`.
- Rollout: `rollout_mode`, `rollout_reason`, `rollout_canary_bucket`, `rollout_canary_percent`.
- Quality proxies: `task_quality_upstream_context_errors`, `task_quality_previous_response_not_found`, `task_quality_invalid_tool_call_continuation`, `task_quality_missing_artifact_requests`, `task_quality_repeated_tool_call_count`, `task_quality_model_reread_requests`, `task_quality_corrective_user_messages`, `task_quality_test_or_build_failed_after_rewrite`, `task_quality_task_completed`, `task_quality_additional_turns_before_completion`, `task_quality_final_total_input_tokens`.

Learning buckets support route, model, profile, provider, context-window band, session-length band, task class, and transform category. Optional dimensions are enforced when present, so low-quality samples from a different window band or transform category do not relax unrelated traffic.

## Replay Benchmark

The deterministic corpus is checked in at:

```text
crates/prodex-runtime-proxy/tests/fixtures/smart_context_replay_corpus.json
```

It contains exact/current/optimized comparisons for 12 long-session scenarios and covers the required hard cases: 30+ turn continuation, repeated build/test output, compiler/runtime errors, large diffs, repository navigation, multi-file refactoring, changing static instructions, missing/corrupted artifacts, duplicate tool calls/output, noisy binary-like command output, failure recovery, and 16k/32k/128k/200k context windows.

Run the strict report with:

```bash
cargo run -q -- context replay-report crates/prodex-runtime-proxy/tests/fixtures/smart_context_replay_corpus.json --strict
```

Current benchmark result:

```text
eligible_long_sessions: 12
current_comparison_sessions: 12
median_input_token_reduction_percent_vs_exact: 45
current_median_input_token_reduction_percent_vs_exact: 18
median_additional_input_token_reduction_percent_vs_current: 32
long_sessions_with_at_least_20_percent_reduction_percent: 100
exact_median_total_tokens_until_completion: 34000
current_median_total_tokens_until_completion: 29000
optimized_median_total_tokens_until_completion: 21000
exact_success_rate_percent: 100
current_success_rate_percent: 100
optimized_success_rate_percent: 100
optimized_missing_context_recovery_turns: 0
success_regression_basis_points: 0
continuation_integrity_percent: 100
tool_call_integrity_percent: 100
critical_signal_recall_percent: 100
unresolved_mandatory_artifact_refs: 0
corrupted_json_count: 0
p95_rewrite_overhead_ms: 28
continuation_fallback_rate_percent: 0
required_replay_coverage: complete
passed: true
```

## Migration Note

Existing Smart Context users do not need to change configuration. The default behavior remains local and deterministic, with safe fallback preserved. Operators can start with shadow mode, then set a small canary percentage, inspect `smart_context_autopilot` log fields, and increase rollout only after replay and runtime telemetry remain clean.

## Remaining Risks

- The replay corpus is deterministic and local; it proves acceptance metrics over representative fixtures, not live model quality across all repositories.
- Optional embeddings are not used by default. Candidate selection remains deterministic, local, and metadata-driven.
- Quality proxy fields are conservative and privacy-safe; they can flag risk and tighten policy, but they are not a substitute for task-specific integration tests.
