use prodex_provider_core::{
    ProviderConformanceExpectedErrorClass, ProviderConformanceExpectedLoss,
    ProviderConformanceOperation, ProviderEndpoint, ProviderErrorClass, ProviderId,
    ProviderTransformInput, ProviderTransformLoss, provider_conformance_cases, provider_translator,
};

fn input(case: &prodex_provider_core::ProviderConformanceCase) -> ProviderTransformInput {
    let mut input = ProviderTransformInput::new(
        case.endpoint,
        serde_json::to_vec(&case.input_body).expect("fixture input serializes"),
    );
    input.model = case.model.clone();
    input.headers = case.input_headers.clone();
    input
}

#[test]
fn deepseek_request_output_items_preserve_alias_call_ids() {
    let request = serde_json::json!({
        "model": "deepseek-chat",
        "input": [
            {
                "type": "function_call",
                "id": "call_function_alias",
                "name": "grep",
                "arguments": {"pattern": "ProviderTranslator"}
            },
            {
                "type": "function_call_output",
                "tool_call_id": "call_function_alias",
                "output": {"match_count": 1}
            },
            {
                "type": "custom_tool_call",
                "id": "call_custom_alias",
                "name": "apply_patch",
                "input": "*** Begin Patch\n*** End Patch"
            },
            {
                "type": "custom_tool_call_output",
                "id": "call_custom_alias",
                "output": [{"type": "output_text", "text": "applied"}]
            },
            {
                "type": "mcp_call",
                "id": "call_mcp_alias",
                "name": "mcp__prodex_sqz__compress",
                "arguments": {"text": "large repeated content"}
            },
            {
                "type": "mcp_tool_result",
                "tool_call_id": "call_mcp_alias",
                "content": [{"type": "output_text", "text": "ref:abc123"}]
            }
        ]
    });
    let result =
        provider_translator(ProviderId::DeepSeek).transform_request(ProviderTransformInput::new(
            ProviderEndpoint::Responses,
            serde_json::to_vec(&request).unwrap(),
        ));
    let body: serde_json::Value = serde_json::from_slice(result.body.as_ref().unwrap()).unwrap();
    let messages = body["messages"].as_array().unwrap();

    assert_eq!(messages[1]["tool_call_id"], "call_function_alias");
    assert_eq!(messages[1]["content"], "{\"match_count\":1}");
    assert_eq!(messages[3]["tool_call_id"], "call_custom_alias");
    assert_eq!(messages[3]["content"], "applied");
    assert_eq!(messages[5]["tool_call_id"], "call_mcp_alias");
    assert_eq!(messages[5]["content"], "ref:abc123");
}

#[test]
fn v1_conformance_fixtures_cover_translated_gemini_and_deepseek_flows() {
    let cases = provider_conformance_cases();
    assert!(cases.len() >= 8);
    assert!(
        cases
            .iter()
            .any(|case| case.provider == ProviderId::DeepSeek)
    );
    assert!(cases.iter().any(|case| case.provider == ProviderId::Gemini));
}

#[test]
fn v1_conformance_cases_execute_expected_request_response_and_stream_shapes() {
    for case in provider_conformance_cases() {
        let translator = provider_translator(case.provider);
        let result = match case.operation {
            ProviderConformanceOperation::Request => translator.transform_request(input(case)),
            ProviderConformanceOperation::Response => translator.transform_response(input(case)),
            ProviderConformanceOperation::StreamEvent => {
                let body = case
                    .input_body
                    .as_str()
                    .expect("stream event fixture must be raw sse string")
                    .as_bytes()
                    .to_vec();
                translator.transform_stream_event(ProviderTransformInput {
                    endpoint: case.endpoint,
                    model: case.model.clone(),
                    headers: case.input_headers.clone(),
                    status: None,
                    body,
                })
            }
        };
        match case.expected_loss {
            ProviderConformanceExpectedLoss::Lossless => {
                assert!(
                    matches!(result.loss, ProviderTransformLoss::Lossless),
                    "{}",
                    case.name
                );
            }
            ProviderConformanceExpectedLoss::Degraded => assert!(
                matches!(result.loss, ProviderTransformLoss::DegradedButSafe { .. }),
                "{}",
                case.name
            ),
            ProviderConformanceExpectedLoss::Rejected => {
                assert!(
                    matches!(result.loss, ProviderTransformLoss::Rejected { .. }),
                    "{}",
                    case.name
                )
            }
            ProviderConformanceExpectedLoss::Unsupported => assert!(
                matches!(
                    result.loss,
                    ProviderTransformLoss::UnsupportedUpstream { .. }
                ),
                "{}",
                case.name
            ),
        }
        if let Some(expected) = &case.expected_body {
            let actual = result.body.as_ref().expect("body present");
            if case.operation == ProviderConformanceOperation::StreamEvent {
                assert_eq!(
                    String::from_utf8_lossy(actual),
                    expected.as_str().unwrap(),
                    "{}",
                    case.name
                );
            } else {
                let actual_json: serde_json::Value =
                    serde_json::from_slice(actual).expect("actual json");
                assert_eq!(actual_json, *expected, "{}", case.name);
            }
        }
        if let Some(expected_usage) = case.expected_usage {
            let body = if case.operation == ProviderConformanceOperation::Response {
                serde_json::to_vec(&case.input_body).unwrap()
            } else {
                serde_json::to_vec(case.expected_body.as_ref().unwrap()).unwrap()
            };
            assert_eq!(
                translator.extract_usage(&body),
                expected_usage,
                "{}",
                case.name
            );
        }
        if let Some(expected_error_class) = case.expected_error_class {
            let actual = translator.classify_error(
                case.error_status,
                case.error_code.as_deref(),
                case.error_text.as_deref(),
            );
            assert_eq!(
                actual.class,
                match expected_error_class {
                    ProviderConformanceExpectedErrorClass::Auth => ProviderErrorClass::Auth,
                    ProviderConformanceExpectedErrorClass::Quota => ProviderErrorClass::Quota,
                    ProviderConformanceExpectedErrorClass::RateLimit => {
                        ProviderErrorClass::RateLimit
                    }
                    ProviderConformanceExpectedErrorClass::Transient => {
                        ProviderErrorClass::Transient
                    }
                    ProviderConformanceExpectedErrorClass::NotFound => {
                        ProviderErrorClass::NotFound
                    }
                    ProviderConformanceExpectedErrorClass::Other => ProviderErrorClass::Other,
                },
                "{}",
                case.name
            );
            if let Some(expected_cooldown_ms) = case.expected_error_cooldown_ms {
                assert_eq!(actual.cooldown_ms, expected_cooldown_ms, "{}", case.name);
            }
        }
    }
}

#[test]
fn translated_providers_advertise_endpoint_support_limitations_explicitly() {
    for (provider, endpoint) in [
        (ProviderId::DeepSeek, ProviderEndpoint::Embeddings),
        (ProviderId::Gemini, ProviderEndpoint::Models),
    ] {
        let support = provider_translator(provider).supported_params(endpoint, "test");
        assert!(!support.supported);
        assert!(!support.unsupported.is_empty());
    }
}

#[test]
fn translated_providers_advertise_known_parameter_limitations_explicitly() {
    let deepseek = provider_translator(ProviderId::DeepSeek)
        .supported_params(ProviderEndpoint::Responses, "deepseek-chat");
    assert!(deepseek.supported);
    assert!(
        deepseek
            .unsupported
            .iter()
            .any(|reason| reason.field == "parallel_tool_calls=false")
    );
    assert!(
        deepseek
            .unsupported
            .iter()
            .any(|reason| reason.field == "web_search_options")
    );
    assert!(
        deepseek
            .unsupported
            .iter()
            .any(|reason| reason.field == "safety_identifier")
    );
    assert!(
        deepseek
            .unsupported
            .iter()
            .any(|reason| reason.field == "tools[type!=function]")
    );
    assert!(
        deepseek
            .unsupported
            .iter()
            .any(|reason| reason.field == "input[*].content[type!=text]")
    );

    let gemini = provider_translator(ProviderId::Gemini)
        .supported_params(ProviderEndpoint::Responses, "gemini-2.5-pro");
    assert!(gemini.supported);
    assert!(
        gemini
            .unsupported
            .iter()
            .any(|reason| reason.field == "input[*].content[type!=text]")
    );
    assert!(
        gemini
            .unsupported
            .iter()
            .any(|reason| reason.field == "response_format.type")
    );
}

#[test]
fn translated_providers_have_explicit_error_mapping_fixtures() {
    for provider in [ProviderId::DeepSeek, ProviderId::Gemini] {
        assert!(
            provider_conformance_cases()
                .iter()
                .any(|case| case.provider == provider && case.expected_error_class.is_some()),
            "missing error mapping fixture for {provider:?}"
        );
    }
}

#[test]
fn v1_translated_providers_have_explicit_non_lossless_fixtures() {
    for provider in [ProviderId::DeepSeek, ProviderId::Gemini] {
        assert!(
            provider_conformance_cases().iter().any(|case| {
                case.provider == provider
                    && !matches!(
                        case.expected_loss,
                        ProviderConformanceExpectedLoss::Lossless
                    )
            }),
            "missing non-lossless fixture for {provider:?}"
        );
    }
}

#[test]
fn translated_providers_have_explicit_objective_coverage_fixtures() {
    let cases = provider_conformance_cases();

    assert!(
        cases
            .iter()
            .any(|case| case.name == "deepseek-request-instructions-prepended")
    );
    assert!(
        cases
            .iter()
            .any(|case| case.name == "deepseek-request-json-schema-degrades")
    );
    assert!(
        cases
            .iter()
            .any(|case| case.name == "deepseek-request-response-format-type-rejected")
    );
    assert!(cases.iter().any(|case| case.name == "deepseek-request-assistant-tool-call-and-tool-output-history"));
    assert!(
        cases
            .iter()
            .any(|case| case.name == "deepseek-response-cache-and-tool-metadata")
    );
    assert!(
        cases
            .iter()
            .any(|case| case.name == "gemini-request-text-input")
    );
    assert!(
        cases
            .iter()
            .any(|case| case.name == "gemini-request-tool-schema-sanitized")
    );
    assert!(
        cases
            .iter()
            .any(|case| case.name == "gemini-request-response-format-type-rejected")
    );
    assert!(
        cases
            .iter()
            .any(|case| case.name == "gemini-request-multimodal-unsupported")
    );
    assert!(
        cases
            .iter()
            .any(|case| case.name == "gemini-response-generate-to-responses")
    );
}

#[test]
fn deepseek_request_fixture_preserves_continuation_metadata_explicitly() {
    let case = provider_conformance_cases()
        .iter()
        .find(|case| case.name == "deepseek-request-text-input")
        .expect("deepseek continuation fixture");
    let result = provider_translator(case.provider).transform_request(input(case));
    let continuation = result
        .metadata
        .get("continuation")
        .and_then(serde_json::Value::as_object)
        .expect("continuation metadata");
    assert_eq!(
        continuation
            .get("x-codex-turn-state")
            .and_then(serde_json::Value::as_str),
        Some("turn-state-a")
    );
    assert_eq!(
        continuation
            .get("session_id")
            .and_then(serde_json::Value::as_str),
        Some("sess-a")
    );
    assert_eq!(
        continuation
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str),
        Some("resp_1")
    );
}

#[test]
fn gemini_request_preserves_continuation_metadata_explicitly() {
    let case = provider_conformance_cases()
        .iter()
        .find(|case| case.name == "gemini-request-text-input")
        .expect("gemini continuation fixture");
    let result = provider_translator(case.provider).transform_request(input(case));
    let continuation = result
        .metadata
        .get("continuation")
        .and_then(serde_json::Value::as_object)
        .expect("continuation metadata");
    assert_eq!(
        continuation
            .get("x-codex-turn-state")
            .and_then(serde_json::Value::as_str),
        Some("turn-state-a")
    );
    assert_eq!(
        continuation
            .get("session_id")
            .and_then(serde_json::Value::as_str),
        Some("sess-a")
    );
    assert_eq!(
        continuation
            .get("previous_response_id")
            .and_then(serde_json::Value::as_str),
        Some("resp_1")
    );
}

#[test]
fn translated_provider_stream_fixtures_emit_canonical_responses_event_names() {
    let translated_responses_streams = provider_conformance_cases().iter().filter(|case| {
        matches!(case.provider, ProviderId::DeepSeek | ProviderId::Gemini)
            && case.endpoint == ProviderEndpoint::Responses
            && case.operation == ProviderConformanceOperation::StreamEvent
    });

    for case in translated_responses_streams {
        let expected = case
            .expected_body
            .as_ref()
            .and_then(serde_json::Value::as_str)
            .expect("stream fixture expected body");
        assert!(
            expected.starts_with("event: response."),
            "{} should emit canonical Responses event names",
            case.name
        );
        assert!(
            expected.contains("\ndata: {\"") || expected.contains("\r\ndata: {\""),
            "{} should emit SSE data payload",
            case.name
        );
    }
}

#[test]
fn responses_surface_has_request_response_and_stream_coverage_for_every_current_provider() {
    for provider in [
        ProviderId::OpenAi,
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
        ProviderId::Local,
    ] {
        let provider_cases: Vec<_> = provider_conformance_cases()
            .iter()
            .filter(|case| {
                case.provider == provider && case.endpoint == ProviderEndpoint::Responses
            })
            .collect();
        assert!(
            provider_cases
                .iter()
                .any(|case| case.operation == ProviderConformanceOperation::Request),
            "missing request fixture for {provider:?}"
        );
        assert!(
            provider_cases
                .iter()
                .any(|case| case.operation == ProviderConformanceOperation::Response),
            "missing response fixture for {provider:?}"
        );
        assert!(
            provider_cases
                .iter()
                .any(|case| case.operation == ProviderConformanceOperation::StreamEvent),
            "missing stream fixture for {provider:?}"
        );
    }
}

#[test]
fn passthrough_providers_have_non_responses_fixture_coverage_where_docs_claim_support() {
    for (provider, endpoint) in [
        (ProviderId::OpenAi, ProviderEndpoint::ChatCompletions),
        (ProviderId::OpenAi, ProviderEndpoint::Messages),
        (ProviderId::Local, ProviderEndpoint::ChatCompletions),
        (ProviderId::Local, ProviderEndpoint::Messages),
    ] {
        let provider_cases: Vec<_> = provider_conformance_cases()
            .iter()
            .filter(|case| case.provider == provider && case.endpoint == endpoint)
            .collect();
        assert!(
            provider_cases
                .iter()
                .any(|case| case.operation == ProviderConformanceOperation::Request)
        );
        assert!(
            provider_cases
                .iter()
                .any(|case| case.operation == ProviderConformanceOperation::Response)
        );
    }
}

#[test]
fn passthrough_providers_have_fixture_coverage_for_all_supported_non_streaming_endpoints() {
    for provider in [ProviderId::OpenAi, ProviderId::Local] {
        for endpoint in [
            ProviderEndpoint::Models,
            ProviderEndpoint::Embeddings,
            ProviderEndpoint::Images,
            ProviderEndpoint::Audio,
            ProviderEndpoint::Batches,
            ProviderEndpoint::Rerank,
            ProviderEndpoint::A2a,
        ] {
            let provider_cases: Vec<_> = provider_conformance_cases()
                .iter()
                .filter(|case| case.provider == provider && case.endpoint == endpoint)
                .collect();
            assert!(
                provider_cases
                    .iter()
                    .any(|case| case.operation == ProviderConformanceOperation::Request),
                "missing request fixture for {provider:?} {endpoint:?}"
            );
            assert!(
                provider_cases
                    .iter()
                    .any(|case| case.operation == ProviderConformanceOperation::Response),
                "missing response fixture for {provider:?} {endpoint:?}"
            );
        }
    }
}

#[test]
fn translated_provider_passthrough_endpoints_have_fixture_coverage_where_claimed() {
    for (provider, endpoint) in [
        (ProviderId::Anthropic, ProviderEndpoint::ChatCompletions),
        (ProviderId::Anthropic, ProviderEndpoint::Messages),
        (ProviderId::Copilot, ProviderEndpoint::ResponsesCompact),
        (ProviderId::Copilot, ProviderEndpoint::ChatCompletions),
        (ProviderId::Copilot, ProviderEndpoint::Messages),
        (ProviderId::DeepSeek, ProviderEndpoint::ChatCompletions),
        (ProviderId::DeepSeek, ProviderEndpoint::Messages),
        (ProviderId::Gemini, ProviderEndpoint::ChatCompletions),
        (ProviderId::Gemini, ProviderEndpoint::Messages),
        (ProviderId::Gemini, ProviderEndpoint::Embeddings),
    ] {
        let provider_cases: Vec<_> = provider_conformance_cases()
            .iter()
            .filter(|case| case.provider == provider && case.endpoint == endpoint)
            .collect();
        assert!(
            provider_cases
                .iter()
                .any(|case| case.operation == ProviderConformanceOperation::Request)
        );
        assert!(
            provider_cases
                .iter()
                .any(|case| case.operation == ProviderConformanceOperation::Response)
        );
    }
}
