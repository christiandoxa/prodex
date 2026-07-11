use crate::translator::{
    ProviderParamSupport, ProviderTransformInput, ProviderTransformResult, ProviderTranslator,
    ProviderUnsupportedReason,
};
use crate::{ProviderEndpoint, ProviderId, ProviderWireFormat, provider_supported_endpoints};
use serde_json::Value;
use std::collections::BTreeMap;

#[path = "kiro/acp.rs"]
mod acp;
#[path = "kiro/compact.rs"]
mod compact;
#[path = "kiro/request.rs"]
mod request;
#[path = "kiro/response.rs"]
mod response;
#[path = "kiro/stream.rs"]
mod stream;
#[path = "kiro/supported_params.rs"]
mod supported_params;

pub use self::acp::{
    kiro_provider_core_acp_assistant_output_message, kiro_provider_core_acp_chat_assistant_message,
    kiro_provider_core_acp_error_value, kiro_provider_core_acp_incomplete_details,
    kiro_provider_core_acp_incomplete_details_value, kiro_provider_core_acp_initialize_request,
    kiro_provider_core_acp_mark_failed_response, kiro_provider_core_acp_mark_incomplete_response,
    kiro_provider_core_acp_metadata, kiro_provider_core_acp_model_value,
    kiro_provider_core_acp_plan_entry, kiro_provider_core_acp_response_value,
    kiro_provider_core_acp_session_info, kiro_provider_core_acp_session_new_request,
    kiro_provider_core_acp_session_prompt_request, kiro_provider_core_acp_stop_reason,
};
pub use self::compact::{
    kiro_provider_core_compact_summary_from_response,
    kiro_provider_core_semantic_compact_instructions,
    kiro_provider_core_semantic_compact_request_body,
};
pub use self::request::{
    KiroProviderCoreRequestError, kiro_provider_core_chat_completions_request_body,
    kiro_provider_core_prompt_from_chat_messages,
    kiro_provider_core_responses_items_from_chat_message,
    kiro_provider_core_tool_choice_from_legacy_chat_function_call,
    kiro_provider_core_tool_from_legacy_chat_function,
};
pub use self::response::{
    kiro_provider_core_anthropic_message_value_from_response,
    kiro_provider_core_apply_response_runtime_metadata,
    kiro_provider_core_chat_completion_finish_reason,
    kiro_provider_core_chat_completion_finish_reason_from_response,
    kiro_provider_core_chat_completion_value_from_response,
    kiro_provider_core_invalid_request_error_value, kiro_provider_core_model_list_value,
    kiro_provider_core_model_value_or_not_found, kiro_provider_core_response_has_tool_calls,
    kiro_provider_core_unsupported_path_error_value,
};
pub use self::stream::{
    kiro_provider_core_acp_chat_tool_call_item, kiro_provider_core_acp_responses_tool_call_item,
    kiro_provider_core_acp_usage_update_json, kiro_provider_core_chat_completion_chunk,
    kiro_provider_core_chat_completion_empty_delta, kiro_provider_core_chat_completion_role_delta,
    kiro_provider_core_chat_completion_text_delta,
    kiro_provider_core_chat_completion_tool_call_delta, kiro_provider_core_output_item_added_event,
    kiro_provider_core_output_item_done_event, kiro_provider_core_output_text_delta_event,
    kiro_provider_core_response_completed_event, kiro_provider_core_response_created_event,
    kiro_provider_core_stream_content_text, kiro_provider_core_stream_tool_arguments,
    kiro_provider_core_stream_tool_call_item,
    kiro_provider_core_tool_call_arguments_delta_chat_value,
};

#[derive(Clone, Copy)]
pub struct KiroTranslator;

impl ProviderTranslator for KiroTranslator {
    fn provider(&self) -> ProviderId {
        ProviderId::Kiro
    }

    fn client_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::OpenAiResponses
    }

    fn upstream_wire_format(&self) -> ProviderWireFormat {
        ProviderWireFormat::Passthrough
    }

    fn supported_params(&self, endpoint: ProviderEndpoint, _model: &str) -> ProviderParamSupport {
        if endpoint == ProviderEndpoint::ChatCompletions {
            return supported_params::kiro_chat_completions_supported_params();
        }
        if provider_supported_endpoints(self.provider()).contains(&endpoint) {
            ProviderParamSupport::full()
        } else {
            ProviderParamSupport {
                supported: false,
                unsupported: vec![ProviderUnsupportedReason {
                    field: endpoint.label().to_string(),
                    reason: format!(
                        "{} does not expose {}",
                        self.provider().label(),
                        endpoint.label()
                    ),
                }],
            }
        }
    }

    fn transform_request(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        if !kiro_supported_endpoint(input.endpoint) {
            return ProviderTransformResult::unsupported(
                self.provider(),
                input.endpoint,
                self.client_wire_format(),
                self.upstream_wire_format(),
                format!(
                    "Kiro translator does not support {}",
                    input.endpoint.label()
                ),
            );
        }
        if input.endpoint == ProviderEndpoint::ChatCompletions {
            return match kiro_provider_core_chat_completions_request_body(&input.body) {
                Ok(body) => ProviderTransformResult::degraded(
                    self.provider(),
                    input.endpoint,
                    self.client_wire_format(),
                    self.upstream_wire_format(),
                    body,
                    "Kiro chat completions are translated to the Responses-style request surface",
                    kiro_degraded_details(
                        input.endpoint,
                        self.client_wire_format(),
                        self.upstream_wire_format(),
                    ),
                ),
                Err(error) => ProviderTransformResult::rejected(
                    self.provider(),
                    input.endpoint,
                    self.client_wire_format(),
                    self.upstream_wire_format(),
                    error.message,
                )
                .with_metadata("error_code", Value::String(error.code)),
            };
        }
        ProviderTransformResult::lossless(
            self.provider(),
            input.endpoint,
            self.client_wire_format(),
            self.upstream_wire_format(),
            input.body,
        )
    }

    fn transform_response(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        if !kiro_supported_endpoint(input.endpoint) {
            return ProviderTransformResult::unsupported(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                format!(
                    "Kiro translator does not support {}",
                    input.endpoint.label()
                ),
            );
        }
        if input.endpoint == ProviderEndpoint::ChatCompletions {
            let response = match serde_json::from_slice::<Value>(&input.body) {
                Ok(response) => response,
                Err(_) => {
                    return ProviderTransformResult::rejected(
                        self.provider(),
                        input.endpoint,
                        self.upstream_wire_format(),
                        self.client_wire_format(),
                        "Kiro chat completions response body must be valid JSON",
                    )
                    .with_metadata("error_code", Value::String("invalid_json".to_string()));
                }
            };
            if !response.is_object() {
                return ProviderTransformResult::rejected(
                    self.provider(),
                    input.endpoint,
                    self.upstream_wire_format(),
                    self.client_wire_format(),
                    "Kiro chat completions response body must be a JSON object",
                )
                .with_metadata(
                    "error_code",
                    Value::String("invalid_response_body".to_string()),
                );
            }
            let body = match serde_json::to_vec(
                &kiro_provider_core_chat_completion_value_from_response(&response, 0),
            ) {
                Ok(body) => body,
                Err(_) => {
                    return ProviderTransformResult::rejected(
                        self.provider(),
                        input.endpoint,
                        self.upstream_wire_format(),
                        self.client_wire_format(),
                        "failed to serialize rewritten Kiro chat completions response body",
                    )
                    .with_metadata(
                        "error_code",
                        Value::String("invalid_response_body".to_string()),
                    );
                }
            };
            return ProviderTransformResult::degraded(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                body,
                "Kiro Responses-style output is translated to chat completions response shape",
                kiro_degraded_details(
                    input.endpoint,
                    self.upstream_wire_format(),
                    self.client_wire_format(),
                ),
            );
        }
        ProviderTransformResult::lossless(
            self.provider(),
            input.endpoint,
            self.upstream_wire_format(),
            self.client_wire_format(),
            input.body,
        )
    }

    fn transform_stream_event(&self, input: ProviderTransformInput) -> ProviderTransformResult {
        if !kiro_supported_endpoint(input.endpoint) {
            return ProviderTransformResult::unsupported(
                self.provider(),
                input.endpoint,
                self.upstream_wire_format(),
                self.client_wire_format(),
                format!(
                    "Kiro translator does not support {}",
                    input.endpoint.label()
                ),
            );
        }
        ProviderTransformResult::lossless(
            self.provider(),
            input.endpoint,
            self.upstream_wire_format(),
            self.client_wire_format(),
            input.body,
        )
    }
}

fn kiro_supported_endpoint(endpoint: ProviderEndpoint) -> bool {
    provider_supported_endpoints(ProviderId::Kiro).contains(&endpoint)
}

fn kiro_degraded_details(
    endpoint: ProviderEndpoint,
    from_format: ProviderWireFormat,
    to_format: ProviderWireFormat,
) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "endpoint".to_string(),
            Value::String(endpoint.label().to_string()),
        ),
        (
            "from_format".to_string(),
            Value::String(from_format.label().to_string()),
        ),
        (
            "to_format".to_string(),
            Value::String(to_format.label().to_string()),
        ),
    ])
}

#[cfg(test)]
#[path = "kiro/tests.rs"]
mod tests;
