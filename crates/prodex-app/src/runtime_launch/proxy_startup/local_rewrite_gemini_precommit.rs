use anyhow::{Context, Result};
use prodex_provider_core::gemini_provider_core_internal_instruction_corpus;
pub(super) use prodex_provider_core::{
    GeminiProviderCorePrecommitDecision as RuntimeGeminiPrecommitDecision,
    GeminiProviderCorePrecommitProbe as RuntimeGeminiPrecommitProbe,
    gemini_provider_core_precommit_decision_for_data_lines as runtime_gemini_precommit_decision_for_data_lines,
};
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
        internal_instruction_corpus: gemini_provider_core_internal_instruction_corpus(
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
