# Gemini rejected builtin retry selection moved to compiled Mojo

The production Gemini bridge now delegates the priority order and exhaustion
of explicitly rejected builtin-tool retry attempts to the existing compiled
Gemini tooling policy module. The four borrowed tool labels are only a stable
ABI index projection: computerUse, codeExecution, urlContext, and googleSearch.
Mojo chooses the first rejected and unattempted index; Rust obtains rejection
facts via the already Mojo-owned provider error classifier, locates the chosen
request member, and applies JSON mutation through the existing Mojo request
bridge. A rejected but absent builtin is skipped, allowing a later rejected
and present builtin to be removed as before.

The prior Rust filter/find_map ordering is deleted, with no Rust fallback
candidate selector. The new versioned ABI validates the supported mask and
output pointer and returns an explicit none index when candidates are exhausted.
Strict direct ABI tests cover each priority tier, sparse masks, exhaustion,
malformed version/masks and output non-mutation. The production caller test
covers absent earlier tools, present later tools, dual rejections, and errors
that do not explicitly reject a request field.

This checkpoint preserves the long-term 75% project target; it does not claim
that target has been reached.
