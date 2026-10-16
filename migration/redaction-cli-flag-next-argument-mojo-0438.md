# Redaction CLI next-argument masking policy in Mojo

The compiled redaction module now decides when a sensitive CLI flag consumes
and redacts the following argument. The versioned Mojo ABI validates UTF-8,
requires a leading hyphen, ignores inline assignments containing `=`,
strips leading hyphens and reuses the existing compiled sensitive-key matcher
on the remaining flag name.

The Rust policy `redaction_cli_flag_name_looks_sensitive`, leading-hyphen
normalization, key/assignment splitting and inline-value guard were removed.
Rust still acquires OsString arguments, maintains iterator state, masks the
next argument when Mojo requires it and delegates inline value redaction to
the existing Mojo-owned secret-text redactor. There is no duplicate or silent
Rust fallback for the migrated sensitive-flag decision.

Strict Mojo ABI tests cover malformed UTF-8 and versions, negative lengths,
null pointers, bare flags, inline assignments, Unicode suffixes, upper case
and non-sensitive flags. Caller regressions verify that consecutive CLI args
preserve masking for sensitive flags but not ordinary flags. The source guard
rejects restoring the previous Rust flag parser.

The 75% project-wide production-share target remains outstanding.
