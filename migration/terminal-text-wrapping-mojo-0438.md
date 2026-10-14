# Mojo authority over terminal Unicode wrapping and ellipsis

This checkpoint moves three deterministic production algorithms from
crates/prodex-terminal-ui/src/text.rs into the existing compiled-in Mojo
terminal layout module:

- prodex_terminal_token_chunk_plan_v1 selects all token split boundaries
  from per-scalar Unicode terminal display widths. Empty input,
  zero-width terminals, combining marks and overwide CJK/emoji scalars
  retain the exact previous behavior. Rust only slices UTF-8 at the
  Mojo-returned scalar indexes.
- prodex_terminal_take_display_cells_v1 selects the prefix scalar count
  for truncation under a Unicode display-cell budget, preserving leading
  zero-cell combining marks.
- prodex_terminal_fit_cell_plan_v1 selects empty, unchanged, dots, or
  prefix-and-ellipsis presentation and its associated cell budget.

Actual terminal user interfaces (wrap_text, fit_cell, pad_cell and
downstream session/report/doctor output) reach these functions through
crates/prodex-mojo-core/src/terminal_layout.rs. Rust's unicode-width
library still supplies Unicode display-cell facts, while allocation,
UTF-8 slicing, strings and terminal I/O remain Rust host concerns.
The previous Rust algorithms for segmentation, prefix planning and fit
presentation are deleted, with no runtime fallback, duplicate semantic
owner or artificially enlarged Mojo source.

## Verification

Real Mojo 1.1.0 code builds the three new versioned C ABI entry points.
The direct required-Mojo core tests cover boundaries, zero-width marks,
Unicode overwide scalars, output modes and invalid ABI pointer/capacity/
width inputs. The full terminal-ui test suite reaches the actual calling
pipeline and preserves wrapping, padding, narrow viewport and Unicode
output.

A separate disposable differential harness compiled the old text.rs
from the preceding commit, and linked against the real new terminal-ui
crate. It compared the exact outputs of chunk_token, fit_cell, pad_cell
and wrap_text for 3,800 deterministic Unicode input strings across
17 requested terminal widths: **258,400 exact matching results**.
The random corpus includes emoji, CJK, combining marks, zero-width
joiners, control characters, newlines and overwide scalars. It also
found and fixed an important legacy compatibility edge: an oversized
character must stay with preceding zero-width scalars when current
display width is zero. A separate Mojo mutation switched the safe legacy-compatible boundary
guard from current cell width to current character count. The exact
Rust-to-Mojo ABI caller immediately failed on the combining-mark-plus-CJK
case, while the original compiled Mojo ABI caller passed. The old
implementation is retained only in temporary test evidence outside the
production repository, never in the compiled application as a fallback.

The authoritative operations are registered in migration/mojo-ownership.json.
The target of 75% canonical Mojo production source has not yet been
reached. This is one validated checkpoint in that larger campaign.
