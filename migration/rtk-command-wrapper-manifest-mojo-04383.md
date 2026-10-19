# RTK automatic command-wrapper manifest migrated to compiled Mojo

The production Codex-home RTK overlay no longer maintains a static Rust
table of wrapped commands or selected noisy subcommands. The compiled
RTK noisy-shell owner now publishes a versioned, bounded manifest of the
fourteen opted-in wrapper commands. The host uses that manifest to find
the real executable on PATH, guard against wrapper recursion, and
generate Unix shell or Windows cmd wrappers.

The original Rust per-command policy table and its static subcommand
lists were deleted. Rust retains only platform-dependent executable
discovery, quoting, script generation, and filesystem writes. Those are
host OS responsibilities; there is no Rust fallback manifest. The
compiled manifest preserves the exact existing command order, the selected
subcommands for git, cargo, npm, yarn, pnpm, bun, go, docker and kubectl,
and the unconditional wrappers for pytest, rg, find, ls, and tree.

The Mojo ABI validates version, caller-provided output and count
pointers, and output capacity before writing. The typed Rust adapter
verifies manifest structure and command/subcommand identifier shapes
without supplying alternative policy entries. Strict Mojo-core tests
assert the exact fourteen commands and all subcommands plus invalid ABI
cases. The live overlay caller test verifies script generation uses the
Mojo manifest, and all 63 optional-tools plus 680 Mojo-core tests pass.

The no-fallback guard rejects a restored Rust wrapper command table.
This production-linked checkpoint preserves platform wrapper semantics
while continuing work toward the separate, unmet 75% project target.
