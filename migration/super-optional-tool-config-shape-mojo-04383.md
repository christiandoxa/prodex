# Super optional-tool TOML shape preflight migrated to Mojo

The production Super optional-tool activation flow now delegates selected
TOML section shape, scope, and error precedence decisions to the compiled
optional-tools Mojo policy owner. Rust continues to read and decode TOML
and provides only the existence/type facts of eight relevant sections:
features, marketplaces, plugins, marketplaces.ponytail,
plugins.ponytail@ponytail, mcp_servers, codebase-memory-mcp, and playwright.

The original Rust Ponytail section validation loop and independently
selected MCP child validation loop were deleted. Mojo decides which
activated sections must be tables, which unselected sections are ignored,
which ancestor errors precede child errors, and the first precise
configuration error. Rust maps the typed decision to the already existing
user-facing message and does not recompute failed-selection policy.

The strict, versioned Mojo ABI validates all eight tri-state TOML shape
tags, the selection mask, output pointer and field count before writing.
Direct compiled tests exercise all error tags, Ponytail-first precedence,
independently selected MCP children, absent parents, invalid masks, and
corrupt shape arrays.

The production optional-tools caller regression covers selected and
unselected sections and preserves config.toml bytes when preflight fails,
before filesystem changes. All 61 optional-tools library tests and 676
Mojo-core tests pass. The anti-fallback guard rejects restored Rust
section validators and requires the compiled preflight and ABI tests.

The long-term 75% Mojo share target remains separate and is not met by
this individual checkpoint.
