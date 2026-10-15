# Smart Context scope parsing migrated to Mojo

The remaining production identifier parser has been migrated to the
compiled `prodex_mojo_smart_context_scope_parse_v1` operation. Mojo now
owns strict `scscope1:` prefix recognition, exactly 64 ASCII hexadecimal
digest characters, lowercasing of uppercase hex, and absence for invalid
or incomplete identifiers. Rust's `strip_prefix`, ASCII character
validation, digest length comparison and `to_ascii_lowercase` policy
were deleted from production.

Rust retains the ContextScopeId type, typed Mojo ABI validation, checked
UTF-8 materialization and the unrelated accessor for the persisted
path component. The new parser uses the same versioned Smart Context
Mojo domain as the preimage and digest serializer. No Rust semantic
fallback remains.

## Local verification

- Strict Mojo direct parser tests: 2 passed (valid lowercase/uppercase,
  invalid hex, prefix case, short identifier and invalid ABI).
- Real ContextScopeId caller regression: passed; persisted SHA-256 digest
  remains byte-identical and the five field boundaries remain distinct.
- Full required-Mojo runtime-proxy library suite: 433 passed.
- Clippy `--all-targets -- -D warnings`: passed.
- Mojo ownership, authority, no-fallback and production-share checks:
  all PASS.
- Broad production share at checkpoint: 33.42% Mojo
  (111,928 Mojo LOC / 334,945 total). The 75% project target
  remains outstanding; this checkpoint does not claim completion.
