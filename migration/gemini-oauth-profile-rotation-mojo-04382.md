# Gemini OAuth profile round-robin rotation moved to compiled Mojo

The production Gemini OAuth account pool now delegates fresh-request profile
attempt ordering to the compiled Gemini Code Assist policy owner. Rust still
looks up profile/session affinity, decodes the requested model and gathers
profile eligibility facts using the existing Mojo-backed model availability
policy. Only after the hard-affinity and sticky-session gates have passed does
the compiled round-robin kernel choose the starting profile, advance the
cursor, filter unavailable profiles, and select a last-chance all-profile
attempt order when every candidate was excluded by local model heuristics.

The prior Rust start-index clamp, modulo advancement, rotation index loop,
profile availability filter and empty-attempt fallback selection were deleted.
Rust only applies validated indices and stores the new cursor. The compiled
owner preserves all previous rotation fairness and avoids overriding existing
response/session affinity or replay restrictions.

The versioned Mojo ABI validates nonempty profile collections, eligibility
flags, pointers and output capacity before writing. Direct ABI tests cover
ordered rotation, unavailable-profile skipping, out-of-range cursor clamping,
single-profile pools, exhausted local eligibility, and malformed inputs. Real
Gemini OAuth request callers verify cursor progression and last-chance retries
when every profile is locally blocked; all 70 Gemini rewrite and 627 Mojo-core
library tests pass. The no-fallback guard rejects restored Rust round-robin
math and selection loops, while leaving unrelated host recovery decisions
unchanged.

This is a production-reachable migration checkpoint. The separate 75% Mojo
production-share target remains outstanding.
