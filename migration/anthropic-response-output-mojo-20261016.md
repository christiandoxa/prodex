# Anthropic response output in Mojo

Anthropic Messages response translation now sends the Mojo-owned response plan and
preflight materialized items through `ResponseOutput` operation 33. Mojo applies
ordered message, tool, reasoning, and web-search output shaping, including the
last matching search-result attachment; Rust retains Serde acquisition,
provider error mapping, and host clock/envelope effects.

Compiled ABI and provider-boundary tests preserve Unicode, escaped reasoning,
duplicate search-call ordering, result-before-call behavior, and existing wire
shapes.
