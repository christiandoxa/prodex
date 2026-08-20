# Rust-Mojo parity matrix

Rust remains the behavioral oracle. Rows are promoted only after exact output parity.

| Component | Rust oracle | Mojo entry point | Inputs | Expected output | Coverage | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Quota remaining percent | `prodex_quota::remaining_percent` Rust branch | `prodex_quota_remaining_percent` | `None`, negative, 0, middle, 100, over 100, extremes | Exact `i64` result | `remaining_percent_matches_rust_oracle` | `MOJO_ENABLED` |
| Quota window status | Rust threshold branches in `quota_window_summary` | `prodex_quota_window_status` | missing window, 0, 1, 5, 6, 15, 16, 100 | Exact status tag | `quota_window_status_matches_rust_oracle` | `MOJO_ENABLED` |
| Quota pressure band | Rust status mapping and max aggregation | `prodex_quota_pressure_band` | every status pair and single status | Exact band tag | `quota_pressure_band_matches_rust_oracle` | `MOJO_ENABLED` |
| Quota window-pair readiness | Rust `window_pair_has_ready_limit` | `prodex_quota_window_pair_has_ready_limit` | empty, partial, ready, exhausted, extreme values | Exact boolean | `quota_window_pair_readiness_matches_rust_oracle` | `MOJO_ENABLED` |
| Runtime route quota pressure | Rust `runtime_proxy_quota_pressure_band_for_route` | `prodex_runtime_quota_pressure_band_for_route` | four routes, missing/negative/threshold/exhausted windows | Exact band tag | `route_pressure_band_matches_rust_oracle_for_all_routes_and_boundaries` | `MOJO_ENABLED` |
| Quota fraction/amount conversion | `gemini_bucket_remaining_percent` | Not started | Fractions, amounts, missing totals | Exact rounded `i64`/`Option` | Existing Rust tests | `AUDIT_ONLY` |
| Quota pool aggregation | `aggregate_main_quota` and pool render helpers | Not started | Multiple provider reports | Exact aggregate/sort output | Existing Rust tests | `AUDIT_ONLY` |
| Provider ranking | `runtime_response_candidate_plan` and provider SPI routing | Not started | Normalized candidate batch | Exact order and skip reasons | Runtime replay suites | `AUDIT_ONLY` |
| Session affinity | Runtime proxy binding/selection helpers | Not started | `previous_response_id`, turn state, `session_id` | Same owning profile | Runtime serial suites | `KEEP_RUST` |
| Smart Context scoring | `smart_context_candidate_score` and selection | Not started | Normalized candidate batch | Exact score/order | Smart Context regression fixtures | `AUDIT_ONLY` |

## Promotion rule

Every future row needs normal, empty, invalid, boundary, extreme, and randomized inputs
where meaningful. A Mojo result mismatch disables the feature for that component and
keeps Rust production behavior until the root cause is fixed.

## Real Mojo CI coverage

| Subsystem | Compiled in real Mojo CI | Executed in real Mojo CI | Fallback disabled | Target |
| --- | --- | --- | --- | --- |
| Quota core | Yes | Yes, including C-ABI smoke and differential tests | Yes, `PRODEX_MOJO_REQUIRED=1` plus activation assertion | Ubuntu 24.04 x86_64 |
| Runtime route quota pressure | Yes | Yes, boundary differential tests | Yes, `PRODEX_MOJO_REQUIRED=1` plus activation assertion | Ubuntu 24.04 x86_64 |
