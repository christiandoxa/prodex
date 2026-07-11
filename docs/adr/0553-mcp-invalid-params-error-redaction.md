# ADR 0553: Stabilize MCP JSON-RPC errors

## Status

Accepted

## Context

The `prodex-inspect` and `prodex-memory` MCP stdio endpoints returned full
`anyhow` error chains for failed `tools/call` requests, echoed unknown method
names in method-not-found errors, and `prodex-memory` returned unknown tool
names as successful text content. Those values can include local paths, Mem0
upstream response bodies, request argument values, or other diagnostics that do
not belong in client-visible JSON-RPC errors.

## Decision

MCP `tools/call` execution failures now return stable JSON-RPC `-32602`
messages:

- `invalid inspect tool parameters`
- `invalid memory tool parameters`

Unknown methods now return stable JSON-RPC `-32601` `method not found`
messages instead of echoing the method string.

Unknown `prodex-memory` tool names now use the same stable invalid-params error
instead of a successful text response that echoes the tool name.

The JSON-RPC code and request ID behavior are unchanged.

## Consequences

- MCP clients still receive a standards-compatible invalid-params error.
- Tool execution diagnostics, unknown tool names, and unknown method values are
  no longer exposed through MCP responses.
- Regression tests pin the stable messages and verify request argument and
  method/tool-name secrets are not echoed.
