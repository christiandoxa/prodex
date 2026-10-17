# Runtime compatibility diagnostic rendering owned by Mojo

The compiled runtime compatibility-surface domain owns the user-facing/logging
projection of its existing tool and continuation flags: exact bit-order labels,
plus-sign joins, and empty `none` markers. It now also owns Unicode whitespace
trimming of User-Agent values and a one-underscore-per-Unicode-scalar log-safe
projection for arbitrary User-Agent strings, with an empty `-` marker.

The old Rust `compatibility_flag_string` and `runtime_capability_log_safe_value`
character loop were deleted. The Rust runtime proxy only parses JSON, obtains
HTTP metadata, and carries Mojo-produced strings to the logging host. Static
warnings still map known Mojo-owned warning flags to borrowed string constants;
this is not an alternate classifier or text-rendering implementation.

The versioned C ABI rejects invalid operation/flags/UTF-8/pointers, bounds the
output buffer and reports errors rather than recomputing text in Rust. ABI
tests check byte capacity, invalid input, Unicode combining marks, zero-length
strings, and ordered flag combinations. Real production caller tests preserve
compatibility-surface route detection, tool order, continuation order, and
Unicode User-Agent behavior. The Mojo no-fallback guard rejects restored
Rust formatters and requires the production compiled ABI and its coverage.

This is a production-reachable ownership checkpoint; the separate 75% project
target remains incomplete.
