# Smart Context structural JSON safety limits migrated to Mojo

The compiled `prodex_smart_context_json_shape_scan_v1` operation now
owns the deterministic security thresholds and first-violation precedence
for Smart Context JSON: maximum structural depth 64 and maximum 50,000
visited nodes. Mojo checks each depth before accounting for that node,
exactly as the original depth-first/LIFO Rust policy.

Rust retains the serde_json Value representation and structural iteration,
collecting at most 1,024 depth facts per bounded ABI call. Rust no longer
compares a visited node's depth or cumulative count against the limit or
implements a second error-priority decision. Its remaining work is host
structure traversal, batch marshalling and translating validated Mojo
reason tags to the existing public strings. No Rust semantic fallback
is retained.

## Required-Mojo verification

- Direct ABI tests: 2 passed, including exact limits, cross-batch node
  counts, same-node precedence for depth vs count, untrusted depths and
  invalid ABI inputs.
- Production shape-guard regressions: 3 passed, including JSON objects,
  depth 64 vs 65 and 50,000 vs 50,001 total nodes across 1,024-node batches.
- Full required-Mojo runtime-proxy library suite: 437 passed.
- Clippy `--all-targets -- -D warnings`: PASS.
- Ownership, authority, no-fallback and production-share guards: PASS.
- Official broad production share: 33.47% Mojo
  (112,376 Mojo LOC out of 335,712 production LOC).
  The 75% project target remains unmet.
