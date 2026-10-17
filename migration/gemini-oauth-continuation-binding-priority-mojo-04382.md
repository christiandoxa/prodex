# Gemini OAuth continuation binding precedence migrated to compiled Mojo

The production Gemini OAuth pool now uses the compiled Gemini Code Assist
policy owner to determine binding precedence, identically for profile
routing and model scope. Rust still parses request JSON and looks up
`previous_response_id` and tool-output call IDs in the pool's state maps,
but Mojo chooses the bound previous response before any bound tool output,
otherwise the first bound tool output in request order. The typed result is
applied to the correct map without a Rust retry/fallback decision.

This replaces two near-identical Rust `if let previous_response` plus
`find_map` routing branches with one common typed host adapter. The
versioned Mojo ABI rejects malformed flags, pointers, lengths or output
shapes and reports `no binding`, `previous response`, or an exact tool-output
index. The Rust adapter validates the selected index against the original
bound inputs before applying it, rather than recomputing the choice.

Direct ABI tests cover missing bindings, single and multiple bound outputs,
priority of a bound previous response, index order and malformed inputs.
The production pool caller regression verifies both profile and model scope
with conflicting bindings, an unknown previous-response ID, multiple tool
outputs, malformed request JSON and no bindings. The full 71 Gemini rewrite
and 631 Mojo-core suites pass. The no-fallback guard rejects restoration of
the prior duplicated Rust `find_map` and previous-response decision tree.

This checkpoint preserves strict continuation affinity and is separate from
the long-term 75% Mojo production ownership target, which remains unmet.
