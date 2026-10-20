# Runtime SSE Response ID Sorting and Deduplication Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled sse_line.mojo kernel now owns lexicographic
UTF-8 byte ordering and exact deduplication of response IDs collected
across SSE inspection events. The versioned
prodex_runtime_sse_response_id_order_v1 ABI accepts original borrowed
UTF-8 views and produces sorted unique source indices. It validates
every row before modifying results and uses an iterative heap sort,
with stable original-index priority when values are equal.

Rust retains the original response-ID String buffers, conversion of
selected indices into owned strings, SSE parser/callback plumbing and
the already Mojo-classified commitment/error decisions. The previous
Rust BTreeSet collection and deduplication policy were removed rather
than kept as a fallback. Events carrying no new IDs preserve the
already Mojo-sorted state without invoking a redundant sort.

## Validation

- All 425 prodex-runtime-proxy tests pass, including a new real
  SSE inspection caller regression for repeated IDs across events,
  Unicode ordering and split stream boundaries.
- Two direct core tests pass for Unicode, empty strings, exact
  duplicate elimination, a 1,025-item ordered batch, malformed
  later UTF-8, ABI versions, count/capacity limits and unchanged
  output on rejection.
- Direct Mojo ABI test, strict no-fallback guard/self-test,
  Cargo formatting and official production-share non-regression
  validation pass.

Canonical broad share rises from 35.89055457119222% to
35.905425870090184% (131,588 Mojo / 234,897 Rust production LOC).
The project target of 75% is still outstanding. No source counting,
release floor, baseline or CI policy was changed. Remote CI green
was not required to publish this checkpoint.
