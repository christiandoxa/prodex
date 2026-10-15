# Gemini response-part batch planning in Mojo

Gemini buffered response translation now sends bounded per-part facts to the compiled Mojo batch planner. Mojo owns action selection, function-call presence flags, and grounding/citation tail order; Rust keeps Serde acquisition, existing media and grounding projections, tool callbacks, and JSON materialization. The ABI rejects more than 65,536 parts before invoking the kernel, and the direct ABI plus provider response fixtures cover ordering, tool-text suppression, empty input, malformed fields, Unicode, and the size bound.
