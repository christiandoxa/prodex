# Dry-run private path redaction migrated to Mojo

The compiled privacy/redaction.mojo domain now owns both operations
required to redact user machine paths in launch dry-run reports.

The path-order operation filters empty values, performs a stable
longest-UTF-8-byte-first ordering, and drops adjacent equal paths.
The replacement operation applies the canonical redacted-path marker
to all nonoverlapping occurrences, including overlapping private
path prefixes. It reports exact output capacity before writing;
the host never guesses a fixed-size buffer for expansion.

The previous Rust path sort, dedup, and str.replace chain was deleted.
Rust retains host filesystem/path display, borrowed string views,
bounded ABI allocation and checked UTF-8 reconstruction. No fallback
Rust redaction algorithm remains. Other token/secret redaction stages
and their existing security contracts are unchanged.

Direct compiled Mojo tests verify overlapping private paths, duplicate
paths, Unicode, short-path expansion, equal-length ordering, strict
capacity and bad ABI. The real runtime-launch dry-run report test
asserts that the private profile path is not present in rendered
arguments/environment, while its public placeholder is preserved.
The 75% broad Mojo source-share project target is not yet achieved.
