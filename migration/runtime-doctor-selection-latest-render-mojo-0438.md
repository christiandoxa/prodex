# Runtime doctor latest selection decision rendering migrated to Mojo

The existing compiled runtime-doctor renderer (one canonical ABI authority) now owns the final
selection summary line. Its new operation writes the original marker
verbatim and appends one ASCII space and the selected detail only when
the detail is present and non-empty, preserving Unicode and field order.

Rust no longer chooses an empty-detail branch or runs format!(marker +
detail); the production selection summary uses the existing checked Mojo
render ABI. The new operation reuses the same renderer and avoids a
duplicate Rust formatter. The host retains BTreeMap collection and
selection state, not the migrated formatting semantics.

Verification includes direct compiled-Mojo cases with missing, empty,
and Unicode details, plus the existing runtime-doctor selection-summary
caller regression with exact latest-event equality.
