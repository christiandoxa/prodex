# Rust-Mojo parity matrix

Rust remains the behavioral oracle. Rows are promoted only after exact output parity.

| Component | Rust oracle | Mojo entry point | Inputs | Expected output | Coverage | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Quota remaining percent | `prodex_quota::remaining_percent` Rust branch | `prodex_quota_remaining_percent` | `None`, negative, 0, middle, 100, over 100, extremes | Exact `i64` result | `remaining_percent_matches_rust_oracle` | `PARITY_PASS` |
| Quota fraction/amount conversion | `gemini_bucket_remaining_percent` | Not started | Fractions, amounts, missing totals | Exact rounded `i64`/`Option` | Existing Rust tests | `AUDIT_ONLY` |
| Quota pool aggregation | `aggregate_main_quota` and pool render helpers | Not started | Multiple provider reports | Exact aggregate/sort output | Existing Rust tests | `AUDIT_ONLY` |
| Provider ranking | `runtime_response_candidate_plan` and provider SPI routing | Not started | Normalized candidate batch | Exact order and skip reasons | Runtime replay suites | `AUDIT_ONLY` |
| Session affinity | Runtime proxy binding/selection helpers | Not started | `previous_response_id`, turn state, `session_id` | Same owning profile | Runtime serial suites | `KEEP_RUST` |
| Smart Context scoring | `smart_context_candidate_score` and selection | Not started | Normalized candidate batch | Exact score/order | Smart Context regression fixtures | `AUDIT_ONLY` |

## Promotion rule

Every future row needs normal, empty, invalid, boundary, extreme, and randomized inputs
where meaningful. A Mojo result mismatch disables the feature for that component and
keeps Rust production behavior until the root cause is fixed.
