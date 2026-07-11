use super::*;

pub fn smart_context_repeat_tool_output_reference_summary(
    artifact: &SmartContextArtifactRef,
    text: &str,
    call_id: Option<&str>,
) -> String {
    let reference = smart_context_short_artifact_ref(&artifact.id);
    if smart_context_command_output_critical_signals(text).count == 0 {
        return reference;
    }

    let previous_record = smart_context_command_output_cache_record(reference.clone(), text);
    let rewrite = smart_context_command_output_cache_rewrite(SmartContextCommandOutputCacheInput {
        id: call_id.unwrap_or(&artifact.id).to_string(),
        text: text.to_string(),
        previous_records: vec![previous_record],
        min_replacement_bytes: SMART_CONTEXT_COMMAND_OUTPUT_CACHE_MIN_BYTES,
    });
    match rewrite.action {
        SmartContextCommandOutputCacheAction::ReplaceWithUnchangedSummary { .. } => {
            let mut output = rewrite.output;
            output.push_str("\nref ");
            output.push_str(&reference);
            output
        }
        SmartContextCommandOutputCacheAction::KeepExact { .. } => reference,
    }
}
