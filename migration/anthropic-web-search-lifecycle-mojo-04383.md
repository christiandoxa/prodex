# Anthropic web-search lifecycle decisions moved to compiled Mojo

The native Anthropic Messages SSE adapter now requests a versioned Mojo
plan before applying server-tool web-search results or finalizing the search
items left pending at message stop. The compiled kernel matches exact tool-use
IDs by UTF-8 bytes in the host's ordered BTreeMap snapshot, preserves the
first index when duplicate IDs exist, and chooses which items may emit a
terminal completion. Repeated results still update their stored sources
after normal validation but do not emit duplicate completion events.

The original Rust BTreeMap search-by-ID closure, pending-completion iteration,
and duplicate-result completion-state gate were deleted. Rust retains
BTreeMap mutation, Serde parsing, native provider web-search source decoding,
and event materialization; these are host storage/transport boundaries rather
than a fallback to an independently implemented selector.

The compiled ABI validates all borrowed UTF-8 identifiers and state flags
before writing to caller-owned buffers. Rust verifies returned positions,
flags and absence of duplicate indices before applying the plan. Direct tests
cover first-match precedence, Unicode, case sensitivity, missing targets,
completed duplicates, terminal sweep order and malformed ABI shapes.

Real native SSE caller tests cover multiple concurrent searches, repeated
results, ambiguous IDs, unknown result IDs, and stable pending-item order.
No committed stream or unrelated runtime affinity semantics are modified.
This production-reachable checkpoint does not complete the 75% Mojo target.
