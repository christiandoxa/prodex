# DeepSeek streamed tool-kind and fallback identifier authority migrated to Mojo

The existing compiled DeepSeek stream policy owner now decides which
tool-call item shape is required for an exact flat tool name: tool_search,
apply_patch, or a generic function-call. The prior duplicated Rust
literal comparison branches in the added/completed event shaping paths
were deleted. The host retains serde_json and typed event materialization.

A second compiled operation emits legacy streamed fallback identifiers:
call_{provider}_{request}_{index},
msg_{provider}_{request}, and resp_{provider}_{request}.
The three former Rust format! implementations were deleted, without
silent fallback. Mojo owns prefixes, decimal formatting, and UTF-8 bytes.

ABI tests cover invalid modes, exact names, Unicode, u64::MAX,
usize::MAX, and capacity faults. The real DeepSeek provider regression
asserts unchanged JSON and IDs for tool search, patch, and namespaced
function tools. This migration is production reachable, though the
separate 75% broad project target remains outstanding.
