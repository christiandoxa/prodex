# Runtime profile selection jitter migrated to Mojo

Profile selection jitter now comes from the required
`prodex_runtime_profile_selection_jitter_v1` ABI in
`mojo/prodex_core/candidate_decision.mojo`. The kernel preserves the existing
`DefaultHasher` byte framing and outputs while validating the route tag,
profile view, ABI version, and caller-owned output pointer.

Rust retains request sequencing, route conversion, profile state, and selection
side effects. The runtime proxy has no Rust hash fallback; compiled-Mojo
vectors and the health caller cover the compatibility and rejection boundaries.
