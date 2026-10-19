# Shared provider SSE JSON frame precedence moved to Mojo

The shared upstream SSE JSON reader now delegates its decision whether to
consume one valid joined JSON payload, consume several independently valid
SSE data lines, accumulate an incomplete fragment across events, or reject
oversized accumulation to the existing compiled SSE line kernel.

The Rust transport still reads bounded lines with BufRead, uses Serde as the
sole JSON syntax parser, retains buffering and observer callbacks, and
materializes events. The old Rust joined/separate/accumulation precedence
and limit-check decision tree were deleted. The Mojo plan prevents
overflow-safe accumulation from exceeding the configured budget, without
rejecting already complete JSON merely because that budget is small.

Direct ABI tests exercise all precedence combinations, accumulation
boundaries and overflow, bad flags, null output, and version mismatch.
Real provider reader tests cover joined multiline JSON, multiple complete
JSON objects in one SSE event, fragmented JSON across SSE events, blank
payloads, and oversize incomplete fragments failing closed. The full
Anthropic SSE and Gemini SSE caller suites confirm the shared reader
remains production compatible.

Only Mojo chooses framing precedence; Rust does not silently recalculate
it on ABI failure. The 75% project-wide share target remains outstanding.
