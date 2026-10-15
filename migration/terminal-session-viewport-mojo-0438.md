# Mojo ownership of session-report viewport planning

Session-list viewport planning now runs through the compiled terminal-layout
Mojo ABI. Mojo owns the saturating report-to-scroll-line calculation, fixed
panel-row allowance, and inline-height clipping; Rust retains terminal-size
acquisition, checked result conversion, and Ratatui rendering. The former Rust
arithmetic is removed from the app caller without a fallback.

Direct ABI and terminal-ui caller tests cover empty, tiny, normal, large, and
saturated report counts, plus a narrow Unicode session report. The 75% project
target remains in progress.
