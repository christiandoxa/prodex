# DeepSeek request rejection diagnostic mapping in compiled Mojo

The production DeepSeek Responses-to-Chat request transformer already returns
one authoritative issue code through the compiled Mojo request-tree kernel.
The same Mojo module now emits the exact stable diagnostic text for all 21
issue codes, including the dynamic response-format type and reasoning-field
detail. The previous Rust match over 21 rejection codes was deleted without
an alternative Rust diagnostic mapping.

The host only validates the issue tag, carries the optional detail supplied by
the compiled planner, and materializes the ABI-checked UTF-8 rejection string
as the provider transform result. The new versioned C ABI validates tag,
length, UTF-8, pointers, and output capacity; malformed input fails closed.
The error text and unsupported-field precedence remain byte-compatible with
the established DeepSeek API behavior.

Direct compiled-Mojo tests assert every issue's exact message, Unicode details,
unknown tags, invalid ABI shape and tiny capacity. All 28 request-transform
caller tests and all 463 provider-core library tests pass. The no-fallback
guard now requires the Mojo issue-message ABI and rejects restored Rust
issue-to-message match branches.

This is a production-reachable ownership checkpoint, not a waiver for the
independent long-term 75% Mojo project target.
