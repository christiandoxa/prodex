# Smart Context persisted rewrite-safety retention moved to compiled Mojo

The production Smart Context token-calibration persistence merge now
delegates the rewrite-safety history retention algorithm to a standalone,
versioned Mojo domain owner. The Rust host still deserializes existing
JSON, acquires the current Unix time, joins persisted and incoming data,
and materializes the selected records. Mojo alone decides whether each
record remains fresh, its stable timestamp order, and which latest K
records survive retention.

The previous Rust rewrite-safety expiry retain predicate, full vector
sort_by_key, and final truncate-tail selection were deleted from the
merge path. They were replaced with a bounded stable top-K algorithm.
The Mojo kernel consumes borrowed timestamps and keeps only K positions
sorted by timestamp and original input position, requiring O(n*K) time
and O(K) selection space. For the current four-record retention limit,
this is linear in the incoming history length instead of sorting the
full persisted collection. Equal timestamp records prefer later
original positions, preserving Rust's stable-sort-then-tail semantics.
Timestamps of zero and timestamps in the future are always fresh,
matching the historical saturating subtraction. Rust's separate token
usage and token calibration unique history merge logic is unchanged;
there is no fallback to the old rewrite-safety selection algorithm.

The strict ABI validates record counts, pointers, output capacity,
version, and selected indexes before accepting results. Direct tests
verify expiration, future times, timestamp ties, zero/large TTLs,
empty collections, zero and oversized selection limits, and malformed
ABI inputs without writes. A test-only stable Rust sort oracle covers
hundreds of combinations; it is not linked as a production fallback.

Real persistence merge caller tests verify that expired records are
pruned, duplicate timestamp precedence remains stable, the four-item
limit holds, and unrelated token-usage/calibration histories retain
their established exact deduplication contract.

All 692 Mojo-core tests and 113 sequential Smart Context app tests pass.
One unrelated Smart Context shortcut test had an intermittent failure
when originally run in parallel, then passed three consecutive
isolated runs and in the complete sequential suite; its assertion
was not modified or weakened. The no-fallback guard prevents restored
Rust rewrite-safety expiration, sort, or history tail selection.

This is a real production-linked checkpoint. The separate 75% Mojo
production source-share target remains unmet.
