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
#[path = "gemini_request.rs"]
mod gemini_request;
#[path = "gemini_response.rs"]
mod gemini_response;
#[path = "gemini_schema.rs"]
mod gemini_schema;

pub(super) use gemini_request::{
    runtime_gemini_generate_request_body, runtime_gemini_request_body_without_google_search,
};
pub(super) use gemini_response::{
    runtime_gemini_chat_assistant_messages_from_generate_value,
    runtime_gemini_custom_tool_call_item, runtime_gemini_custom_tool_input_from_arguments,
    runtime_gemini_normalized_response_value, runtime_gemini_responses_usage,
    runtime_gemini_responses_value_from_generate_value,
    runtime_gemini_web_search_call_from_grounding,
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
    if let Some(response_id) = response.get("id").and_then(serde_json::Value::as_str) {
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
#[path = "gemini_rewrite_tests.rs"]
mod gemini_rewrite_tests;
