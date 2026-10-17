# Quota pool summary section selection migrated to Mojo

The production CLI quota overview already aggregates OpenAI, Gemini and
Copilot snapshots through compiled Mojo kernels. The same quota-pool owner
now decides the complete section order when displaying the aggregated data:
OpenAI pools show ready, five-hour and weekly rows; without OpenAI windows,
Gemini/Copilot pools show the main remaining pool; if neither has data, the
summary shows the two unavailable OpenAI window rows.

The previous Rust `if/else if/else` summary layout policy was deleted. Rust
still materializes labels, calls existing Mojo quota-value formatters and
formats wall-clock reset timestamps. The versioned ABI returns a validated
ordered section plan without changing which aggregation source takes
precedence. No Rust fallback calculates the moved row-selection decision.

Direct ABI tests cover all three layouts, mixed OpenAI/main pools, large
profile counts, malformed inputs, invalid versions and insufficient buffers.
All 98 quota crate tests pass, including a real no-data caller regression,
Gemini/Copilot aggregation and a pool of 1025 OpenAI profiles. The no-fallback
guard requires the compiled plan and prevents restoring the Rust branches.

The separate project-wide 75% Mojo production-share target remains unmet.
