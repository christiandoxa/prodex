# Rust-Mojo parity matrix

Rust is a temporary parity oracle. A row is `MOJO` only when the compiled Mojo result is
authoritative on the supported Mojo target; a Rust-only target is a separate build policy,
not runtime fallback.

| Component | Rust oracle | Mojo entry point | Inputs | Expected output | Coverage | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Quota remaining percent | `prodex_quota::remaining_percent` Rust branch | `prodex_quota_remaining_percent` | `None`, negative, 0, middle, 100, over 100, extremes | Exact `i64` result | `remaining_percent_matches_rust_oracle` | `MOJO` |
| Quota window status | Rust threshold branches in `quota_window_summary` | `prodex_quota_window_status` | missing window, 0, 1, 5, 6, 15, 16, 100 | Exact status tag | `quota_window_status_matches_rust_oracle` | `MOJO` |
| Quota pressure band | Rust status mapping and max aggregation | `prodex_quota_pressure_band` | every status pair and single status | Exact band tag | `quota_pressure_band_matches_rust_oracle` | `MOJO` |
| Quota window-pair readiness | Rust `window_pair_has_ready_limit` | `prodex_quota_window_pair_has_ready_limit` | empty, partial, ready, exhausted, extreme values | Exact boolean | `quota_window_pair_readiness_matches_rust_oracle` | `MOJO` |
| Runtime route quota pressure | Rust `runtime_proxy_quota_pressure_band_for_route` | `prodex_runtime_quota_pressure_band_for_route` | four routes, missing/negative/threshold/exhausted windows | Exact band tag | `route_pressure_band_matches_rust_oracle_for_all_routes_and_boundaries` | `MOJO` |
| Runtime quota window summary | Rust `runtime_proxy_quota_window_summary` status thresholds | Reused `prodex_quota_window_status` and `prodex_quota_pressure_band` | Active proxy observations and usage snapshots | Exact status and pressure tags | `quota_window_summary_uses_the_compiled_status_kernel` plus runtime quota tests | `MOJO` |
| Runtime profile scheduling order | Rust-only `ready_profile_runtime_sort_key_from_score` comparator | `prodex_runtime_quota_profile_schedule_batch` | Up to 256 normalized 16-field profile rows, including provider, cooldown, raw pressure/window completeness, reset, source, preferred, and input order | Exact stable ordered indices; Mojo derives scaling, reserve bias, and ordering | Fixed ordering oracle, healthy/thin/critical/exhausted/unknown boundary fixtures, and runtime quota scheduler tests | `MOJO` |
| Runtime policy numeric validation | One shared primitive evaluator for Rust-only targets | `prodex_runtime_policy_validate_numeric` | Section-sized non-zero, bounded-range, and `<=` relation rule batches | Exact failed-rule indices in input order | Fixed boundary fixtures, a 130-rule ABI batch, and runtime-policy section tests | `MOJO` |
| Critical-signal loss/gain arithmetic | Rust `saturating_sub` counter oracle | `prodex_context_signal_diff` | Seven normalized counters before/after line classification | Exact lost and gained counters | `signal_diff_matches_rust_oracle_for_generated_counters` (2,000 fixed-seed cases), context compaction suites | `MOJO` |
| Critical-signal UTF-8 grouping | Test-only Rust `critical_signal_normalized_rows_rust` oracle | `prodex_context_prepare_signal_rows_v1` | Up to 65,536 non-secret `ProdexStringView` records per side plus seven counters per line | Exact key IDs, duplicate counts, row fields, required capacities, and status | 512 generated end-to-end cases; empty, Unicode, combining, emoji, embedded nul, long, malformed/truncated UTF-8, null/length, ABI mismatch, capacity, repeated, and concurrent calls | `MOJO` |
| Gemini quota numeric batch | Shared Rust-only platform evaluator | `prodex_quota_gemini_bucket_batch` | Rust-parsed remaining amounts plus optional `f64` fractions | Exact remaining, total, rounded percent, and exhausted flag | Normalized golden presence/boundary fixture and quota renderer tests | `MOJO` |
| Quota pool aggregation | `aggregate_main_quota` and pool render helpers | `prodex_quota_main_aggregate_batch` | Up to 1,024 normalized main-quota rows: presence, remaining percent, reset epoch | Exact profile count, saturating remaining sum, earliest reset | `main_quota_aggregate_matches_rust_oracle_for_generated_rows` (2,000 fixed-seed cases) plus quota render tests | `MOJO` |
| Provider ranking score | Rust-only feature-off `prodex-provider-spi::score_provider_rust` test oracle | Internal score sub-batch of `prodex_routing_plan_batch` | normalized provider signals, weights, affinity, ties | Exact components, weighted total, score | `routing_score_batch_matches_rust_oracle_for_seeded_vectors` exercises scores through the production routing plan | `MOJO` |
| Governed provider routing plan | Rust-only feature-off `plan_governed_provider_route` test oracle and governed-routing invariants | `prodex_routing_plan_batch` | Up to 64 candidates: hard-eligibility flags, seven-bit capability masks, provider order, seven normalized signals, quota presence, affinity, required mask, weights | Per-candidate eligibility/reason tag, seven score components, weighted total, score, and stable eligible index order | `routing_plan_matches_rust_oracle_for_fixed_seed_batches` (4 × 192 generated cases), tie/affinity/boundary cases, strict provider SPI suite | `MOJO` |
| Provider route capability matching | Rust-only feature-off negotiation test oracle | `prodex_capability_match_batch` | Up to 64 well-formed flags, seven-bit capability masks, required mask | Compatible flags, malformed/missing reason tags, first compatible index, first well-formed incompatible index | `capability_matching_matches_rust_oracle_for_fixed_seed_batches` (4 × 256 generated cases), malformed/tie cases, and provider negotiation integration tests | `MOJO` |
| Accounting reservation commit | `prodex-domain::accounting::commit_reservation` Rust arithmetic | Not started; no production Mojo seam | Reserved/actual token and cost values, snapshot totals, overflow boundaries | Rust-owned result and durable accounting contract | Domain/storage regression suites | `RUST_ONLY` |
| Rate-limit admission arithmetic | `prodex-domain::rate_limit::evaluate_rate_limit` Rust helper | Not started; distributed path is Redis/runtime-owned | Fixed-width counters, reset/clock values, valid/invalid windows, tenant gate | Rust-owned admission and persistence contract | Domain/runtime/storage regression suites | `RUST_ONLY` |
| Session affinity | Runtime proxy binding/selection helpers | Not started | `previous_response_id`, turn state, `session_id` | Same owning profile | Runtime serial suites | `KEEP_RUST` |
| Smart Context byte estimate | Rust-only feature-off `smart_context_estimate_tokens_from_body_bytes` test oracle | `prodex_smart_context_estimate_tokens_from_body_bytes` | zero, rounding boundaries, `usize::MAX` | Exact saturated `u64` estimate | `smart_context_byte_estimate_matches_rust_oracle_at_boundaries` | `MOJO` |
| Smart Context pressure snapshot | `smart_context_pressure_snapshot` Rust-only test oracle | `prodex_smart_context_pressure_snapshot` | optional window, reserved output, effective input, source tag, risk flags, integer extremes | Exact usable tokens, pressure basis points, pressure band, safety floor, confidence | `pressure_snapshot_matches_rust_oracle_for_generated_inputs` (300 fixed-seed cases), production token-accounting tests | `MOJO` |
| Runtime candidate execution plan | Rust-only feature-off sort oracle | `prodex_runtime_candidate_plan_batch` | Up to 256 normalized 22-field candidate rows and route tag | Exact ready order, fallback order, stable ties, backoff tuple precedence | `candidate_plan_matches_rust_oracle_for_generated_batches` (300 fixed-seed cases), strict runtime-proxy suite | `MOJO` |
| Optimistic current-candidate decision | Rust-only feature-off predicate test oracle | `prodex_runtime_optimistic_current_candidate_decision` | Normalized route/source/band tags, booleans, bounded counters, and Rust-normalized prompt-cache presence/owner match | Keep or exact first ordered skip reason | `optimistic_current_candidate_matches_rust_oracle_for_generated_inputs` (5,000 fixed-seed cases), precedence fixtures, strict runtime-proxy suite | `MOJO` |
| Provider request constraints | Rust-only feature-off evaluator used as a differential test oracle | `prodex_provider_constraints_evaluate_v2` | Versioned 17/7-word input and 12/5-word output buffers after Rust parsing and typed normalization | Exact decision, eligibility, totals, context, output adjustment, missing feature, warnings | ABI count/version/tag/malformed-output tests, `provider_constraints_match_rust_oracle_for_generated_normalized_cases` (2,000 fixed-seed cases), 304 provider-core tests, strict provider suite | `MOJO` |
| Smart Context rehydration plan | Rust-only feature-off ordering/admission oracle | `prodex_smart_context_rehydrate_plan_batch` | Rust-ranked artifact rows with token cost, required/present flags, tier, and budget; maximum 256 | Exact rehydrate/defer tags and used-token total; Rust restores IDs | `rehydrate_plan_matches_rust_oracle_for_generated_inputs` (2,000 fixed-seed cases), active body-transform path | `MOJO` |
| Runtime tuning defaults | Rust-only feature-off `runtime_tuning_defaults_rust` test oracle | `prodex_runtime_tuning_defaults` | Normalized host parallelism | Exact worker/log/websocket default tuple | `tuning_defaults_match_rust_oracle_for_generated_parallelism` (2,000 fixed-seed cases), runtime config and probe queue callers | `MOJO` |
| Smart Context candidate scoring | `smart_context_candidate_score` and selection | Not started | Normalized candidate batch | Exact score/order | Smart Context regression fixtures | `AUDIT_ONLY` |

## Prodex 0.419.0 additions

| Component | Rust oracle | Mojo entry point | Inputs | Expected output | Coverage | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Provider catalog identity and choices | feature-off catalog matcher/planner | `prodex_mojo_rich_catalog_resolve_v1`, `prodex_mojo_rich_catalog_choices_v1` | bounded provider catalog, aliases, configured IDs, current ID | canonical identity and stable provider/default/configured/custom choice order | provider-core catalog suite, alias/order/capacity tests | `MOJO` |
| Provider catalog merge deduplication | feature-off catalog merge oracle | `prodex_mojo_rich_catalog_merge_v1` | bounded canonical IDs, aliases, additional IDs | accepted additional indices with alias/canonical deduplication | provider-core merge suite and rich alias regression | `MOJO` |
| Route-aware quota pressure score | `runtime_proxy_quota_score_for_route_rust` | `prodex_runtime_quota_score_batch` | bounded two-window observations and route tag | pressure band, weighted pressure, reserve floor, remaining/reset values | 300 generated parity cases and runtime-quota batch/scalar equivalence | `MOJO` |
| Observed Smart Context usage totals | `smart_context_observed_usage_totals_rust` | `prodex_smart_context_token_usage_summary_batch` | bounded input/cached/output/reasoning token rows | saturating totals and last-observation accounting values | generated saturation parity and full Mojo runtime suite | `MOJO` |

## Promotion rule

Every future row needs normal, empty, invalid, boundary, extreme, and randomized inputs
where meaningful. Before promotion, a mismatch means Mojo is not ready. After promotion,
a mismatch is a CI/validation failure. It never selects a Rust implementation at runtime.
Rust may remain only as a separate Rust-only target build or a test oracle with an explicit
maintenance reason.

## Real Mojo CI coverage

| Subsystem | Compiled in real Mojo CI | Executed in real Mojo CI | Runtime fallback | Target |
| --- | --- | --- | --- | --- |
| Quota core | Yes | Yes, including C-ABI smoke and differential tests | None; `PRODEX_MOJO_REQUIRED=1` plus activation assertion | Ubuntu 24.04 x86_64 |
| Runtime route quota pressure | Yes | Yes, boundary differential tests | None; `PRODEX_MOJO_REQUIRED=1` plus activation assertion | Ubuntu 24.04 x86_64 |
| Runtime profile scheduling order | Yes | Yes, fixed ordering/boundary fixtures and the active scheduler path | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Runtime policy numeric validation | Yes | Yes, fixed rule boundaries, a 130-rule ABI batch, and runtime-policy section tests | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Critical-signal loss/gain arithmetic | Yes | Yes, 2,000-case counter differential suite and active context validation path | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Critical-signal UTF-8 grouping | Yes | Yes, raw text ABI tests, 512-case differential suite, and active lost-range/Smart Context callers | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Provider ranking and routing plan | Yes | Yes, provider SPI strict suite covers score, candidate filtering, and stable plan ordering | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Provider route capability matching | Yes | Yes, provider SPI strict suite covers compatible, incompatible, and empty-candidate behavior; malformed handling remains in the contract | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Smart Context byte estimate | Yes | Yes, boundary differential test and binary self-test path | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Smart Context pressure snapshot | Yes | Yes, 300-case generated differential suite, token-accounting production path, and module self-test | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Runtime candidate execution plan | Yes | Yes, 300-case generated ordering suite and public runtime-proxy selection path | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Optimistic current-candidate decision | Yes | Yes, 5,000-case precedence differential suite and normal runtime selection caller | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Provider request constraints | Yes | Yes, normalized provider-core production evaluation and 2,000-case differential suite | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Quota pool aggregation | Yes | Yes, normalized quota render aggregation and 2,000-case differential suite | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Smart Context rehydration | Yes | Yes, active Smart Context body-transform planner and 2,000-case differential suite | None; strict activation assertion | Ubuntu 24.04 x86_64 |
| Runtime tuning defaults | Yes | Yes, runtime config/probe queue defaults and 2,000-case differential suite | None; strict activation assertion | Ubuntu 24.04 x86_64 |

## Rich ABI v6 promotions (2026-08-28)

These rows supersede the baseline's normalization-only boundary descriptions.

| Component | Rust oracle | Mojo entry point | Inputs/outputs | Coverage | Status |
| --- | --- | --- | --- | --- | --- |
| Context diagnostic analysis | `critical_signal_counts_for_line` plus Rust normalized text | `prodex_mojo_rich_context_analyze_v2` | bounded UTF-8 text; grouped `DiagnosticRecord` table, counts, normalized output strings | existing context suite plus 20,000 generated Unicode/ANSI/CRLF cases | `MOJO` |
| Provider/model fallback parser | `provider_model_fallback_chain_rust` | `prodex_mojo_rich_model_fallback_v2` | provider/model UTF-8 views; ordered deduped model records | 20,000 valid/invalid `combo:` and alias cases plus exact Gemini catalog aliases | `MOJO` |
| Gateway route-alias policy parser | `validate_gateway_route_alias_rust` | `prodex_mojo_rich_policy_alias_v2` | alias, model list, optional strategy, metric list; normalized model records or issue fields | 20,000 generated valid/invalid grammar cases and existing policy suite | `MOJO` |
| Governed provider route plan | `plan_governed_provider_route_rust` | `prodex_mojo_rich_route_plan_v2` | provider/model/capability text and bounded signals; candidate objects, score components, reasons, order | 10,000 generated candidate sets plus existing provider SPI suite | `MOJO` |
| Smart Context rehydration plan | `smart_context_auto_rehydrate_plan_rust` | `prodex_mojo_rich_context_plan_v2` | opaque artifact-reference views, optional availability set, required/token fields; action graph | existing 2,000 generated cases and active app rehydration callers | `MOJO` |
| Runtime log event classification | Rust log-key classifier (test oracle) | `prodex_mojo_log_classify_v3` | bounded UTF-8 event key | category and severity tags used by the shared log renderer | classifier self-test, operational log rendering tests, and Mojo-enabled app log suite | `MOJO` |

Each Rust wrapper validates version, status, counts, offsets, lengths, UTF-8, indices, tags,
ordering, and duplicate invariants. The Rust implementations remain test-only or Rust-only-target
oracles and are never selected after a Mojo-enabled error.

The release-wide generated differential corpus contains 55,000 deterministic cases: 20,000
Unicode/ANSI/CRLF context cases, 20,000 valid/invalid fallback-parser cases, 10,000 governed
provider candidate sets, and 5,000 optimistic runtime-selection cases. Parser, routing, and
selection cases compare the Mojo result with a Rust oracle; malformed ABI and boundary corpora
remain additional negative coverage.
