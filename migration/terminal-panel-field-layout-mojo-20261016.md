# Mojo panel field-row layout

Panel field rows now obtain their wrapped value width and first/continuation
label padding from `prodex_terminal_field_row_layout_v1` in the compiled
terminal-layout Mojo module. Rust retains Unicode cell measurement, string
allocation, wrapping, and terminal rendering. Required-Mojo ABI and
terminal-ui caller tests cover Unicode labels, zero-width input, and tiny
terminal widths without a Rust layout fallback.
