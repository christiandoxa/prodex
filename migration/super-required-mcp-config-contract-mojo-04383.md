# Required inherited MCP configuration verification moved to Mojo

The production Super optional-tool activation flow now delegates inherited
MCP enabled state, exact command matching and ordered argument-vector
matching to the compiled optional-tools policy owner. This applies to
required Codebase Memory MCP and Playwright without silently overwriting
user configuration.

The previous Rust enabled check and command/args equality decision
functions were deleted. Rust still obtains TOML values, resolves a managed
OS executable, and performs optional tool filesystem configuration as
host responsibilities. Before resolving the expected command, the caller
invokes the compiled policy in enabled-only mode to preserve the established
precedence of disabled/nonboolean config errors over command-resolution
errors. The second compiled invocation verifies exact UTF-8 command bytes
and every argument in order. There is no Rust matcher fallback.

The versioned Mojo ABI validates enabled tags, lengths, pointers, UTF-8
command and argument spans and vector sizes before writing the typed
decision. Its tests cover absent/true/false/invalid enabled fields,
nonmatching command paths, reordered/missing/wrong-type arguments, Unicode
strings, malformed pointers and version mismatch.

All 62 optional-tools and 678 Mojo-core tests pass. The caller regression
covers required Playwright inherited config and verifies rejection of
disabled, nonboolean, custom-command and argument-list cases. The source
no-fallback guard prohibits restoration of Rust required MCP command or
enabled validators, and requires the compiled contract and ABI tests.

This is one completed, production-linked checkpoint. The long-term Mojo
production-share target of 75% is not yet achieved.
