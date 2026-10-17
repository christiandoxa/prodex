# Chat-compatible response tool-call classification moved to Mojo

The compiled chat-tools domain now owns the validity and special-tool type
classification of response tool calls from OpenAI-compatible providers.
The production chat-compatible response bridge supplies whether the function
object and string name are present; Mojo distinguishes a missing function,
missing/Unicode-whitespace-only name, exact `tool_search`, exact
`apply_patch`, and an ordinary function. Rust no longer branches on the
special names or duplicates the name-blank classifier.

Rust retains serde field acquisition, provider-specific error transport,
JSON validation of tool arguments, RTK argument rewriting, generation of
fallback call IDs, signature metadata extraction and result materialization.
The existing compiled OpenAI chat-bridge response renderer remains the owner
of the final OpenAI Responses tool item layout. There is no Rust fallback
classifier on Mojo ABI failures.

The versioned ABI rejects bad flags, lengths, pointers and malformed UTF-8
before writing. Direct compiled tests verify exact-case names, Unicode
whitespace and invalid ABI shapes. The real provider response caller test
verifies special tools, plain names differing only by trailing whitespace,
Unicode MCP function names and a failure from an all-whitespace function
name; the entire 468-test provider-core suite passes. The source guard
rejects restored Rust special-name comparison and Unicode blank policies.

The independent 75% production Mojo ownership target remains incomplete.
