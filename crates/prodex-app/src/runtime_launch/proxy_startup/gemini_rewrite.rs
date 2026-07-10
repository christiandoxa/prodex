use super::deepseek_rewrite::{
    RuntimeDeepSeekConversationStore, runtime_deepseek_store_conversation,
};
#[cfg(test)]
use super::gemini_thought_signatures::runtime_gemini_harden_tool_call_thought_signatures;
use super::provider_bridge::{
    RuntimeProviderBridgeKind, runtime_provider_log_response_conformance,
    runtime_provider_response_conformance_result,
};
use crate::RuntimeHeapTrimmedBufferedResponseParts;
use anyhow::{Context, Result};
use prodex_provider_core::{
    gemini_provider_core_buffered_responses_value, gemini_provider_core_chat_assistant_messages,
    gemini_provider_core_normalized_response_value,
    gemini_provider_core_response_terminal_without_history,
};
use std::io::Read;
use std::path::PathBuf;

#[path = "gemini_request.rs"]
mod gemini_request;
#[path = "gemini_request_extensions.rs"]
mod gemini_request_extensions;
#[path = "gemini_request_io.rs"]
mod gemini_request_io;
#[path = "gemini_request_policy.rs"]
mod gemini_request_policy;
#[path = "gemini_request_session.rs"]
mod gemini_request_session;
#[path = "gemini_request_tool_output.rs"]
mod gemini_request_tool_output;

pub(super) use gemini_request::{
    runtime_gemini_blocked_tool_call_message, runtime_gemini_generate_request_body,
};

#[allow(dead_code)]
pub(in super::super) fn runtime_gemini_responses_value_from_generate_value(
    value: &serde_json::Value,
    request_id: u64,
) -> serde_json::Value {
    prodex_provider_core::gemini_provider_core_runtime_responses_value(
        value,
        request_id,
        prodex_provider_core::provider_core_chat_compatible_created_at(),
        prodex_runtime_gemini::GEMINI_DEFAULT_MODEL,
        runtime_gemini_blocked_tool_call_message,
    )
}

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
    runtime_shared: &crate::RuntimeRotationProxyShared,
) -> Result<RuntimeHeapTrimmedBufferedResponseParts> {
    let mut body = Vec::new();
    response
        .read_to_end(&mut body)
        .context("failed to read Gemini response body")?;
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
    let value = gemini_provider_core_normalized_response_value(&value);
    let response = gemini_provider_core_buffered_responses_value(
        value.as_ref(),
        request_id,
        translated.as_ref(),
        runtime_gemini_blocked_tool_call_message,
    );
    if !gemini_provider_core_response_terminal_without_history(&response)
        && let Some(response_id) = response.get("id").and_then(serde_json::Value::as_str)
    {
        runtime_deepseek_store_conversation(
            conversations,
            response_id,
            conversation_messages,
            gemini_provider_core_chat_assistant_messages(&value, request_id, |name, args| {
                runtime_gemini_blocked_tool_call_message(name, args)
            }),
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

pub(super) fn runtime_gemini_native_request_body(
    body: &[u8],
    auth: &RuntimeGeminiAuth,
) -> Result<Vec<u8>> {
    prodex_provider_core::gemini_provider_core_native_request_body_with_project(
        body,
        runtime_gemini_project_id(auth),
    )
    .context("failed to serialize Gemini native request JSON")
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
