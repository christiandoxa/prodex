# Chat tool namespace splitting moved to compiled Mojo

The production chat-compatible tool bridge now delegates decoding flat
namespace/tool labels to the existing compiled chat-tool name domain. The same
Mojo module already owns namespace flattening; the new versioned C ABI returns
validated borrowed UTF-8 ranges for the namespace and tool name. This is used
by Anthropic Messages and DeepSeek stream mappings as well as host tool
adapters.

The compiled algorithm gives priority to the final double-hyphen separator
when both Unicode-trimmed sides are nonblank. Otherwise exact `mcp__` names
use their final double-underscore separator when the tool portion is
nonblank. Invalid or unqualified names remain intact, preserving original
bytes, whitespace and Unicode. Rust no longer performs its previous
`rsplit_once`, prefix parsing, or `rfind` fallback. It only validates the
returned UTF-8 spans and materializes strings for the existing Rust API.

Direct strict ABI tests cover invalid versions/pointers/buffer capacities,
malformed UTF-8, Unicode and separator precedence. All 465 provider-core
library tests and all 13 relevant application provider-tool tests pass.
The no-fallback guard requires the compiled decoder, real caller coverage,
and absence of the former Rust name parser.

The independent long-term 75% Mojo production-share target remains unmet.
