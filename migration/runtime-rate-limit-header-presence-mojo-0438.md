# Rate-limit header presence policy in compiled Mojo

The production runtime proxy now selects the first nonblank valid-UTF-8
`x-codex-rate-limit-reached-type` header using the same compiled Mojo Unicode
trim owner already used to classify its value. The Rust adapter continues to
acquire HTTP headers, compare names through Mojo, and discard values that are
not valid UTF-8. Mojo determines whether each candidate contains meaningful
text. The first nonblank matching header remains authoritative even if its
value is an unknown future classification.

The Rust `map(str::trim).find(|value| !value.is_empty())` selection predicate
was deleted. The previous silent `rate_limit_header_class(value).unwrap_or(0)`
is now a required, fail-closed Mojo call. There is no Rust fallback or
duplicate trimming/classification behavior for the migrated policy.

Direct compiled Mojo ABI tests cover empty input, Unicode whitespace,
nonempty values, malformed UTF-8, invalid pointers/lengths, and ABI version.
The runtime proxy caller regression checks duplicate-header precedence and
recognized quota values after Unicode padding, while the existing status-429,
invalid-header and committed-stream tests still pass. The no-fallback guard
requires production Mojo delegation and refuses the old Rust predicates.

This checkpoint does not claim the separate 75% project production-share
target has been reached.
