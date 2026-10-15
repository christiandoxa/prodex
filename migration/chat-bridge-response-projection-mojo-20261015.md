# Chat bridge response projection in Mojo

The buffered chat-compatible response bridge now sends its validated response context to
`prodex_mojo_openai_chat_response_v1(operation=5)`. Mojo owns the Responses envelope,
assistant text item, tool output ordering and shapes, flat namespace projection, and
partial output on a later tool error. Rust retains Serde acquisition, clock and fallback
ID generation, exact argument diagnostics, RTK/apply-patch adapters, and typed decoding.

The caller tests exercise compiled Mojo with fallback IDs, Unicode text and namespaced
tools, `tool_search`, `apply_patch`, and partial malformed-tool output. There is no Rust
projection fallback.
