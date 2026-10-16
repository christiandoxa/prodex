# Provider quota header text and Gemini model source migrated to Mojo

The production adapter that presents provider/Gemini quota information as
Codex-compatible HTTP rate-limit headers now delegates text normalization
and model label selection to the existing compiled provider constraints owner.

Mojo owns CR/LF removal, Unicode edge-whitespace trimming (without altering
interior Unicode whitespace), and the precedence of a nonblank Gemini model
ID over its token type. The exact `models/` prefix is stripped only from
selected model IDs. A selected model ID consisting solely of the prefix
still suppresses token-type fallback, preserving the preexisting behavior.

The Rust `clean_header_value` and `gemini_bucket_model_label` implementations
were deleted with no semantic fallback. Rust retains quota-record acquisition,
HTTP header storage, time parsing, numeric formatting, and materialization of
the returned strings, with fail-closed compiled Mojo ABI calls.

Strict ABI tests check version, output capacity, pointers, model source
precedence, CR/LF, leading/trailing Unicode whitespace, preserved inner
whitespace, case-sensitive prefixes, and Unicode model IDs. The production
caller tests cover multiple Gemini buckets, fallback account names, model
source precedence, and OpenAI-style quota label formatting. The no-fallback
guard requires the compiled Mojo source and prevents restored Rust parsers.

This is a reachable production migration, not a waiver for the independent
75% long-term Mojo ownership goal.
