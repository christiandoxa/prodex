# Live log TUI ASCII case-insensitive substring search migrated to Mojo

All three production log search consumers (live transcript TUI, upstream
payload logs, and aggregated load-run searches) now delegate ASCII-folded
substring matching to a single compiled Mojo log-semantic owner. The old
Rust search algorithm, which allocated lowercased copies of both the
haystack and the query, was deleted; the Rust entrypoint now fails
explicitly on invalid Mojo ABI output and never recomputes the match.

The compiled implementation uses a Knuth-Morris-Pratt prefix table over
borrowed, validated UTF-8 byte views. Only ASCII A-Z bytes fold to a-z,
while non-ASCII UTF-8 remains byte-exact. This preserves the old
to_ascii_lowercase-plus-contains contract, including empty needles,
overlapping prefixes, emoji, accented letters and case-sensitive Unicode.
Time complexity is O(haystack bytes + query bytes) and host scratch is
O(query bytes), without copying the entire log entry.

Versioned direct ABI tests cover invalid UTF-8, missing pointers, bad
lengths, undersized scratch, malformed ABI version and no-partial-write
behavior. The test-only historical expression is used solely as an oracle
for compatibility, not as a production fallback. Production caller
regressions verify case-insensitive Ready/Blocked quota entries and
unchanged Unicode matching in log search.

All 683 Mojo-core tests, 15 log TUI tests, and 12 upstream log tests
pass, as do the runtime guard and upstream compatibility baseline.
No filter that ran zero tests is counted as positive coverage. The
no-fallback guard requires real Mojo delegation and rejects restored
Rust lowercase matching.

The independent 75% Mojo project target remains outstanding.
