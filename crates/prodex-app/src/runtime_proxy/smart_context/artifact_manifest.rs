//! Canonical artifact labels for Smart Context test fixtures.
#![cfg(test)]

pub(super) fn runtime_smart_context_artifact_line_ref(
    id: &str,
    start: usize,
    end: usize,
) -> String {
    format!("{}#L{start}-L{end}", runtime_smart_context_artifact_ref(id))
}

pub(super) fn runtime_smart_context_artifact_ref(id: &str) -> String {
    format!("p{id}")
}
