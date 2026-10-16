# Terminal geometry input parsing migrated to compiled Mojo

Prodex's existing `terminal_layout.mojo` production owner now validates and
parses terminal dimension overrides and the first two `stty size` fields.
The Mojo ABI accepts borrowed UTF-8 input and the host `usize` upper bound,
trims Unicode whitespace, supports an optional decimal `+` sign, rejects
negative values and checked unsigned overflows, and rejects zero-sized
environment overrides. The `stty` parser preserves valid zero values and
ignores additional tokens after the first two dimensions, matching the
existing terminal behavior.

The previous Rust `.trim().parse::<usize>()` override parser and
`split_whitespace`/parse chain for `stty` output were removed. Rust still
owns environment reads, `/dev/tty`, the `stty` process, UTF-8 conversion and
host-size materialization, not the migrated decisions. No Rust parser fallback
is present. Malformed Mojo ABI output fails closed.

Production caller tests cover missing and Unicode-padded values and malformed
stty output. Direct compiled-Mojo tests cover large bounds, the full
`usize::MAX` value, optional plus, zero, invalid digits, Unicode spacing,
trailing tokens and wrong ABI versions, pointers, lengths and capacities.
The broad 75% production-share target remains outstanding.
