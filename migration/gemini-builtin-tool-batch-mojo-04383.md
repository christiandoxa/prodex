# Gemini builtin tool array selection consolidated into Mojo batch ABI

The production Gemini request bridge now sends one Serde-normalized tools
array to the compiled provider-constraints owner and receives a typed
ordered list of builtin tool positions. The same batch plan is reused to
build canonical Gemini builtin-tool groups and to skip those positions
while building function declarations from a mixed tool array.

The earlier Rust per-item classifier created a synthetic request with one
tool, called the compiled SimpleRequest check for that single tool, and
then repeated this expensive JSON serialization and Mojo round trip for
every item. That entire Rust predicate was deleted. The ordinary Gemini
function-declaration caller likewise no longer calls a single-item
classifier from inside its iteration. Rust still performs serde
acquisition, applies Mojo-selected indexes, removes the unused function
property from builtin tool copies, and materializes Gemini JSON output.

The new versioned batch ABI reuses the existing exact
gemini_bridge_request_builtin_tool function in provider_constraints.mojo.
It validates the complete canonical tools-array JSON shape and counts all
entries before touching caller output. It rejects insufficient capacity
without partial output writes, then emits one builtin flag per original
tool index. This retains all supported tool aliases, property-presence
overrides and case-sensitivity; it does not introduce any classifier
implemented in Rust. Invalid ABI results fail explicitly.

Direct compiled ABI tests cover function and builtin tool mixes, several
alias families, canonical ordering, unsupported tools, empty arrays,
out-of-range expected counts, malformed JSON, missing output pointers,
insufficient capacity and version mismatch. A real provider request
regression proves mixed builtins and four Unicode/non-Unicode function
declarations preserve original function order while builtins retain
their canonical grouped order.

All 474 provider-core and 697 Mojo-core tests pass. The upstream Codex
compatibility baseline and no-fallback guard also pass. The source
guard rejects restoration of the per-tool Rust predicate and repeated
single-item SimpleRequest calls.

This is a production-reachable, performance-relevant migration, not
completion of the separate 75% Mojo production source-share objective.
