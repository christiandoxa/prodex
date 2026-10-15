# Runtime Retry-After text migration checkpoint

The compiled `rich_fallback.mojo` owner parses the first case-insensitive
`try again in` message marker, Unicode whitespace, duration tokens and
units through `prodex_mojo_rich_retry_after_text_v1` (rich ABI 6). Header
values use the same existing numeric duration policy and 300-second cap.
Rust retains header iteration, UTF-8 borrowing and Duration projection;
the prior Rust token scanner and suffix parser are removed.

Required-Mojo validation before interruption: 2 direct tests, 9 Retry-After
caller tests, and 430 runtime-proxy library tests passed. Clippy for the
owning crate and ownership/authority/no-fallback/share guards passed.
The focused ABI tests are kept in a private test module to preserve the
repository size budget. This checkpoint does not claim the 75% target.
