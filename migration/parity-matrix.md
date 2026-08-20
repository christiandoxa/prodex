# Rust-Mojo parity matrix

Rust remains the behavioral oracle. Rows are promoted only after exact output parity.

| Component | Rust oracle | Mojo entry point | Inputs | Expected output | Coverage | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Quota remaining percent | `prodex_quota::remaining_percent` Rust branch | `prodex_quota_remaining_percent` | `None`, negative, 0, middle, 100, over 100, extremes | Exact `i64` result | `remaining_percent_matches_rust_oracle` | `MOJO_ENABLED` |
| Quota window status | Rust threshold branches in `quota_window_summary` | `prodex_quota_window_status` | missing window, 0, 1, 5, 6, 15, 16, 100 | Exact status tag | `quota_window_status_matches_rust_oracle` | `MOJO_ENABLED` |
| Quota pressure band | Rust status mapping and max aggregation | `prodex_quota_pressure_band` | every status pair and single status | Exact band tag | `quota_pressure_band_matches_rust_oracle` | `MOJO_ENABLED` |
| Quota window-pair readiness | Rust `window_pair_has_ready_limit` | `prodex_quota_window_pair_has_ready_limit` | empty, partial, ready, exhausted, extreme values | Exact boolean | `quota_window_pair_readiness_matches_rust_oracle` | `MOJO_ENABLED` |
| Runtime route quota pressure | Rust `runtime_proxy_quota_pressure_band_for_route` | `prodex_runtime_quota_pressure_band_for_route` | four routes, missing/negative/threshold/exhausted windows | Exact band tag | `route_pressure_band_matches_rust_oracle_for_all_routes_and_boundaries` | `MOJO_ENABLED` |
| Runtime profile quota scoring | `runtime_proxy_quota_profile_score_rust` | `prodex_runtime_quota_profile_score_batch` | bounded profile batches, saturation, missing-window sentinels | Exact four-field score | `profile_score_batch_matches_the_rust_arithmetic_oracle` | `MOJO_ENABLED` |
| Quota fraction/amount conversion | `gemini_bucket_remaining_percent` | Not started | Fractions, amounts, missing totals | Exact rounded `i64`/`Option` | Existing Rust tests | `AUDIT_ONLY` |
| Quota pool aggregation | `aggregate_main_quota` and pool render helpers | Not started | Multiple provider reports | Exact aggregate/sort output | Existing Rust tests | `AUDIT_ONLY` |
| Provider ranking score | `prodex-provider-spi::score_provider_rust` | `prodex_routing_score_batch` | normalized provider signals, weights, affinity, ties | Exact components, weighted total, score | `routing_score_batch_matches_rust_oracle_for_seeded_vectors`; score sub-batch of the routing plan | `MOJO_ENABLED` |
| Governed provider routing plan | Rust feature-off `plan_governed_provider_route` path and governed-routing invariants | `prodex_routing_plan_batch` | Up to 64 candidates: hard-eligibility flags, seven-bit capability masks, provider order, seven normalized signals, quota presence, affinity, required mask, weights | Per-candidate eligibility/reason tag, seven score components, weighted total, score, and stable eligible index order | `routing_plan_matches_rust_oracle_for_fixed_seed_batches` (4 × 192 generated cases), tie/affinity/boundary cases, strict provider SPI suite | `MOJO_ENABLED` |
| Provider route capability matching | `negotiate_capability` and Rust provider-route negotiation | `prodex_capability_match_batch` | Up to 64 well-formed flags, seven-bit capability masks, required mask | Compatible flags, malformed/missing reason tags, first compatible index, first well-formed incompatible index | `capability_matching_matches_rust_oracle_for_fixed_seed_batches` (4 × 256 generated cases), malformed/tie cases, and provider negotiation integration tests | `MOJO_ENABLED` |
| Accounting reservation commit | `prodex-domain::accounting::commit_reservation` Rust arithmetic | Not started; no production Mojo seam | Reserved/actual token and cost values, snapshot totals, overflow boundaries | Rust-owned result and durable accounting contract | Domain/storage regression suites | `RUST_ONLY` |
| Rate-limit admission arithmetic | `prodex-domain::rate_limit::evaluate_rate_limit` Rust helper | Not started; distributed path is Redis/runtime-owned | Fixed-width counters, reset/clock values, valid/invalid windows, tenant gate | Rust-owned admission and persistence contract | Domain/runtime/storage regression suites | `RUST_ONLY` |
| Session affinity | Runtime proxy binding/selection helpers | Not started | `previous_response_id`, turn state, `session_id` | Same owning profile | Runtime serial suites | `KEEP_RUST` |
| Smart Context byte estimate | `smart_context_estimate_tokens_from_body_bytes` Rust branch | `prodex_smart_context_estimate_tokens_from_body_bytes` | zero, rounding boundaries, `usize::MAX` | Exact saturated `u64` estimate | `smart_context_byte_estimate_matches_rust_oracle_at_boundaries` | `MOJO_ENABLED` |
| Smart Context pressure snapshot | `smart_context_pressure_snapshot` Rust fallback | `prodex_smart_context_pressure_snapshot` | optional window, reserved output, effective input, source tag, risk flags, integer extremes | Exact usable tokens, pressure basis points, pressure band, safety floor, confidence | `pressure_snapshot_matches_rust_oracle_for_generated_inputs` (300 fixed-seed cases), production token-accounting tests | `MOJO_ENABLED` |
| Runtime candidate execution plan | `build_runtime_response_candidate_execution_plan` Rust sort path | `prodex_runtime_candidate_plan_batch` | Up to 256 normalized 22-field candidate rows and route tag | Exact ready order, fallback order, stable ties, backoff tuple precedence | `candidate_plan_matches_rust_oracle_for_generated_batches` (300 fixed-seed cases), strict runtime-proxy suite | `MOJO_ENABLED` |
| Smart Context candidate scoring | `smart_context_candidate_score` and selection | Not started | Normalized candidate batch | Exact score/order | Smart Context regression fixtures | `AUDIT_ONLY` |

## Promotion rule

Every future row needs normal, empty, invalid, boundary, extreme, and randomized inputs
where meaningful. A Mojo result mismatch disables the feature for that component and
keeps Rust production behavior until the root cause is fixed.

## Real Mojo CI coverage

| Subsystem | Compiled in real Mojo CI | Executed in real Mojo CI | Fallback disabled | Target |
| --- | --- | --- | --- | --- |
| Quota core | Yes | Yes, including C-ABI smoke and differential tests | Yes, `PRODEX_MOJO_REQUIRED=1` plus activation assertion | Ubuntu 24.04 x86_64 |
| Runtime route quota pressure | Yes | Yes, boundary differential tests | Yes, `PRODEX_MOJO_REQUIRED=1` plus activation assertion | Ubuntu 24.04 x86_64 |
| Runtime profile quota scoring | Yes | Yes, saturation/batch differential tests | Yes, strict activation assertion | Ubuntu 24.04 x86_64 |
| Provider ranking and routing plan | Yes | Yes, provider SPI strict suite covers score, candidate filtering, and stable plan ordering | Yes, strict activation assertion | Ubuntu 24.04 x86_64 |
| Provider route capability matching | Yes | Yes, provider SPI strict suite covers compatible, incompatible, and empty-candidate behavior; malformed handling remains in the contract | Yes, strict activation assertion | Ubuntu 24.04 x86_64 |
| Smart Context byte estimate | Yes | Yes, boundary differential test and binary self-test path | Yes, strict activation assertion | Ubuntu 24.04 x86_64 |
| Smart Context pressure snapshot | Yes | Yes, 300-case generated differential suite, token-accounting production path, and module self-test | Yes, strict activation assertion | Ubuntu 24.04 x86_64 |
| Runtime candidate execution plan | Yes | Yes, 300-case generated ordering suite and public runtime-proxy selection path | Yes, strict activation assertion | Ubuntu 24.04 x86_64 |
