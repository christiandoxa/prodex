# Codex-compatible quota header identifier normalization migrated to Mojo

The production provider rate-limit adapter now delegates allowed-character
projection, separator collapsing, and final dash-delimited quota header ID
rendering to the compiled provider-constraints Mojo owner. Its previous Rust
loop over ASCII alphanumeric characters, dash/underscore rewriting, duplicate
separator suppression, and trailing/leading separator trimming were removed
without an alternate Rust semantic implementation.

The narrow Rust adapter retains only contextual Unicode lowercasing through
the host standard library, after trimming the original text. This retains
Rust's exact multi-scalar and context-sensitive Unicode behavior (including
Kelvin sign and capital dotted I) where the current Mojo stdlib does not
expose an equivalent full Unicode lowercase operation. Mojo receives the
lowered UTF-8 bytes and solely decides which ASCII characters survive into
the header ID and how hyphens are emitted. This is not a Rust fallback for
the moved ID normalization algorithm.

The versioned ABI rejects malformed pointers, UTF-8 and insufficient capacity
before modifying output. Direct Mojo-linked tests check repeated separators,
leading/trailing separators, Unicode symbols, casefold expansion, UTF-8 and
capacity. The production caller test verifies real OpenAI-compatible quota
header names with Unicode source prefixes; the no-fallback guard disallows
restoring the previous Rust sanitizer.

The independent long-term 75% Mojo share goal remains unfinished.
