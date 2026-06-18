use super::super::gemini_rewrite::{
    runtime_gemini_finish_reason, runtime_gemini_finish_reason_retryable_invalid,
    runtime_gemini_internal_instruction_corpus, runtime_gemini_media_content_item_from_part,
    runtime_gemini_normalized_response_value, runtime_gemini_prompt_feedback_failure,
    runtime_gemini_text_echoes_internal_instruction, runtime_gemini_text_from_special_part,
    runtime_gemini_visible_text_from_part,
};
use anyhow::{Context, Result};
use std::io::Read;

const RUNTIME_GEMINI_PRECOMMIT_PEEK_LIMIT: usize = 64 * 1024;

pub(super) enum RuntimeGeminiPrecommitPeek {
    Committed {
        response: reqwest::blocking::Response,
        prefix: Vec<u8>,
    },
    RetryableInvalid {
        response: reqwest::blocking::Response,
        prefix: Vec<u8>,
        reason: String,
    },
}

#[derive(Default)]
pub(super) struct RuntimeGeminiPrecommitProbe {
    pub(super) visible_output: bool,
    pub(super) reasoning_output: bool,
    pub(super) internal_instruction_corpus: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RuntimeGeminiPrecommitDecision {
    Continue,
    Commit,
    RetryableInvalid(String),
}

pub(super) fn runtime_gemini_response_is_sse(response: &reqwest::blocking::Response) -> bool {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().contains("text/event-stream"))
}

pub(super) fn runtime_gemini_peek_stream_for_retry(
    mut response: reqwest::blocking::Response,
    conversation_messages: &[serde_json::Value],
) -> Result<RuntimeGeminiPrecommitPeek> {
    let mut prefix = Vec::new();
    let mut line = Vec::new();
    let mut data_lines = Vec::new();
    let mut probe = RuntimeGeminiPrecommitProbe {
        internal_instruction_corpus: runtime_gemini_internal_instruction_corpus(
            conversation_messages,
        ),
        ..RuntimeGeminiPrecommitProbe::default()
    };
    let mut byte = [0_u8; 1];

    loop {
        let read = response
            .read(&mut byte)
            .context("failed to read Gemini stream precommit prefix")?;
        if read == 0 {
            if !line.is_empty() {
                let decision =
                    runtime_gemini_precommit_process_line(&line, &mut data_lines, &mut probe);
                if let RuntimeGeminiPrecommitDecision::RetryableInvalid(reason) = decision {
                    return Ok(RuntimeGeminiPrecommitPeek::RetryableInvalid {
                        response,
                        prefix,
                        reason,
                    });
                }
                line.clear();
            }
            if !data_lines.is_empty() {
                match runtime_gemini_precommit_decision_for_data_lines(&data_lines, &mut probe) {
                    RuntimeGeminiPrecommitDecision::Commit => {
                        return Ok(RuntimeGeminiPrecommitPeek::Committed { response, prefix });
                    }
                    RuntimeGeminiPrecommitDecision::RetryableInvalid(reason) => {
                        return Ok(RuntimeGeminiPrecommitPeek::RetryableInvalid {
                            response,
                            prefix,
                            reason,
                        });
                    }
                    RuntimeGeminiPrecommitDecision::Continue => {}
                }
            }
            let reason = if probe.visible_output || probe.reasoning_output {
                return Ok(RuntimeGeminiPrecommitPeek::Committed { response, prefix });
            } else {
                "gemini_empty_response".to_string()
            };
            return Ok(RuntimeGeminiPrecommitPeek::RetryableInvalid {
                response,
                prefix,
                reason,
            });
        }

        prefix.push(byte[0]);
        line.push(byte[0]);
        if prefix.len() >= RUNTIME_GEMINI_PRECOMMIT_PEEK_LIMIT {
            return Ok(RuntimeGeminiPrecommitPeek::Committed { response, prefix });
        }
        if byte[0] != b'\n' {
            continue;
        }

        match runtime_gemini_precommit_process_line(&line, &mut data_lines, &mut probe) {
            RuntimeGeminiPrecommitDecision::Commit => {
                return Ok(RuntimeGeminiPrecommitPeek::Committed { response, prefix });
            }
            RuntimeGeminiPrecommitDecision::RetryableInvalid(reason) => {
                return Ok(RuntimeGeminiPrecommitPeek::RetryableInvalid {
                    response,
                    prefix,
                    reason,
                });
            }
            RuntimeGeminiPrecommitDecision::Continue => {}
        }
        line.clear();
    }
}

fn runtime_gemini_precommit_process_line(
    line: &[u8],
    data_lines: &mut Vec<String>,
    probe: &mut RuntimeGeminiPrecommitProbe,
) -> RuntimeGeminiPrecommitDecision {
    let line = String::from_utf8_lossy(line);
    let line = line.trim_end_matches(['\r', '\n']);
    if line.trim().is_empty() {
        if data_lines.is_empty() {
            return RuntimeGeminiPrecommitDecision::Continue;
        }
        let decision = runtime_gemini_precommit_decision_for_data_lines(data_lines, probe);
        data_lines.clear();
        return decision;
    }
    let Some(data) = line.strip_prefix("data:") else {
        return RuntimeGeminiPrecommitDecision::Continue;
    };
    data_lines.push(data.trim_start().to_string());
    RuntimeGeminiPrecommitDecision::Continue
}

pub(super) fn runtime_gemini_precommit_decision_for_data_lines(
    data_lines: &[String],
    probe: &mut RuntimeGeminiPrecommitProbe,
) -> RuntimeGeminiPrecommitDecision {
    let data = data_lines.join("\n");
    let trimmed = data.trim();
    if trimmed == "[DONE]" {
        return if probe.visible_output || probe.reasoning_output {
            RuntimeGeminiPrecommitDecision::Commit
        } else {
            RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
        };
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return runtime_gemini_precommit_decision_for_value(&value, probe);
    }
    let mut parsed_any = false;
    for line in data_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        parsed_any = true;
        match runtime_gemini_precommit_decision_for_value(&value, probe) {
            RuntimeGeminiPrecommitDecision::Continue => {}
            decision => return decision,
        }
    }
    if parsed_any {
        RuntimeGeminiPrecommitDecision::Continue
    } else {
        RuntimeGeminiPrecommitDecision::Commit
    }
}

fn runtime_gemini_precommit_decision_for_value(
    value: &serde_json::Value,
    probe: &mut RuntimeGeminiPrecommitProbe,
) -> RuntimeGeminiPrecommitDecision {
    let value = runtime_gemini_normalized_response_value(value);
    let value = value.as_ref();
    if value.get("error").is_some() || runtime_gemini_prompt_feedback_failure(value).is_some() {
        return RuntimeGeminiPrecommitDecision::Commit;
    }
    if runtime_gemini_precommit_has_grounding(value) {
        probe.visible_output = true;
        return RuntimeGeminiPrecommitDecision::Commit;
    }
    runtime_gemini_precommit_apply_parts(value, probe);
    if probe.visible_output {
        return RuntimeGeminiPrecommitDecision::Commit;
    }
    let Some(reason) = runtime_gemini_finish_reason(value) else {
        return RuntimeGeminiPrecommitDecision::Continue;
    };
    if runtime_gemini_finish_reason_retryable_invalid(&reason) {
        return RuntimeGeminiPrecommitDecision::RetryableInvalid(reason);
    }
    match reason.as_str() {
        "STOP" if !probe.reasoning_output => {
            RuntimeGeminiPrecommitDecision::RetryableInvalid("gemini_empty_response".to_string())
        }
        _ => RuntimeGeminiPrecommitDecision::Commit,
    }
}

fn runtime_gemini_precommit_apply_parts(
    value: &serde_json::Value,
    probe: &mut RuntimeGeminiPrecommitProbe,
) {
    let Some(parts) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    for part in parts {
        if part.get("functionCall").is_some()
            || runtime_gemini_media_content_item_from_part(part).is_some()
            || runtime_gemini_text_from_special_part(part).is_some()
        {
            probe.visible_output = true;
            return;
        }
        let Some(text) = part.get("text").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        if part
            .get("thought")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            probe.reasoning_output = true;
        } else if runtime_gemini_visible_text_from_part(part).is_none()
            || runtime_gemini_text_echoes_internal_instruction(
                text,
                &probe.internal_instruction_corpus,
            )
        {
            continue;
        } else {
            probe.visible_output = true;
            return;
        }
    }
}

fn runtime_gemini_precommit_has_grounding(value: &serde_json::Value) -> bool {
    value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("groundingMetadata"))
        .is_some_and(|metadata| {
            metadata
                .get("webSearchQueries")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|queries| !queries.is_empty())
                || metadata
                    .get("groundingChunks")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|chunks| !chunks.is_empty())
        })
}
