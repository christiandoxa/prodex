# Runtime-doctor route-health marker matching and numeric fields in Mojo

The production runtime-doctor health report now uses its existing compiled
runtime-doctor marker owner for both marker scope and health score/expiry
parsing. The typed Rust adapter provides optional profile/route borrowed
strings, requested profile/route, and the selected u32/i64 numeric field;
Mojo decides whether missing or explicit `-` marker dimensions match and
parses signed or unsigned decimal fields with checked overflow bounds.

The duplicate Rust marker `is_none_or`/wildcard matching and `parse()` numeric
implementations have been deleted. Rust retains log-map lookup, typed output
materialization, event grouping and report DTO updates, with no Rust semantic
fallback. Invalid Mojo ABI results fail closed.

Compiled Mojo tests cover missing fields, Unicode profiles, empty values,
explicit wildcard markers, incompatible routes, leading plus/minus signs,
leading zeros, malformed digits and u32/i64 boundary overflow. Strict ABI
tests reject bad versions, modes and pointers without mutating outputs. The
real runtime-doctor caller checks wildcard score application, i64::MIN circuit
expiry, incorrect profile isolation, and invalid transport expiry preservation.

This checkpoint preserves the independently audited 75% project objective;
it does not claim to have reached it.
