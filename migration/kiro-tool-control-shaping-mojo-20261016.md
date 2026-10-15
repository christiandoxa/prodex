# Kiro tool-control shaping in Mojo

Kiro request rewriting now calls the compiled Mojo `prodex_mojo_kiro_external_tool_controls_v1` kernel to remove ACP-owned external tool declarations and sub-agent controls. Rust keeps JSON acquisition, canonicalization, the ABI boundary, and request errors; the Mojo kernel preserves surviving field order and UTF-8 data without a production fallback.
