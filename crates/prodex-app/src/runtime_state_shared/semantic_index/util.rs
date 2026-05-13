use crate::runtime_state_shared::RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_FIELD_BYTES;

pub(in crate::runtime_state_shared) fn runtime_smart_context_bounded_string(
    value: &str,
) -> Option<String> {
    (!value.is_empty() && value.len() <= RUNTIME_SMART_CONTEXT_MAX_SEMANTIC_FIELD_BYTES)
        .then(|| value.to_string())
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_semantic_index_complete_default()
-> bool {
    true
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_bool_is_true(value: &bool) -> bool {
    *value
}

pub(in crate::runtime_state_shared) fn runtime_smart_context_u8_is_zero(value: &u8) -> bool {
    *value == 0
}
