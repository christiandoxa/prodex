# Diagnostic redaction snippets in compiled Mojo

The production redaction adapter now delegates its post-redaction diagnostic
snippet formatter to the existing compiled `redaction.mojo` owner. The Mojo
kernel collapses Unicode whitespace into one ASCII space, emits a `-` for
empty or whitespace-only input, clips by UTF-8 scalar count, and appends the
exact ellipsis suffix when the normalized text exceeds its character budget.

The previous Rust `split_whitespace`, `chars().take`, character count,
and ellipsis formatter were removed. Rust keeps input acquisition, JSON
redaction, the OsString/secret-handling boundary, and checked UTF-8 output
construction. The versioned Mojo ABI validates pointers, UTF-8 input, and
output capacity before writing and fails closed on invalid input. There is
no Rust fallback reimplementing the migrated formatter.

Direct Mojo-linked tests cover Unicode whitespace and combining marks,
empty and tiny widths, exact and truncated snippets, invalid ABI versions,
bad pointers, and insufficient buffers. The real `prodex-redaction` caller
test verifies Unicode normalization and the JSON body diagnostic path.
The no-fallback guard enforces the production Mojo call and rejects
reintroduction of the Rust snippet normalization algorithm.

This is a production-reachable checkpoint; the 75% long-term target remains
incomplete.
