# Smart Context bounded static content migrated to Mojo

The production static-context fingerprint path now uses the compiled
`prodex_mojo_smart_context_static_bounded_text_v1` operation. Mojo
owns the 256 KiB boundary, Unicode-safe prefix selection, static
canonicalization and the `psc static fingerprint item truncated`
diagnostic suffix. The canonicalization kernel is shared with the
previous full-text Mojo operation rather than duplicated.

Rust's former `len() <= MAX` branching, `is_char_boundary` loop,
prefix slice and `format!` suffix have been removed. Rust computes the
SHA-256 of the **original** text only after Mojo explicitly returns a
digest-needed ABI status, then calls Mojo again to write the bounded
output. Rust retains owned input strings, bounded allocation, verified
UTF-8 output and the existing fingerprint DTOs; there is no Rust
semantic fallback or duplicate production truncation algorithm.

## Checkpoint verification

- Required-Mojo direct ABI tests: 2 passed, covering exactly 256 KiB,
  multibyte overlimit input, lazily requested original-text SHA-256 and
  fail-closed invalid ABI.
- Required-Mojo production caller regression: passed.
- Full required-Mojo runtime-proxy library suite: 432 passed.
- Clippy for all runtime-proxy targets with `-D warnings`: PASS.
- Mojo ownership, authority, no-fallback and production share checks:
  PASS.
- Broad production Mojo share: 33.39% (111,706 / 334,596 LOC).
  The 75% project target is still not met; this is an incremental,
  production-reachable checkpoint, not an end-state claim.
