# DeepSeek/Kiro input-item issue messages migrated into Mojo

The compiled DeepSeek input-item validator already returns the authoritative
issue tag and optional detail. The same compiled domain now renders the exact
provider-prefixed diagnostic for supported DeepSeek and Kiro Responses input
items. This covers missing item fields, unsupported role/content types, tool
call arguments and output requirements, local shell commands, and the generic
unknown-tag message, including all historical multiple-tag aliases.

Rust's tag-to-diagnostic `match` and its provider-specific reason strings were
deleted. Rust retains serde acquisition, host JSON ownership, typed Mojo ABI
transport, and Result propagation. Even a non-object input obtains its message
from the compiled Mojo owner, with no Rust fallback diagnostic table.

The versioned ABI checks issue tags, pointers, UTF-8, lengths and output
capacity before writing. All known issue tags, multiple provider labels,
unknown tags, Unicode detail, invalid inputs and no-partial-write behavior
have dedicated ABI regressions. The real DeepSeek and Kiro caller test checks
stable errors. The complete 464-test prodex-provider-core library suite passes.
The no-fallback guard requires Mojo delegation and rejects restoring the Rust
issue-message switch.

This checkpoint does not claim the independent 75% production-share project
target has been achieved.
