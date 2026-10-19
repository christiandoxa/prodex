# Gemini full-array tool declaration validation migrated to Mojo

The Gemini request translator now validates the complete Serde-normalized
tools array using a single compiled Gemini bridge request operation rather
than serializing each tool into an independent request and looping over a
Rust-owned validation plan. The production Mojo scanner reports the exact
original array position and first error, retaining the established public
messages for missing/invalid function fields, bad non-object entries and
incorrect tool-array shape. Media path exceptions are scoped to the
individual tool item, not to a later unrelated item in the same array.

OpenAI Responses tool shapes requiring legacy bridge translation remain
converted by the existing compiled chat-tools transform. After a translated
tool has been validated by the Mojo scanner, the Rust host submits a compact
ASCII verified-index mask and asks Mojo to resume scanning the *original*
array. The mask only records which original positions were already verified;
it is not a second Rust classifier. The Mojo owner validates the complete
mask grammar and exact array-length match before returning any success or
early tool error. The host rejects malformed, repeated or out-of-range
bridge indexes; the scanner continues through all remaining tools rather
than inadvertently accepting a request with an invalid trailing tool.

The old Rust per-tool validation loop, single-tool synthetic request
classifier, and error-index relabel helper were deleted. Serde remains the
JSON acquisition and serialization boundary, and Rust still materializes
the existing compiled-tool transformation outputs; no feature-off or
semantic Rust fallback was added. The new isolated-array operation reuses
the existing Mojo function declaration and builtin classifiers instead
of duplicating their policy.

Direct ABI tests prove the first failing original index, isolated local
media exception, batch bridge skip mask, corrupt lengths/characters,
unknown operation kinds and invalid shapes. Real provider regressions
verify valid and invalid tool declarations following one or multiple
bridged Responses tools, a mixed media/bridge/invalid array, and the
original precise error strings.

Local validation passed 476 provider-core, 699 compiled Mojo-core and
189 sequential Gemini app tests, together with the upstream Codex
compatibility baseline and no-fallback guard. Project-wide Mojo source
share remains below the independent 75% target.
