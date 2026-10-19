# Anthropic native SSE dispatch migrated to compiled Mojo

The native Anthropic Messages streaming adapter now uses one versioned
compiled Mojo policy for event dispatch and stop-reason semantics. The
production Rust caller supplies the decoded SSE JSON event type, optional
content-block type/name, delta type, and existing ignored-block/web-search
presence facts. Mojo selects whether the event is a message start, tool
start, server web search, search result, text or tool-argument delta,
reasoning delta, web-search input fragment, message delta, terminal event,
invalid event, or a compatible ignored/future event.

The old Rust event-type, block-type, delta-type, web-search recognition,
and stop-reason comparison trees were deleted. Rust still owns stream
acquisition, JSON parsing, tool-call state, storage of web-search results,
and materializing the Mojo-selected response objects. The original
ignored-block and active web-search precedence remains in Mojo, so
malformed deltas inside ignored result blocks stay ignored and text
fragments inside active search blocks cannot leak into visible output.
Missing content-block or delta type remains an explicit stream failure.
Unknown future event types and signatures remain ignored.

Direct ABI tests verify version checks, malformed UTF-8, null pointers,
invalid flags, exact event names, all supported tool/delta families,
ignored/search state precedence, and stop reasons. Real production caller
tests verify text and reasoning, tool input fragments emitted once,
web-search result finalization, malformed events, ignored search deltas
and tolerated future events. There is no Rust classifier fallback.

This is a real production-linked migration checkpoint. The distinct
long-term 75% Mojo production-share goal remains unmet.
