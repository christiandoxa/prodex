# Gemini quota response classification migrated to compiled Mojo

The production Gemini error and retry pipeline now delegates its nested JSON
quota classification to a single compiled Mojo kernel. Rust remains responsible
for decoding ordinary JSON and SSE `data:` frames with Serde, and for building
the borrowed node tree. Mojo traverses the decoded tree and independently
reports terminal quota exhaustion, rate-limit signals, and the most relevant
explicit-quota message (or the established generic quota message).

The old Rust recursive JSON-stack classifiers in `quota_retry.rs`, including
terminal quota, rate-limit and daily-limit checks, and the duplicate nested
quota-message traversal in `body.rs`, were removed. Rust retains the separate
retry-delay duration parser pending its own migration and applies the Mojo
quota facts to existing response routing, error normalization and retry policy.
There is no Rust fallback quota classifier if the compiled ABI fails.

The compiled kernel preserves case-insensitive trimmed quota/rate-limit codes,
case-sensitive `PerDay`/`Daily` quota identifier detection, the existing
`quotaId` > `quota_limit` > `quotaLimit` property precedence, and the
`message` > `detail` > `error` field precedence. It reproduces the original
reverse-sibling depth-first visitation induced by Rust's BTreeMap/Vec stack,
so competing quota messages retain their established selection order. A
caller-owned previous-sibling scratch index keeps traversal linear rather
than quadratic for large nested responses.

The versioned ABI validates node topology, UTF-8 spans, pointers, array bounds
and output fields. Direct ABI tests include malformed pointers/capacities and
exact quota/rate-limit messages. The production caller regression verifies
Unicode, nested arrays, quota daily metadata, nonstring codes, empty messages,
competing nested messages, unknown rate codes and mixed valid/invalid SSE
frames. All 469 provider-core library tests pass, as do the compiled ABI tests.
The no-fallback guard requires the production Mojo call and prevents restoring
Rust recursive classifiers.

The independent long-term 75% production-share goal remains unmet.
