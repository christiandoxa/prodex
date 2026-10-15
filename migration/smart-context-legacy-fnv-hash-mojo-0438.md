# Smart Context legacy FNV fingerprint hash migrated to Mojo

The production Smart Context compatibility path now calls
`prodex_mojo_smart_context_fnv_hash_text_v1` from the existing compiled
`smart_context_normalization.mojo` owner. Mojo computes the wrapping
64-bit FNV-1a hash of caller-provided bytes, serializes exactly sixteen
lowercase hexadecimal digits, and optionally prepends the stable
`scv:` prefix for normalized command-output artifact hashes.

Rust's original FNV hashing loop, constant seeds, wrapping arithmetic,
numeric hex formatting and `scv:` composition were removed. Rust now
supplies borrowed UTF-8/normalized output bytes to the Mojo operation,
validates its ABI, length and UTF-8 result, and retains unrelated
cryptographic SHA-256 for `sc2:` hashes. The previous `sc:` legacy
comparison is preserved for already-persisted fingerprints. No Rust
algorithm fallback or duplicate production owner remains.

## Local verification

- Required-Mojo direct FNV ABI tests: 2 passed; golden byte vectors
  cover empty data, a, foobar, NUL, Unicode and both text modes; invalid
  versions, modes, pointers and output capacity are rejected.
- Full required-Mojo runtime-proxy library suite: 433 passed, including
  added integration checks for legacy `sc:` and normalized `scv:` hashes.
- Clippy `--all-targets -- -D warnings`: PASS.
- Mojo ownership, authority, no-fallback and production-share guards:
  PASS.
- Broad production share at checkpoint: 33.39% Mojo
  (111,760 / 334,679 LOC); the 75% target remains outstanding.
