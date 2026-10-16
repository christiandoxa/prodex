# HTTP response Connection-token filtering migrated to Mojo

The compiled response_forwarding.mojo domain now accepts a borrowed list of
all upstream Connection header values and makes one authoritative decision
for each response header. The existing Mojo hop-by-hop and comma-delimited,
case-insensitive token matcher is reused within the same kernel.

Rust's repeated any-match loop and second per-header filtering policy were
deleted. The host still owns Hyper/HTTP headers, collects Connection values
using the existing Mojo classifier, and copies the ordered forwardable
binary or text headers into output DTOs. No Rust token-matching fallback
or duplicate skip decision remains.

Direct compiled Mojo ABI tests cover each hop-by-hop name, mixed token
case and whitespace, multiple Connection values, invalid UTF-8, and invalid
ABI/pointer input. Real proxy callers validate multi-Connection header
removal without dropping unrelated binary values and turn-state metadata.

This incremental migration retains the exact upstream transparency and
committed-response semantics. It does not claim completion of the
separate 75% broad production Mojo-share goal.
