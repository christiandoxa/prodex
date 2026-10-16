# Runtime-doctor Smart Context reason tokenizer in compiled Mojo

Smart Context diagnostic reasons and policy reasons now use the existing
runtime-doctor marker module to split comma-separated fields, trim Unicode
whitespace and discard empty or single-dash placeholders. The Rust
`split(',')`, `trim`, and ignored-value filtering chain was removed. Rust
acquires the log field and copies verified source spans; no semantic Rust
parser or fallback survives the migration.

The versioned two-pass ABI first counts accepted spans, then fills caller-owned
UTF-8 offsets. It validates source views, output capacity, and an explicit
65,536-row upper bound. The Rust adapter validates the returned byte ranges
before constructing owned strings. Strict ABI tests cover punctuation,
Unicode trimming, repeated separators, non-placeholder dashes, incorrect ABI
versions, insufficient buffers, and invalid capacities.

The real runtime-doctor Smart Context caller regression covers Unicode reason
fields from JSON logs, alongside the existing 64 passing doctor tests.
This checkpoint is production reachable but does not claim the 75% goal.
