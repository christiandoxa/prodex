# MCP stdio first-line classification in Mojo

MCP stdio framing now delegates the first non-whitespace byte classification to
the required `prodex_mcp_first_line_is_header_v1` operation. Rust keeps buffered
stdin reads, UTF-8 and JSON parsing, framing budgets, size limits, and writes;
the former Rust classifier is deleted.

Compiled Mojo byte-matrix and ABI rejection tests, plus MCP caller regressions,
preserve malformed-input handling and bounded JSONL/header framing.
