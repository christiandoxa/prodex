# Status resource aggregation in Mojo

`prodex_status_resource_aggregate_v1` now owns saturated aggregation of the
host-collected process and network counters used by `prodex status`. Rust keeps
`/proc` reads, process and socket enumeration, parser calls, and caller-owned
buffer materialization. Empty or malformed host samples retain the existing
zero-value behavior, while ABI and capacity errors mark the resource snapshot
unavailable without recomputing the totals in Rust.
