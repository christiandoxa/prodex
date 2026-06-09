use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_store_conversation,
};
#[cfg(test)]
use super::gemini_thought_signatures::runtime_gemini_harden_tool_call_thought_signatures;
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use anyhow::{Context, Result};
use std::io::Read;
use std::path::PathBuf;

#[path = "gemini_apply_patch.rs"]
mod gemini_apply_patch;
#[path = "gemini_command_output.rs"]
mod gemini_command_output;
#[path = "gemini_history_hardening.rs"]
mod gemini_history_hardening;
#[path = "gemini_request.rs"]
mod gemini_request;
#[path = "gemini_response.rs"]
mod gemini_response;
#[path = "gemini_schema.rs"]
mod gemini_schema;

pub(super) use gemini_request::{
    runtime_gemini_blocked_tool_call_message, runtime_gemini_generate_request_body,
    runtime_gemini_request_body_without_tool,
};
pub(super) use gemini_response::{
    runtime_gemini_chat_assistant_messages_from_generate_value, runtime_gemini_citation_text,
    runtime_gemini_custom_tool_call_item, runtime_gemini_custom_tool_input_from_arguments,
    runtime_gemini_finish_reason, runtime_gemini_finish_reason_failure,
    runtime_gemini_finish_reason_incomplete, runtime_gemini_finish_reason_retryable_invalid,
    runtime_gemini_image_generation_call_item_from_part,
    runtime_gemini_internal_instruction_corpus, runtime_gemini_media_content_item_from_part,
    runtime_gemini_normalized_response_value, runtime_gemini_prompt_feedback_failure,
    runtime_gemini_response_metadata, runtime_gemini_responses_usage,
    runtime_gemini_responses_value_from_generate_value,
    runtime_gemini_text_echoes_internal_instruction, runtime_gemini_text_from_special_part,
    runtime_gemini_visible_text_from_part, runtime_gemini_web_search_call_from_grounding,
};

#[derive(Clone)]
pub(crate) enum RuntimeGeminiAuth {
    ApiKey {
        api_key: String,
    },
    OAuth {
        access_token: String,
        project_id: Option<String>,
    },
}

#[derive(Clone)]
pub(crate) enum RuntimeGeminiProviderAuth {
    ApiKeys {
        api_keys: Vec<String>,
    },
    OAuthProfiles {
        profiles: Vec<RuntimeGeminiOAuthProfileAuth>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeGeminiOAuthProfileAuth {
    pub(crate) profile_name: String,
    pub(crate) codex_home: PathBuf,
    pub(crate) email: Option<String>,
    pub(crate) access_token: String,
    pub(crate) project_id: Option<String>,
}

impl RuntimeGeminiOAuthProfileAuth {
    pub(crate) fn auth(&self) -> RuntimeGeminiAuth {
        RuntimeGeminiAuth::OAuth {
            access_token: self.access_token.clone(),
            project_id: self.project_id.clone(),
        }
    }
}

pub(super) struct RuntimeGeminiTranslatedRequest {
    pub(super) body: Vec<u8>,
    pub(super) messages: Vec<serde_json::Value>,
    pub(super) model: String,
    pub(super) stream: bool,
}

pub(super) fn runtime_gemini_generate_buffered_response_parts(
    status: u16,
    mut response: reqwest::blocking::Response,
    request_id: u64,
    conversation_messages: Vec<serde_json::Value>,
    conversations: &RuntimeDeepSeekConversationStore,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let mut body = Vec::new();
    response
        .read_to_end(&mut body)
        .context("failed to read Gemini response body")?;
    let value: serde_json::Value =
        serde_json::from_slice(&body).context("failed to parse Gemini response JSON")?;
    let value = runtime_gemini_normalized_response_value(&value);
    let response = runtime_gemini_responses_value_from_generate_value(&value, request_id);
    let terminal_without_history = matches!(
        response.get("status").and_then(serde_json::Value::as_str),
        Some("failed" | "incomplete")
    ) || response.get("error").is_some();
    if !terminal_without_history
        && let Some(response_id) = response.get("id").and_then(serde_json::Value::as_str)
    {
        runtime_deepseek_store_conversation(
            conversations,
            response_id,
            conversation_messages,
            runtime_gemini_chat_assistant_messages_from_generate_value(&value, request_id),
        );
    }
    let body = serde_json::to_vec(&response).context("failed to serialize Responses JSON")?;
    Ok(RuntimeHeapTrimmedBufferedResponseParts {
        status,
        headers: vec![(
            "content-type".to_string(),
            b"application/json; charset=utf-8".to_vec(),
        )],
        body: body.into(),
    })
}

pub(super) fn runtime_gemini_upstream_url(
    base_url: &str,
    auth: &RuntimeGeminiAuth,
    model: &str,
    stream: bool,
) -> String {
    let method = if stream {
        "streamGenerateContent"
    } else {
        "generateContent"
    };
    match auth {
        RuntimeGeminiAuth::ApiKey { .. } => {
            let model_path = if model.starts_with("models/") {
                model.to_string()
            } else {
                format!("models/{model}")
            };
            let suffix = if stream {
                format!("/{model_path}:{method}?alt=sse")
            } else {
                format!("/{model_path}:{method}")
            };
            format!("{}{}", base_url.trim_end_matches('/'), suffix)
        }
        RuntimeGeminiAuth::OAuth { .. } => {
            let mut url = format!("{}:{method}", crate::gemini_code_assist_endpoint());
            if stream {
                url.push_str("?alt=sse");
            }
            url
        }
    }
}

pub(super) fn runtime_gemini_native_upstream_url(
    base_url: &str,
    auth: &RuntimeGeminiAuth,
    path_and_query: &str,
) -> String {
    match auth {
        RuntimeGeminiAuth::ApiKey { .. } => format!(
            "{}{}",
            base_url.trim_end_matches('/'),
            path_and_query
                .strip_prefix("/v1beta")
                .unwrap_or(path_and_query)
        ),
        RuntimeGeminiAuth::OAuth { .. } => {
            let endpoint = crate::gemini_code_assist_endpoint();
            let endpoint_root = endpoint
                .strip_suffix("/v1internal")
                .unwrap_or(endpoint.as_str());
            format!("{}{}", endpoint_root.trim_end_matches('/'), path_and_query)
        }
    }
}

pub(super) fn runtime_gemini_request_upstream_url(
    base_url: &str,
    auth: &RuntimeGeminiAuth,
    path_and_query: &str,
    model: &str,
    stream: bool,
    responses_route: bool,
) -> String {
    if responses_route {
        runtime_gemini_upstream_url(base_url, auth, model, stream)
    } else {
        runtime_gemini_native_upstream_url(base_url, auth, path_and_query)
    }
}

pub(super) fn runtime_gemini_project_id(auth: &RuntimeGeminiAuth) -> Option<&str> {
    match auth {
        RuntimeGeminiAuth::ApiKey { .. } => None,
        RuntimeGeminiAuth::OAuth { project_id, .. } => project_id.as_deref(),
    }
}

#[cfg(test)]
#[path = "gemini_rewrite_custom_tool_tests.rs"]
mod gemini_rewrite_custom_tool_tests;

#[cfg(test)]
mod native_url_tests {
    use super::*;

    #[test]
    fn native_code_assist_url_preserves_method_and_query() {
        let auth = RuntimeGeminiAuth::OAuth {
            access_token: "token".to_string(),
            project_id: None,
        };
        assert_eq!(
            runtime_gemini_native_upstream_url(
                "https://generativelanguage.googleapis.com/v1beta",
                &auth,
                "/v1internal:streamGenerateContent?alt=sse",
            ),
            "https://cloudcode-pa.googleapis.com/v1internal:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn native_api_key_url_preserves_model_path() {
        let auth = RuntimeGeminiAuth::ApiKey {
            api_key: "key".to_string(),
        };
        assert_eq!(
            runtime_gemini_native_upstream_url(
                "https://generativelanguage.googleapis.com/v1beta",
                &auth,
                "/v1beta/models/gemini-test:generateContent",
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-test:generateContent"
        );
    }
}

#[cfg(test)]
#[path = "gemini_rewrite_command_output_tests.rs"]
mod gemini_rewrite_command_output_tests;

#[cfg(test)]
#[path = "gemini_rewrite_history_tests.rs"]
mod gemini_rewrite_history_tests;

#[cfg(test)]
#[path = "gemini_rewrite_test_support.rs"]
mod gemini_rewrite_test_support;

#[cfg(test)]
#[path = "gemini_rewrite_tests.rs"]
mod gemini_rewrite_tests;
