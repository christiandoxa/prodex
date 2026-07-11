use prodex_provider_core::gemini_provider_core_bool_value;

pub(super) fn runtime_gemini_bool_value(value: &serde_json::Value) -> Option<bool> {
    gemini_provider_core_bool_value(value)
}
