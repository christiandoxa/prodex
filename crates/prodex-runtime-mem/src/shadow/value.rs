use super::summary::runtime_mem_value_contains_text;
use super::*;

pub(crate) fn runtime_mem_value_is_text(value: &Value) -> bool {
    value.as_str().is_some_and(|text| !text.trim().is_empty())
}

pub(crate) fn runtime_mem_value_contains_artifact_marker(value: &Value) -> bool {
    if runtime_mem_extract_artifact_marker(value).is_some() {
        return true;
    }
    runtime_mem_value_contains_text(value, "prodex smart context artifact")
}
