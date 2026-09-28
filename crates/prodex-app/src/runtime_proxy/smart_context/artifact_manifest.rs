use super::*;

#[cfg(test)]
pub(super) fn runtime_smart_context_artifact_line_ref(
    id: &str,
    start: usize,
    end: usize,
) -> String {
    format!("{}#L{start}-L{end}", runtime_smart_context_artifact_ref(id))
}

pub(super) fn runtime_smart_context_artifact_ref(id: &str) -> String {
    if let Some(hash) = id.strip_prefix("sc2:") {
        format!("psc2:{hash}")
    } else {
        format!(
            "{SMART_CONTEXT_SHORT_ARTIFACT_REF_PREFIX}{}",
            id.strip_prefix("sc:").unwrap_or(id)
        )
    }
}

pub(super) fn runtime_smart_context_artifact_id_valid(id: &str) -> bool {
    prodex_mojo_core::smart_context_artifact_ref::artifact_id_valid(id)
        .expect("Mojo smart-context artifact id validator returned invalid output")
}
