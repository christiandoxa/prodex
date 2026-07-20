use super::super::RuntimeDeepSeekConversationStore;
use prodex_provider_core::deepseek_provider_core_history_has_tool_call;

pub(in crate::runtime_launch::proxy_startup::deepseek_rewrite) fn runtime_deepseek_history_for_tool_call(
    conversations: &RuntimeDeepSeekConversationStore,
    call_id: &str,
) -> Option<Vec<serde_json::Value>> {
    conversations
        .find_history(|history| deepseek_provider_core_history_has_tool_call(history, call_id))
}
