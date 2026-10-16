# SSE data payload framing and UTF-8 BOM normalization in Mojo

The existing compiled SSE domain now owns
prodex_runtime_sse_payload_join_v1: it joins the contents of all data:
lines using one LF between adjacent lines, then removes every leading
UTF-8 BOM sequence before handing the result to the host serde_json parser.

The previous production Rust join and trim_start_matches U+FEFF logic
was deleted, without a semantic fallback. Rust retains ownership of
Vec of String, the trusted borrowed pointer/length ABI views,
allocation/error validation, and JSON decoding.

The ABI preflights pointers, all source lengths and output capacity before
writing. Golden compiled Mojo tests cover empty data, repeated leading BOMs,
embedded BOMs after an LF, Unicode strings and hundreds of lines. A production
runtime-proxy regression checks equivalent JSON and event IDs across multiple
CRLF SSE lines and varying transport fragment boundaries. No retry/commit
safety semantics are relaxed.

This is one production-reachable checkpoint; the separate broad 75% Mojo
project target remains outstanding.
