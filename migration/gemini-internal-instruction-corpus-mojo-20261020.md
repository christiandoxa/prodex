# Gemini System Instruction Corpus Traversal Migrated to Mojo (2026-10-20)

## Ownership, security and behavior

The production Gemini value kernel now builds the raw internal-instruction
corpus through its versioned SystemCorpus JSON operation. It selects
messages with exact role "system", recursively evaluates the content field
with an explicit iterative traversal, visits arrays in their original
order, and visits objects only through text, content, input, output in
that priority. Each extracted string receives a newline and each
system message has an additional newline separator. The existing
compiled internal-instruction normalizer and echo detector continue
to consume the resulting raw text.

Rust only builds a flat borrowed tree from the Serde message slice,
decodes the Mojo-generated JSON string and invokes the existing
compiled normalizer. The old Rust role check, recursive JSON collector
and field-priority loop were removed. The generic host acquisition
helpers for one borrowed value and a borrowed synthetic array now
reuse a single iterative append/link implementation. No instruction
payload is cloned or serialized merely for the structural selection.

The kernel uses no unbounded recursive stack or dynamic List
allocation. It escapes JSON strings through the existing checked
JsonSink and therefore preserves quotes, backslashes, control
characters and Unicode before final normalization.

## Verification

- All 527 strict provider-core tests pass, including system-role
  filtering, object member priority, ignored unrelated branches,
  multiple system messages and a 1 MiB borrowed string nested inside
  256 levels of arrays.
- All 12 direct Gemini value ABI tests pass, including exact escaping
  of newline/quote/backslash, empty and non-array roots and scalar
  rejection.
- Source no-fallback guard/self-test, Cargo format, diff checks and
  canonical reachable production-share non-regression validation pass.

Official production share rises from 35.78999741606329% to
35.803093429485415% (130,276 Mojo / 233,592 Rust production LOC).
The 75% project target remains open, without changes to counting,
baselines or release policy. Remote CI is not a blocking prerequisite.
