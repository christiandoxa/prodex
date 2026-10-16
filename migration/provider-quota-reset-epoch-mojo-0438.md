# Provider quota numeric and relative reset timestamps in compiled Mojo

The production OpenAI-compatible quota-header adapter now delegates numeric
reset interpretation and overflow-safe relative timestamp computation to the
compiled provider-constraints Mojo owner. Its versioned ABI owns the existing
precedence rule: numeric values above 10_000_000_000 are millisecond epochs,
values strictly newer than the current host time minus 60 seconds are
absolute-second epochs, and all remaining numeric values are nonnegative
relative seconds. The companion duration operation performs saturated
addition after the caller parses provider duration syntax.

Rust still owns HTTP header acquisition, UTF-8 and numeric decoding,
RFC3339 conversion, duration grammar decoding, and wall-clock acquisition.
The old Rust numeric timestamp threshold, `saturating_sub`/`saturating_add`
selection and relative-duration addition were deleted. No Rust fallback
recomputes Mojo-owned epoch decisions; malformed ABI output fails closed.

Direct ABI tests cover threshold boundaries, exact seconds versus
milliseconds, zero/negative relative inputs, i64 minimum and maximum,
overflow saturation, invalid operation codes, versions and pointers. The real
runtime caller verifies absolute-millisecond projection and 60-second
relative timestamps in emitted Codex rate-limit headers. Source guards
reject restoring the deleted numeric reset planning code.

This migration remains short of the independent long-term 75% Mojo production
ownership target.
