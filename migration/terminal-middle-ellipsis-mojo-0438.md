# Mojo middle ellipsis policy

The log TUI's middle ellipsis boundary selection now runs through
`prodex_terminal_middle_ellipsis_plan_v1` in the compiled terminal-layout Mojo
module. Mojo selects empty, unchanged, dot-filled, or prefix/suffix output and
their scalar budgets. Rust still measures Unicode terminal cells, slices valid
UTF-8, builds the display string, and owns TUI I/O.

Required-Mojo ABI and terminal-ui caller tests cover wide and zero-cell Unicode,
combining marks, tiny widths, and the host `u16` viewport limit. The app log
header reaches the same helper, with no Rust semantic fallback.
