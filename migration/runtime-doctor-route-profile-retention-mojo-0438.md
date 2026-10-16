# Runtime doctor route/profile event acceptance and retention in Mojo

The compiled runtime-doctor renderer now owns the acceptance and FIFO
eviction policy for diagnostic route/profile events. Events require a known
route-action marker and at least one nonempty profile or route. Twenty
events are retained, and the oldest is removed before/after appending
the twenty-first while preserving order and exact String contents.

The previous Rust early-exit decision, MAX_EVENTS constant and post-push
length comparison were deleted. Rust still stores structured event data
and gathers profile and route fields; compiled Mojo exclusively chooses
whether to accept and whether the oldest event should be evicted.

The strict Mojo unit tests cover all marker/presence combinations and
the 0, 19, 20, 21, and maximal count boundaries. Runtime-doctor caller
tests verify skipped invalid rows, Unicode profiles, 25 arrivals and
route-only diagnostic rows. There is no semantic Rust fallback.

This incremental checkpoint does not claim that the broad Mojo production
share has reached the separate target of 75%.
