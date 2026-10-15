# Runtime log throughput display policy in Mojo

The log TUI's output-rate display policy now runs through the required Mojo ABI. Mojo owns positive-rate validation, sub-100 tenths rounding, and the integer-versus-tenths precision decision. Rust retains optional-value marshaling, returned-plan validation, and final string construction. The compiled Mojo bridge and app caller tests cover invalid values, rounding boundaries, and threshold formatting.
