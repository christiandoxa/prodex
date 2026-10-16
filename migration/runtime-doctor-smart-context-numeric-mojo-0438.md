# Runtime doctor Smart Context numeric and counter ownership in Mojo

The compiled runtime-doctor marker owner now computes Smart Context byte
savings, token estimates, diagnostic counter aggregation and the action of
rewritten, pass-through and fallback decisions through versioned strict ABIs.
Explicit saved-byte fields take priority; absent values derive only when both
before and after are present, with saturating subtraction and ceil(byte / 4).
Accumulated totals use saturating UInt64 arithmetic, preserving overflow
safety and event-order-independent counters.

The old Rust saved-byte fallback, token-division algorithm, seven saturating
counter additions and rewrite/fallback classification branches have been
deleted. Rust retains JSON/log field acquisition, trusted redaction,
summary DTO fields and keyed count materialization; no Rust numeric fallback
or duplicate migrated decision remains.

The Smart Context numeric parser now reuses the existing production-reachable
Mojo runtime-doctor unsigned field parser instead of Rust `parse::<usize>`.
The original decimal grammar, `usize::MAX` cap and invalid-input handling stay
owned by Mojo.

Strict ABI tests cover supplied and derived amounts, missing fields, underflow,
UInt64 maxima, overflow saturation, malformed ABI flags and decision classes.
Runtime-doctor caller tests cover the same boundaries through parsed log lines.
The broad project target of 75% Mojo production share is still outstanding.
