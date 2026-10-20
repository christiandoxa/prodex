# Strict Chat-Completions SSE Envelope Migrated to Mojo (2026-10-20)

## Production ownership

The compiled openai_compat owner now classifies chat-completions SSE
events as unsupported framing, a completed DONE event, or borrowed
JSON data. The versioned prodex_openai_compat_sse_envelope_v1 ABI
requires the exact "data: " prefix and exact two-LF suffix, and only
recognizes the exact "[DONE]" payload as completed.

The exported entrypoint is defined in the linked rich_abi.mojo wrapper,
calling the sole classification implementation in openai_compat.mojo.
This avoids relying on an exported symbol from an imported-only module.

Rust retains lossy UTF-8 transport acquisition, checked result
decoding, borrowed slice application, Serde JSON validation and
Responses event materialization. The previous Rust prefix/suffix and
DONE classification branches were deleted, with no Rust semantic
fallback.

## Verification

- All 533 provider-core tests pass, including two new translator
  regressions for DONE, Unicode, malformed UTF-8 replacement, and
  malformed/noncanonical framing.
- Two core ABI tests verify borrowed ranges, empty JSON, case
  sensitivity, CRLF rejection, invalid UTF-8/pointers and no writes
  on rejected ABI requests.
- Direct Mojo regression, no-fallback guard/self-test, Cargo format
  and diff checks pass.
- Official reachable working-tree production share:
  35.88328851577359% (131,307 Mojo / 234,621 Rust).

The 75% project goal remains unmet. Metric definitions and release
floors were not changed. Remote CI green is not a prerequisite.
