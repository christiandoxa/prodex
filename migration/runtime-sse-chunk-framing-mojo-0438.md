# SSE chunk newline framing moved to compiled Mojo

The existing compiled sse_line.mojo owner now exposes a bounded batch
scanner for LF byte offsets. Both normal Responses SSE streaming and the
pre-commit SSE inspection path consume this same Mojo plan. Rust still
owns stream buffers, appends validated spans, parses JSON with serde_json
and dispatches callbacks, but no longer independently loops over bytes
to identify SSE line boundaries. There is no fallback byte scanner.

The ABI returns a byte-consumption count and up to 256 monotonically
increasing LF offsets per call, preventing unbounded offset storage for
large incoming chunks. Rust repeatedly requests the next bounded plan
for chunks with more than 256 line breaks. It preserves incomplete lines
across transport boundaries and hands CRLF/UTF-8 contents to the existing
Mojo SSE line and event classifiers without changing commit boundaries.

Verification includes Mojo ABI validation and real parser regression
against 600-plus-line SSE bodies with different transport fragment widths.
This is a production-reachable migration checkpoint, not a claim that
the long-term 75% Mojo project target has been reached.
