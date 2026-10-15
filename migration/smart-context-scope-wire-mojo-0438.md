# Smart Context scope framing and digest rendering migrated to Mojo

The stable ContextScopeId wire format now uses two versioned compiled
Mojo operations in the existing Smart Context normalization domain owner:

- `prodex_mojo_smart_context_scope_preimage_v1` writes the original
  `prodex-smart-context-scope-v1\0` domain separator followed by the
  five ordered UTF-8 context fields, each prefixed by exactly eight
  big-endian length bytes.
- `prodex_mojo_smart_context_scope_digest_text_v1` renders a 32-byte
  host SHA-256 digest as `scscope1:` and 64 lowercase hex digits.

The Rust `ContextScopeId::new` preimage-construction loop, byte-length
encoding, `hex_digest` implementation and `format!` for scope text
were deleted. The Rust host retains its existing SHA-256 implementation,
owned strings and DTOs, while Mojo owns wire framing and serialization.
Existing `ContextScopeId::parse` and `path_component` contracts are
unchanged. No migrated algorithm has a Rust production fallback.

## Local validation

- Required-Mojo direct scope ABI tests: 3 passed, including UTF-8
  field length boundaries, binary preimage equality, digest rendering,
  bad ABI, invalid item count and insufficient output capacity.
- Required-Mojo production scope regression: passed, including a
  hard-coded historical SHA-256 scope digest to prove persistence
  compatibility and isolation across tenant/profile/provider/workspace/session.
- Full required-Mojo runtime-proxy library suite: 433 passed.
- Clippy `--all-targets -- -D warnings`: PASS.
- Mojo ownership, authority, no-fallback, production-share guards: PASS.
- Official broad production share: 33.41% Mojo
  (111,870 Mojo LOC of 334,852 total LOC). Project target 75%
  remains unmet; this is a production-reachable incremental checkpoint.
