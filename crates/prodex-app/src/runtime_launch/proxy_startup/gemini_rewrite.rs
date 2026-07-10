use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_store_conversation,
};
#[cfg(test)]
use super::gemini_thought_signatures::runtime_gemini_harden_tool_call_thought_signatures;
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_log_response_conformance,
    runtime_provider_response_conformance_result,
};
use crate::{
    RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, RuntimeHeapTrimmedBufferedResponseParts,
    read_runtime_buffered_response_body_with_limit,
};
use anyhow::{Context, Result};
use prodex_provider_core::ProviderTransformLoss;
use std::path::PathBuf;

#[path = "gemini_apply_patch.rs"]
mod gemini_apply_patch;
#[path = "gemini_command_output.rs"]
mod gemini_command_output;
#[path = "gemini_history_hardening.rs"]
mod gemini_history_hardening;
#[path = "gemini_request.rs"]
mod gemini_request;
#[path = "gemini_request_extensions.rs"]
mod gemini_request_extensions;
#[path = "gemini_request_generation.rs"]
mod gemini_request_generation;
#[path = "gemini_request_io.rs"]
mod gemini_request_io;
#[path = "gemini_request_media.rs"]
mod gemini_request_media;
#[path = "gemini_request_policy.rs"]
mod gemini_request_policy;
#[path = "gemini_request_session.rs"]
mod gemini_request_session;
#[path = "gemini_request_tool_output.rs"]
mod gemini_request_tool_output;
#[path = "gemini_response.rs"]
mod gemini_response;
#[path = "gemini_schema.rs"]
mod gemini_schema;

#[cfg(test)]
pub(super) use gemini_request::runtime_gemini_generate_request_body;
pub(super) use gemini_request::{
    runtime_gemini_blocked_tool_call_message,
    runtime_gemini_generate_request_body_with_local_file_access,
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
    response: reqwest::blocking::Response,
    request_id: u64,
    conversation_messages: Vec<serde_json::Value>,
    conversations: &RuntimeDeepSeekConversationStore,
    runtime_shared: &crate::RuntimeRotationProxyShared,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let body = read_runtime_buffered_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read Gemini response body",
    )?;
    let value: serde_json::Value =
        serde_json::from_slice(&body).context("failed to parse Gemini response JSON")?;
    let translated = runtime_provider_response_conformance_result(
        RuntimeProviderBridgeKind::Gemini,
        status,
        &body,
    );
    if let Some(result) = translated.as_ref() {
        runtime_provider_log_response_conformance(
            runtime_shared,
            request_id,
            RuntimeProviderBridgeKind::Gemini,
            result,
        );
    }
    let value = runtime_gemini_normalized_response_value(&value);
    let mut response = translated
        .as_ref()
        .filter(|_| runtime_gemini_provider_core_simple_response(&value))
        .and_then(|result| match result.loss {
            ProviderTransformLoss::Lossless | ProviderTransformLoss::DegradedButSafe { .. } => {
                result.body.as_ref()
            }
            ProviderTransformLoss::Rejected { .. }
            | ProviderTransformLoss::UnsupportedUpstream { .. } => None,
        })
        .and_then(|body| serde_json::from_slice::<serde_json::Value>(body).ok())
        .unwrap_or_else(|| runtime_gemini_responses_value_from_generate_value(&value, request_id));
    if response.get("created_at").is_none() {
        response["created_at"] = serde_json::Value::Number(serde_json::Number::from(
            crate::runtime_launch::proxy_startup::deepseek_rewrite::runtime_deepseek_created_at(),
        ));
    }
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

fn runtime_gemini_provider_core_simple_response(value: &serde_json::Value) -> bool {
    let Some(candidate) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
    else {
        return true;
    };
    let Some(parts) = candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    else {
        return true;
    };
    runtime_gemini_provider_core_message_only_parts(parts)
        || runtime_gemini_provider_core_function_call_only_parts(parts)
}

fn runtime_gemini_provider_core_message_only_parts(parts: &[serde_json::Value]) -> bool {
    parts
        .iter()
        .all(runtime_gemini_provider_core_message_only_part)
}

fn runtime_gemini_provider_core_message_only_part(part: &serde_json::Value) -> bool {
    if part.get("functionCall").is_some() {
        return false;
    }
    if part
        .get("text")
        .and_then(serde_json::Value::as_str)
        .is_some()
    {
        return part.as_object().is_some_and(|object| {
            object
                .keys()
                .all(|key| matches!(key.as_str(), "text" | "thought"))
        });
    }
    part.as_object().is_some_and(|object| {
        object.keys().all(|key| {
            matches!(
                key.as_str(),
                "inlineData"
                    | "inline_data"
                    | "fileData"
                    | "file_data"
                    | "executableCode"
                    | "codeExecutionResult"
                    | "videoMetadata"
            )
        })
    })
}

fn runtime_gemini_provider_core_function_call_only_parts(parts: &[serde_json::Value]) -> bool {
    !parts.is_empty()
        && parts
            .iter()
            .all(runtime_gemini_provider_core_function_call_part)
}

fn runtime_gemini_provider_core_function_call_part(part: &serde_json::Value) -> bool {
    if part.get("text").is_some()
        && !part
            .get("thought")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    {
        return false;
    }
    let Some(function_call) = part
        .get("functionCall")
        .and_then(serde_json::Value::as_object)
    else {
        return part.as_object().is_some_and(|object| {
            object
                .keys()
                .all(|key| matches!(key.as_str(), "text" | "thought"))
        });
    };
    let Some(name) = function_call
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
    else {
        return false;
    };
    let args = function_call
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    runtime_gemini_blocked_tool_call_message(name, &args).is_none()
        && part.as_object().is_some_and(|object| {
            object
                .keys()
                .all(|key| matches!(key.as_str(), "functionCall" | "thoughtSignature"))
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
    let path_and_query = runtime_proxy_crate::runtime_escape_url_path_dot_segments(path_and_query);
    let path_and_query = path_and_query.as_ref();
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

pub(super) fn runtime_gemini_native_request_body(
    body: &[u8],
    auth: &RuntimeGeminiAuth,
) -> Result<Vec<u8>> {
    let Some(project_id) = runtime_gemini_project_id(auth) else {
        return Ok(body.to_vec());
    };
    let mut value = match serde_json::from_slice::<serde_json::Value>(body) {
        Ok(value) => value,
        Err(_) => return Ok(body.to_vec()),
    };
    runtime_gemini_stamp_native_project(&mut value, project_id);
    serde_json::to_vec(&value).context("failed to serialize Gemini native request JSON")
}

fn runtime_gemini_stamp_native_project(value: &mut serde_json::Value, project_id: &str) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let project = serde_json::Value::String(project_id.to_string());
    if object.contains_key("project") {
        object.insert("project".to_string(), project.clone());
    }
    if object.contains_key("projectId") {
        object.insert("projectId".to_string(), project.clone());
    }
    if object.contains_key("cloudaicompanionProject") {
        object.insert("cloudaicompanionProject".to_string(), project.clone());
    }
    runtime_gemini_stamp_native_metadata_project(object.get_mut("metadata"), project_id);
    if let Some(request) = object.get_mut("request") {
        runtime_gemini_stamp_native_project(request, project_id);
    }
}

fn runtime_gemini_stamp_native_metadata_project(
    value: Option<&mut serde_json::Value>,
    project_id: &str,
) {
    let Some(metadata) = value.and_then(serde_json::Value::as_object_mut) else {
        return;
    };
    if metadata.contains_key("duetProject") {
        metadata.insert(
            "duetProject".to_string(),
            serde_json::Value::String(project_id.to_string()),
        );
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

    #[test]
    fn native_oauth_body_uses_selected_profile_project() {
        let auth = RuntimeGeminiAuth::OAuth {
            access_token: "token".to_string(),
            project_id: Some("project-selected".to_string()),
        };
        let body = serde_json::to_vec(&serde_json::json!({
            "project": "project-from-env",
            "cloudaicompanionProject": "project-from-env",
            "metadata": {"duetProject": "project-from-env"},
            "request": {
                "project": "project-from-env",
                "metadata": {"duetProject": "project-from-env"}
            }
        }))
        .unwrap();

        let rewritten = runtime_gemini_native_request_body(&body, &auth).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&rewritten).unwrap();

        assert_eq!(value["project"], "project-selected");
        assert_eq!(value["cloudaicompanionProject"], "project-selected");
        assert_eq!(value["metadata"]["duetProject"], "project-selected");
        assert_eq!(value["request"]["project"], "project-selected");
        assert_eq!(
            value["request"]["metadata"]["duetProject"],
            "project-selected"
        );
    }
}

#[cfg(test)]
#[path = "gemini_rewrite_command_output_tests.rs"]
mod gemini_rewrite_command_output_tests;

#[cfg(test)]
#[path = "gemini_rewrite_context_tests.rs"]
mod gemini_rewrite_context_tests;

#[cfg(test)]
#[path = "gemini_rewrite_history_tests.rs"]
mod gemini_rewrite_history_tests;

#[cfg(test)]
#[path = "gemini_rewrite_leak_tests.rs"]
mod gemini_rewrite_leak_tests;

#[cfg(test)]
#[path = "gemini_rewrite_media_tests.rs"]
mod gemini_rewrite_media_tests;

#[cfg(test)]
#[path = "gemini_rewrite_optional_tool_tests.rs"]
mod gemini_rewrite_optional_tool_tests;

#[cfg(test)]
#[path = "gemini_rewrite_status_tests.rs"]
mod gemini_rewrite_status_tests;

#[cfg(test)]
#[path = "gemini_rewrite_thought_and_native_tool_tests.rs"]
mod gemini_rewrite_thought_and_native_tool_tests;

#[cfg(test)]
#[path = "gemini_rewrite_tool_output_history_tests.rs"]
mod gemini_rewrite_tool_output_history_tests;

#[cfg(test)]
#[path = "gemini_rewrite_test_support.rs"]
mod gemini_rewrite_test_support;

#[cfg(test)]
#[path = "gemini_rewrite_tests.rs"]
mod gemini_rewrite_tests;
