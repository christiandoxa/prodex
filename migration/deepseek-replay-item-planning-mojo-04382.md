# DeepSeek replay item classification and emission moved to Mojo

The compiled DeepSeek domain now owns the replay item-kind classification and
the final decision to suppress an item, re-emit a tool call, or emit a new MCP
result without replaying the previously emitted tool call. Versioned ABIs
expose four supported categories (text, tool call, tool output, MCP call) and
an explicit host-applied plan for streaming the call and result. This keeps
Responses history reconstruction and continuation replay consistent with the
existing compiled RawBridgeInputItem kernel.

The old Rust item-type match arms, MCP classification, replay skip conditions,
and call/output emission decisions have been deleted. Rust retains serde
item acquisition, BTreeSet history membership, Unicode text retrieval,
duplicate-text facts, and the presence bits of the four possible MCP result
fields. Mojo alone combines these facts and selects the output actions.
There is no alternate Rust replay or suppression algorithm on Mojo errors.

Direct strict ABI tests cover all item types, malformed UTF-8, missing ABI
pointers, invalid tag/capacity/boolean fields, replayed calls, result fields,
duplicate output IDs, and blank or previously seen text. The actual provider
caller test reproduces a replayed MCP call with a newly available tool result,
then verifies the result is not replayed twice and duplicate text is skipped.
All 466 provider-core tests pass, and the no-fallback guard enforces the
compiled path and rejects restoration of the Rust classification branches.

The separate 75% project Mojo production-share target is still outstanding.
