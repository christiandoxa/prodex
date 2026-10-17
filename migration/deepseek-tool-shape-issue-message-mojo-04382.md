# DeepSeek/Kiro tool-shape diagnostic messages moved to compiled Mojo

The compiled DeepSeek request policy has long selected the authoritative
`ToolsShape` issue tag and bounded detail string. That same compiled domain
now formats the stable provider-prefixed diagnostic for all 23 supported
shape failures (plus the generic unknown-issue case), including MCP toolset,
namespace function, strict schema and custom-tool validation.

The Rust `match plan.tag` mapping was deleted. The caller retains JSON
acquisition and typed failure propagation, while the versioned Mojo ABI
renders the precise error message for DeepSeek or Kiro. There is no Rust
fallback implementation of the moved issue-to-message policy.

The ABI validates version, pointer, UTF-8, output capacity and payload
lengths before writing any bytes. Direct tests verify every issue code,
dynamic namespace/tool details, Unicode provider labels and malformed ABI
inputs without partial writes. Production callers cover both DeepSeek and
Kiro labels, unsupported tool types and MCP quota-like toolsets. All 467
provider-core library tests pass and a no-fallback guard prevents restoring
the Rust error switch.

This is a production-reachable migration checkpoint; the independent 75%
Mojo production-share target remains outstanding.
