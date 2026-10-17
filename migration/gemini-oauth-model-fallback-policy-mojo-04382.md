# Gemini OAuth fallback-model selection moved into compiled Mojo

The production Gemini OAuth profile pool now delegates model-availability
eligibility and stable preferred-model reordering to the existing compiled
Gemini Code Assist policy owner. Rust supplies cached profile/endpoint
cooldown timestamps, cached successful model preference, and ordered model
names; Mojo decides which models are eligible and which preferred model
appears first. The single compiled policy is used both to filter outgoing
model chains and to decide whether a candidate OAuth profile has any usable
model for the current endpoint.

The Rust model-availability predicates and preferred-chain matching/reordering
were deleted without a Rust fallback. Profile/session affinity lookups,
BTreeMap state, clock acquisition, expiration pruning, credentials, and
application of returned typed model indices remain in Rust. Missing cached
deadlines are represented by zero, preserving the previous allow-default
behavior. A model is eligible exactly at its cooldown deadline. Existing
stable order and case-sensitive Unicode model matching remain intact;
duplicate instances of the preferred model collapse to one as before.

The versioned availability and preference ABIs validate lengths, pointer
shapes, UTF-8 model views, output capacity, and result bounds before callers
materialize strings. Direct ABI regressions cover UTC millisecond thresholds,
endpoint-specific cooldowns, missing/unknown preferences, duplicate preferred
model names, Unicode and malformed ABI input. The real OAuth pool regression
covers active cooldowns, independent endpoint blocking, duplicated preferred
models and stable fallback chains. All 68 relevant Gemini rewrite tests and
625 Mojo-core tests pass. A no-fallback guard rejects restoration of the Rust
availability/reordering code.

The independent 75% Mojo production-share project target remains outstanding.
