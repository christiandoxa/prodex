# DeepSeek Buffered Text-Item JSON Construction Migrated to Mojo (2026-10-20)

## Production ownership and buffer sizing

The live DeepSeek buffered chat-to-Responses translator now calls
the compiled output-text item JSON writer rather than building
message, role, and content objects with Rust JSON macros.
The existing DeepSeek kernel owns escaping, Unicode, message layout,
and response text item shape. One shared deepseek_write_output_text_item
renderer serves both streaming operation 15 and buffered operation 51.

These typed operations preserve distinct resource limits: streaming
output-text remains bounded to 4 MiB; buffered output-text accepts
up to 16 MiB, matching the original buffered-response envelope.
Rust retains Serde field acquisition, checking Mojo output,
applying the compiled text-item object and preserving tool-call
order and errors. The duplicated Rust JSON assembly was deleted
and no Rust semantic fallback remains.

## Verification

All 535 strict provider-core tests pass, including new real buffered
caller checks for Unicode, CRLF and JSON escaping, tool-call ordering
and absent or non-string content. Two core tests check byte-identical
stream and buffered output and a greater-than-4-MiB buffered payload
without relaxing stream limits. Direct Mojo writer test, anti-fallback
guard, Cargo format, diff check and canonical share check pass.

Official production share rises from 35.905425870090184% to
35.90709045120557% (131,602 Mojo / 234,905 Rust production LOC).
The 75% goal remains open. Counting rules, release floor and
non-regression baseline were not changed. No remote green CI gate
was required to publish this checkpoint.
