#[path = "tooling/messages.rs"]
mod messages;
#[path = "tooling/response_tool_calls.rs"]
mod response_tool_calls;

pub(super) use self::messages::deepseek_messages_from_request;
pub(crate) use self::messages::deepseek_tool_call_thought_signature_object;
pub(super) use self::response_tool_calls::deepseek_responses_tool_call_item;
pub(crate) use self::response_tool_calls::deepseek_rtk_wrapped_tool_arguments;

#[cfg(test)]
mod tests {
    use super::deepseek_tool_call_thought_signature_object;
    use serde_json::json;

    #[test]
    fn deepseek_tool_call_thought_signature_object_reads_google_extra_content() {
        let value = json!({
            "extra_content": {
                "google": {
                    "thought_signature": "sig-1"
                }
            },
            "thought_signature": "ignored"
        });

        let object = value.as_object().unwrap();
        assert_eq!(
            deepseek_tool_call_thought_signature_object(object),
            Some("ignored".to_string())
        );
    }
}
