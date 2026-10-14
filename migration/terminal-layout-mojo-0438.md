# Mojo terminal layout semantic ownership checkpoint

This checkpoint migrates three previously Rust-owned terminal calculations
to a compiled-in, versioned Mojo ABI with no Rust algorithm fallback:

- Panel label width: observed Unicode cell widths remain host inputs,
  but terminal width limits, preferred column size, and label clamping are
  decided by prodex_terminal_panel_label_width_v1 in Mojo.
- Session report columns: Unicode-aware gap width is measured by Rust,
  while five columns, maximum widths, tiny-terminal floor and remainder
  split are decided by prodex_terminal_session_columns_v1 in Mojo.
- Ratatui inline viewport: minimum and maximum line counts are decided
  by prodex_terminal_panel_inline_height_v1 in Mojo.

The shared build graph compiles mojo/prodex_core/terminal_layout.mojo
under mojo-runtime and mojo-core. The typed ABI host module is
crates/prodex-mojo-core/src/terminal_layout.rs. The actual production
callers are crates/prodex-terminal-ui/src/panel.rs and
crates/prodex-terminal-ui/src/session.rs. Rust remains responsible only
for terminal I/O, Unicode cell measurement, string materialization,
Ratatui styling/drawing, and ABI marshaling.

## Verification

The compiled Mojo ELF object exports all three ABI v1 symbols with no
unresolved external symbols. A Rust executable linked directly to this
object confirms panel width, session columns and viewport height at
the ABI boundary. Deliberately mutating the Mojo label-width subtraction
from 20 to 21 causes the same executable to fail, while the original
source and compiled object remain unchanged.

Required-Mojo tests passed:

- terminal-ui caller suite: 26 passed, covering Unicode, tiny terminals,
  report truncation, and Ratatui layout.
- terminal_layout Rust-to-Mojo direct suite: 4 passed, covering bad
  ABI/pointer/capacity, odd remainders, u16 upper limits and width clamps.

All new operations are registered in canonical Mojo ownership with
production reachability, explicit ABI owner and production_fallback=false.
This checkpoint does not change runtime CLI flags, session storage,
provider routing or upstream requests.

The 75% share goal remains outstanding: this is a validated production
checkpoint, not filler code or a claim of campaign completion.
