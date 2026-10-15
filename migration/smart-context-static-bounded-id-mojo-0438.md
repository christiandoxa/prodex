# Smart Context bounded static identifiers migrated to Mojo

`prodex_mojo_smart_context_static_bounded_id_v1` now owns Unicode
whitespace trimming, slash normalization, the 256-byte UTF-8-safe
identifier prefix, and long-ID marker serialization. The previous Rust
trim/replace/char-boundary/format implementation was removed.

The new ABI returns a special digest-required status only for IDs
exceeding the Mojo-owned byte limit. Rust computes the original
input's SHA-256 digest only on that explicit request, then calls the
same Mojo kernel to emit `<id-hash=...>`. No Rust semantic fallback,
duplicate cutoff algorithm, or unconditional short-ID hashing exists.
The adapter validates status codes, UTF-8 output, and maximum buffer
length before materializing the owned `String`.

## Local verification

- Required-Mojo direct bounded-ID ABI tests: 2 passed (Unicode cutoff,
  slash normalization, exact 256-byte limit, lazy digest callback and
  fail-closed invalid ABI/capacity).
- Real runtime-proxy bounded-ID caller test: passed.
- Full required-Mojo runtime-proxy library suite: 431 passed.
- Clippy `--all-targets -- -D warnings`: passed.
- Ownership, authority, no-fallback, and production-share guards: PASS.
- Broad production share: 33.37% Mojo, 111,623 LOC of 334,469 LOC.
  The 75% project target remains outstanding.
