# Gemini nested retry-delay candidate scan and aggregation in compiled Mojo

The production Gemini retry-delay pipeline now delegates nested JSON field
selection, rate-limit and per-minute quota minimums, and the final maximum
across all valid JSON/SSE frames to a versioned compiled Mojo owner.
The previous Rust recursive JSON stack, quota-identifier recursion, rate-limit
code matcher, 10/60-second policy, and `Option<u64>` max aggregator were deleted
without a Rust fallback. Serde still owns parsing ordinary JSON and SSE
`data:` frames and provides a borrowed tree to Mojo.

Rust retains the existing standard-library `f64` parsing of individual
`retryDelay` and message duration strings. That boundary is deliberate:
Rust's accepted decimal/scientific notation, Unicode whitespace trimming,
ceiling to milliseconds, saturating conversion and malformed-value behavior
must remain byte-compatible until a separately qualified numeric kernel can
replace the standard parser. This Rust boundary no longer decides which JSON
fields are eligible or which delay wins. The existing compiled Gemini retry
policy still chooses the final retry strategy using the derived body delay.

The new ABI emits only borrowed JSON string-node indices tagged as explicit
`retryDelay` or message/detail/error text, plus the Mojo-computed floor.
A companion compiled kernel computes the maximum after Rust converts those
candidate strings to millisecond values. The ABI validates all tree shapes,
UTF-8, pointers, output capacity and array indices before accepting results.

Direct Mojo tests cover rate-limit floors, candidate extraction, empty
candidate sets, saturating maxima and malformed ABI inputs without writes.
Production caller tests cover nested messages, `1.5s`, `250ms`, scientific
notation, per-minute quota metadata, rate-limit codes, values exceeding
heuristic floors, zeros, malformed frames and multi-frame SSE maxima. All
470 provider-core tests pass. The no-fallback guard rejects reintroduced
Rust JSON-stack traversal, quota minimums and body-delay max aggregation.

The separate long-term 75% Mojo production-share target remains unmet.
