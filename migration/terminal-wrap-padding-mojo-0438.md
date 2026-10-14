# Compiled Mojo ownership of terminal word joining and padding

This checkpoint extends the prior terminal Unicode wrapping migration
with two final deterministic text-layout policies. Both use versioned
Mojo ABIs in mojo/prodex_core/terminal_layout.mojo:

- prodex_terminal_wrap_piece_action_v1 chooses whether the next piece
  starts an empty line, joins a nonempty line with a single separating
  space, or flushes the current line. The policy uses the actual
  UnicodeWidthStr cell count measured by the Rust host and preserves
  the exact inclusive boundary behavior. The prior Rust branch and
  numeric line-width condition have been removed.
- prodex_terminal_pad_cell_spaces_v1 computes the exact count of
  padding spaces after fitting a terminal cell, saturating at zero.
  The former Rust width-comparison arithmetic has been removed.

The host continues to measure Unicode properties, manipulate UTF-8
strings, maintain terminal line buffers and operate I/O. The newly
compiled Mojo functions own only presentation decisions and return
bounded, validated selectors or counts; the host has no duplicate
semantic algorithm or fallback when an ABI call fails.

## Verification

Direct Mojo ABI tests cover empty buffers, exact-fit joins, line
flushing, zero-width Unicode strings, oversize widths, saturated
padding, invalid selectors, mismatched ABI versions and null pointers.
The complete terminal-ui tests cover wrapping, Unicode, ellipsis,
padding, sessions and actual Ratatui render callers.

A temporary differential Rust harness includes the preceding
production text.rs version only as a reference oracle outside the
repository. It compares actual terminal-ui compiled Mojo outputs
against pre-migration Rust for chunking, fitting, padding and wrapping
under the same deterministic Unicode corpus and width matrix. The
old behavior is never compiled or linked into Prodex production.
All **258,400 exact comparisons** were rerun and passed with the new
Mojo-owned wrap/padding rules active, across 3,800 deterministic Unicode
strings and 17 terminal widths. A separately compiled Mojo mutant
changed inclusive line-fit to exclusive line-fit. The same ABI caller
detected that regression, while the original compiled kernel passed.
The reference implementation remains outside the production repository.

These operations are declared in canonical Mojo ownership inventory
with production_fallback=false. The 75% target remains open and should
be measured from origin/main after integrating this checkpoint.
