# Smart Context token and calibration history dedup migrated to Mojo

The production Smart Context token-calibration persistence merge now uses
the compiled Smart Context history owner not only for rewrite-safety
stable top-K timestamp retention, but also for append-only deduplication
and tail retention of token usage and token-calibration observations.

The old Rust generic contains()-based unique-append loop and the
subsequent drain()-based truncation helper were deleted. Rust still
acquires and serializes strongly typed persisted records through Serde,
then materializes the Mojo-chosen original positions. Serde's stable
field and object ordering on these particular integer and optional
string record shapes gives a canonical byte representation of their
previous structural equality contract. Existing duplicates are kept
until tail truncation; only incoming entries identical to any existing
or earlier accepted incoming entry are excluded.

The compiled Mojo owner validates view-array lengths and pointers
before writes, builds the accepted-index sequence, checks incoming
serialized equality over borrowed bytes, and returns the final ordered
K positions. Rust validates bounds and increasing unique positions,
and fails explicitly on ABI errors without any Rust dedup/truncation
fallback. The same owner handles both history limits, 8 and 16,
without changing the separate rewrite-safety TTL policy.

Direct ABI tests cover unchanged existing duplicates, duplicate
incoming entries, source representation with Unicode, zero and large
limits, malformed view pointers, bad lengths, and no partial output
writes. Test-only Rust equality/original-tail oracles verify the
stable contract and never run in production. A real persisted merge
caller test covers Unicode calibration buckets, duplicates in both
inputs, case-sensitive routes, and unchanged ordering.

All 114 sequential Smart Context app tests and 695 Mojo-core tests
pass, as does the upstream compatibility baseline and no-fallback
guard. The separate 75% Mojo production-source target remains
outstanding.
