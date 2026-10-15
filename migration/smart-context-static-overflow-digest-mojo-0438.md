# Smart Context static fingerprint overflow migrated to compiled Mojo

Two versioned Mojo operations now own the deterministic overflow
fingerprint behavior in `smart_context_normalization.mojo`:

- `prodex_mojo_smart_context_static_overflow_preimage_v1` serializes
  evicted-item ID bytes, NUL, the precomputed content hash, NUL, and
  the item byte count in host-width little-endian encoding.
- `prodex_mojo_smart_context_static_overflow_accumulate_v1` folds each
  resulting 32-byte SHA-256 into the aggregate with per-byte
  modulo-256 addition.

Previously both framing and wrapping addition were performed in Rust
inside `smart_context_add_static_context_overflow_digest`. The Rust
implementation was removed. Rust continues to run SHA-256 as a host
cryptographic primitive between the two Mojo kernels, and owns the
BTreeMap-independent item selection DTOs. It does not reimplement
either Mojo decision or silently invoke a Rust fallback.

## Local validation

- Required-Mojo direct overflow tests: 3 passed, including a golden
  byte-for-byte binary preimage, Unicode IDs, `usize::MAX`,
  platform-width length framing, wrapping carry and invalid ABI buffers.
- Real 129-plus static items fingerprint regression and input-order
  invariance: passed.
- Full required-Mojo runtime-proxy library suite: 436 passed.
- Clippy `--all-targets -- -D warnings`: passed.
- Mojo ownership, authority, no-fallback and broad production-share
  guards: PASS.
- Official production share at checkpoint: 33.46% Mojo
  (112,227 Mojo LOC of 335,438 total LOC).
  The 75% project target remains pending.
