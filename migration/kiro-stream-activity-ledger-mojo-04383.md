# Kiro streaming tool-activity ledger migrated to compiled Mojo

The live Kiro ACP server-to-client streaming adapter no longer owns a
separate Rust tool-activity ledger policy. It now uses the very same
versioned compiled Mojo owner as the completed ACP turn-state collector.
The host continues to store the streaming BTreeMap of labels, count
emitted activities, and materialize user-visible progress JSON.

Previously the streaming path implemented another independent Rust
decision tree to stop after the first 127 activities, emit one
terminal truncation marker, reject caching tool-call IDs over 256
UTF-8 bytes, inherit prior tool names and kinds, and choose
whether to update label metadata. That entire policy tree was deleted.
Mojo returns one typed action (ignore, append, or append-truncated)
and exact metadata inheritance/recording flags. The Rust caller only
applies the plan and treats invalid Mojo ABI output as an explicit
failure, never falling back to the old decisions or to a Rust default
activity name.

The existing compiled ABI tests cover malformed flags, UTF-8, bounds,
version mismatches, the activity cap and label inheritance. Two new
live streaming caller regressions verify inherited title/kind, 128
bounded events, exactly one truncation marker, no events after
truncation, and no label caching or inheritance for oversized Unicode
IDs. All 24 local Kiro rewrite tests, 32 ACP runtime tests and 3
direct Kiro activity ABI tests pass.

The no-fallback guard now protects both the buffered ACP turn-state
collector and the streaming activity ledger against reintroduction of
Rust cap, ID-size or label inheritance decisions. All other Kiro
session, SSE, affinity and continuation logic remains unchanged.

The independent 75% Mojo production-source share target remains unmet.
