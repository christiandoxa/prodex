# Runtime-doctor facet and report planning ownership

Runtime-doctor facet winners and optional human-report row planning now run in
the compiled Mojo doctor owners. Mojo selects the highest-count facet with the
existing lexical tie rule and plans legacy, compact, sync-probe, probe-pressure,
and hot-reason rows, including `cold_start_jobs` precedence. Rust retains map
lookup, bounded ABI marshalling, value formatting, and `(label, value)` report
materialization. Required-Mojo caller tests cover Unicode ties and the exact
optional-row order.
