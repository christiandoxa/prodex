# Smart Context model prefix scan

The oversized request-body model prefix scan now runs in
`mojo/prodex_core/smart_context_model_registry.mojo` through
`prodex_smart_context_model_prefix_scan_v1`. Mojo owns the 4 KiB bound, first
`"model"` occurrence, Unicode whitespace handling, quoted-value escape rules,
control rejection, and output-buffer validation. Rust keeps serde JSON parsing
and DTO normalization at the host boundary.

Compiled Mojo ABI tests and the runtime-proxy caller regression cover the
bounded extraction contract. No Rust fallback or duplicate prefix scanner
remains.
