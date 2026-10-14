# Compiled Mojo ownership of transcript binary and mojibake classification

This migration removes the Rust production implementation of the
transcript-line and whole-output binary/mojibake policies, including
the punctuation table, Unicode scalar suspiciousness classification,
aggregation, and numeric thresholds. The policies are now entirely
authoritative in prodex_mojo_transcript_binary_policy_v1, exported from
mojo/prodex_core/log_semantics.mojo.

Rust remains a minimal Unicode-library fact adapter. It samples at most
4,096 valid Unicode scalars from a transcript line or output, forwarding
a compact array of codepoints and Rust standard-library whitespace,
alphanumeric and control properties to one bounded Mojo batch call.
Mojo owns punctuation semantics, replacement/control classification,
suspiciousness, all mojibake thresholds and final line/text decisions.
No per-character ABI calls, Rust rule duplication or fallback parser
remain. Errors and malformed ABI inputs fail closed.

The real production caller
crates/prodex-app/src/app_commands/log_transcript_text.rs keeps file/JSON
processing and applying visibility decisions. Rust-only Unicode table
facts remain in the typed caller-owned adapter
crates/prodex-mojo-core/src/log/transcript_ansi.rs. The existing log
module and mandatory Mojo archive build this operation on the real
prodex-app path, with versioned ABI and coverage in the canonical
Mojo ownership inventory.

## Verification

The Mojo 1.1.0 optimized ELF object exports the new ABI symbol and the
previous transcript ANSI normalization symbol. Full compiled-in Mojo
tests cover Unicode controls, CJK, emoji, replacement characters,
non-ASCII alphanumeric text, readable mixed text, thresholds, invalid
version/mode/count/flags, null pointers and surrogate rejection.

All 27 real prodex-app log caller tests passed, including:
- readable lines retained when tool output begins with binary garbage;
- binary/mojibake-looking lines suppressed without hiding readable lines;
- ANSI/OSC normalization, attachment path redaction, session log progress.

A separate reference harness assembled solely from the original
committed Rust production policy was used as a temporary differential
oracle. Across **20,010 deterministic texts and 40,020 line/text policy
evaluations**, compiled Mojo returned the exact same booleans as the
original Rust implementation. The reference is outside the source
tree and never part of a production binary or fallback.

The existing Mojo no-fallback, ownership, authority, source-size and
release-floor guards apply unchanged. The project-wide 75% Mojo target
is not yet achieved; this is a genuine authority transfer checkpoint
rather than inflated code volume.
