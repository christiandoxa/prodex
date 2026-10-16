# Gemini function-tool source shape owned by Mojo

A flattened OpenAI function tool previously required an additional Rust JSON
object, function-field map, and nested-function rewrite before entering the
already compiled Gemini ToolDeclaration kernel. The existing Mojo provider
constraints owner now chooses between nested, flat, or no function source via
its strict versioned ABI. A present nested `function` field always takes
precedence, including null, incorrect type, and a competing flat declaration.
A flat declaration is eligible only when the nested field is absent and its
`type` string exactly matches `function`.

The translation caller selects the Mojo-authorized borrowed JSON source, and
the existing Mojo request-content operation continues to own schema
normalization and wire shape. The previous Rust flat-to-nested map copy and
fallback path were deleted. Rust retains Serde parsing, value acquisition,
and JSON output projection; no duplicate source arbitration remains.

Direct ABI tests verify exact type spelling, Unicode padding, nested precedence,
invalid ABI values and shape flags. The production Gemini bridge caller test
verifies flattened Unicode names, nested precedence and explicit null nested
fields. The no-fallback guard disallows reintroducing Rust source selection.
The broad production-share goal of 75% remains separate and unfinished.
