# Mojo ownership of persisted transcript ANSI normalization

This checkpoint removes the previous Rust escape-sequence state machine
from the real Prodex transcript tool-output viewer. Its replacement is
one authoritative, compiled-in Mojo ABI, independent of the host TUI:

- prodex_mojo_transcript_ansi_strip_v1 in
  mojo/prodex_core/log_semantics.mojo strips CSI terminal sequences
  through the first ASCII final byte, OSC titles through BEL or
  ESC-backslash, and one Unicode scalar following a nonstandard escape.
  It also maps lone CR to LF and CRLF to LF.
- crates/prodex-mojo-core/src/log/transcript_ansi.rs is the narrow
  caller-owned byte-buffer and UTF-8/result validation adapter.
- crates/prodex-app/src/app_commands/log_transcript_text.rs calls the
  Mojo ABI directly and fails closed if it returns an error.
  The duplicate Rust ANSI and OSC parser functions were deleted.

Existing tool-output visibility, binary/mojibake classification,
attachment concealment, log I/O and terminal display responsibilities
remain unchanged in their existing host modules. The separate generic
pointer-address marshaling helpers moved from the oversized log.rs ABI
surface to a focused private log/pointers.rs host module. The
size-guard allowlist for log.rs was reduced from 965 to 959 lines.

## Verification and safety boundaries

The real Mojo 1.1.0 source compiled to an optimized x86_64 ELF object;
llvm-nm confirmed the required C ABI symbol. An independent differential
harness linked to the unchanged original compiled Mojo object compared
6,000 deterministic valid-UTF-8 escape/CSI/OSC/CR combinations with
Rust's former production implementation recovered from the prior commit;
all 6,000 matched byte-for-byte. A separately compiled Mojo source mutant
that breaks CR normalization failed that harness, while the original
kernel still passed. The Rust reference exists only in disposable test
evidence, never in the production source or as a fallback. Direct required-Mojo tests
cover CSI, OSC/BEL, ESC-backslash, complete and incomplete escapes,
multibyte Unicode, CRLF and lone CR, blank payloads, invalid ABI,
null pointers and output-capacity failure. Caller tests exercise the
actual persisted transcript text pipeline with colored text and Unicode.
No Rust fallback parser or duplicate semantic logic remains.

The source and host adapter are registered in the canonical ownership
inventory with production_fallback=false. The release still requires
the same mandatory Mojo archive to link successfully on all supported
platforms. Production log redaction, path hiding, transport behavior,
session attachments and credentials are unaffected by the migration.

This is a distinct, measured semantic-owner checkpoint. The long-term
75% canonical Mojo production-share target remains outstanding.
