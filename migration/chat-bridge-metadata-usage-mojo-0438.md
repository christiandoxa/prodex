# Chat-compatible bridge metadata and usage in Mojo

The existing compiled OpenAI chat-response owner now exposes two additional
ABI operations: metadata projection (3) and usage mapping (4). The bridge
keeps its separate semantics from the ordinary Responses translator.

Mojo owns optional-field type/emptiness tests, first-choice selection,
annotations, finish reasons, provider metadata, numeric defaults, saturated
totals and cache/reasoning details. Rust mapping loops and filter helpers
were removed; only borrowed Serde acquisition and result decoding remain.
The existing shared Mojo saturating-add routine is reused.

Required-Mojo verification:
- Full provider-core library: 451 tests passed.
- Direct OpenAI chat response ABI integration suite: 6 passed.
- 4,096 metadata plus 4,096 usage combinations match the removed Rust code.
  Temporary reference implementations were removed after the tests.
- Provider-core all-target Clippy with warnings denied: passed.
- Ownership, authority, no-fallback, production-share and size guards: passed.

The isolated checkpoint measures 33.34% Mojo
(111,381 / 334,098 production LOC). The 75% project target is not yet met;
no release metadata, frozen baseline or counting rule was changed.
