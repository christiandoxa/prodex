use prodex_provider_core::{
    ProviderConformanceExpectedErrorClass, ProviderConformanceExpectedLoss,
    ProviderConformanceOperation, ProviderEndpoint, ProviderErrorClass, ProviderId,
    ProviderTransformInput, ProviderTransformLoss, classify_provider_error_body,
    provider_conformance_cases, provider_translator,
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

fn provider_error_class(expected: ProviderConformanceExpectedErrorClass) -> ProviderErrorClass {
    match expected {
        ProviderConformanceExpectedErrorClass::Auth => ProviderErrorClass::Auth,
        ProviderConformanceExpectedErrorClass::Quota => ProviderErrorClass::Quota,
        ProviderConformanceExpectedErrorClass::RateLimit => ProviderErrorClass::RateLimit,
        ProviderConformanceExpectedErrorClass::Transient => ProviderErrorClass::Transient,
        ProviderConformanceExpectedErrorClass::NotFound => ProviderErrorClass::NotFound,
        ProviderConformanceExpectedErrorClass::Other => ProviderErrorClass::Other,
    }
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
fn v1_conformance_fixtures_cover_explicit_non_openai_provider_flows() {
    let cases = provider_conformance_cases();

    for provider in [ProviderId::Anthropic, ProviderId::Copilot, ProviderId::Kiro] {
        assert!(
            cases.iter().any(|case| case.provider == provider),
            "missing any conformance fixture for {provider:?}"
        );
        assert!(
            cases.iter().any(|case| {
                case.provider == provider
                    && case.endpoint == ProviderEndpoint::Responses
                    && case.operation == ProviderConformanceOperation::Request
            }),
            "missing responses request fixture for {provider:?}"
        );
        assert!(
            cases.iter().any(|case| {
                case.provider == provider
                    && case.endpoint == ProviderEndpoint::Responses
                    && case.operation == ProviderConformanceOperation::Response
            }),
            "missing responses response fixture for {provider:?}"
        );
        assert!(
            cases.iter().any(|case| {
                case.provider == provider
                    && case.endpoint == ProviderEndpoint::Responses
                    && case.operation == ProviderConformanceOperation::StreamEvent
            }),
            "missing responses stream fixture for {provider:?}"
        );
    }

    assert!(
        cases.iter().any(|case| {
            case.name == "copilot-request-responses-compact-passthrough"
                && case.endpoint == ProviderEndpoint::ResponsesCompact
                && case.operation == ProviderConformanceOperation::Request
        }),
        "missing Copilot compact request fixture"
    );
    assert!(
        cases.iter().any(|case| {
            case.name == "copilot-response-responses-compact-passthrough"
                && case.endpoint == ProviderEndpoint::ResponsesCompact
                && case.operation == ProviderConformanceOperation::Response
        }),
        "missing Copilot compact response fixture"
    );
    assert!(
        cases.iter().any(|case| {
            case.name == "kiro-chat-completions-request-translated"
                && case.endpoint == ProviderEndpoint::ChatCompletions
                && case.operation == ProviderConformanceOperation::Request
        }),
        "missing translated Kiro chat-completions request fixture"
    );
    assert!(
        cases.iter().any(|case| {
            case.name == "kiro-chat-completions-request-rejects-parallel-tool-calls-false"
                && case.endpoint == ProviderEndpoint::ChatCompletions
                && case.operation == ProviderConformanceOperation::Request
                && matches!(
                    case.expected_loss,
                    ProviderConformanceExpectedLoss::Rejected
                )
        }),
        "missing rejected Kiro chat-completions request fixture"
    );
    assert!(
        cases.iter().any(|case| {
            case.name == "kiro-chat-completions-request-strips-accepted-controls"
                && case.endpoint == ProviderEndpoint::ChatCompletions
                && case.operation == ProviderConformanceOperation::Request
                && matches!(
                    case.expected_loss,
                    ProviderConformanceExpectedLoss::Degraded
                )
        }),
        "missing accepted-control Kiro chat-completions request fixture"
    );
    assert!(
        cases.iter().any(|case| {
            case.name == "kiro-chat-completions-request-maps-legacy-functions"
                && case.endpoint == ProviderEndpoint::ChatCompletions
                && case.operation == ProviderConformanceOperation::Request
                && matches!(
                    case.expected_loss,
                    ProviderConformanceExpectedLoss::Degraded
                )
        }),
        "missing legacy-function Kiro chat-completions request fixture"
    );
    assert!(
        cases.iter().any(|case| {
            case.name == "kiro-chat-completions-response-translated"
                && case.endpoint == ProviderEndpoint::ChatCompletions
                && case.operation == ProviderConformanceOperation::Response
                && matches!(
                    case.expected_loss,
                    ProviderConformanceExpectedLoss::Degraded
                )
        }),
        "missing translated Kiro chat-completions response fixture"
    );
    assert!(
        cases.iter().any(|case| {
            case.name == "kiro-chat-completions-response-rejects-invalid-shape"
                && case.endpoint == ProviderEndpoint::ChatCompletions
                && case.operation == ProviderConformanceOperation::Response
                && matches!(
                    case.expected_loss,
                    ProviderConformanceExpectedLoss::Rejected
                )
        }),
        "missing rejected Kiro chat-completions response fixture"
    );
}

#[test]
fn v1_conformance_fixtures_cover_error_classification_for_supported_non_openai_translators() {
    let cases = provider_conformance_cases();

    for provider in [
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
    ] {
        assert!(
            cases.iter().any(|case| {
                case.provider == provider
                    && case.endpoint == ProviderEndpoint::Responses
                    && case.operation == ProviderConformanceOperation::Request
                    && case.expected_error_class.is_some()
            }),
            "missing error classification fixture for {provider:?}"
        );
    }
}

#[test]
fn anthropic_and_copilot_responses_translators_publish_chat_compat_surface() {
    for (provider, endpoint, model, expected_upstream) in [
        (
            ProviderId::Anthropic,
            ProviderEndpoint::Responses,
            "auto",
            prodex_provider_core::ProviderWireFormat::OpenAiChatCompletions,
        ),
        (
            ProviderId::Copilot,
            ProviderEndpoint::Responses,
            "codex",
            prodex_provider_core::ProviderWireFormat::OpenAiChatCompletions,
        ),
        (
            ProviderId::Copilot,
            ProviderEndpoint::ResponsesCompact,
            "codex",
            prodex_provider_core::ProviderWireFormat::OpenAiChatCompletions,
        ),
    ] {
        let translator = provider_translator(provider);
        let input = ProviderTransformInput::new(
            endpoint,
            br#"{"model":"test","input":"hello","stream":false}"#.to_vec(),
        );

        let result = translator.transform_request(input);
        assert!(
            ProviderConformanceExpectedLoss::Lossless.matches_status(&result.status()),
            "{provider:?} {endpoint:?}"
        );
        assert_eq!(result.provider, provider, "{provider:?} {endpoint:?}");
        assert_eq!(result.endpoint, endpoint, "{provider:?} {endpoint:?}");
        assert_eq!(
            result.from_format,
            translator.client_wire_format(),
            "{provider:?} {endpoint:?}"
        );
        assert_eq!(
            result.to_format, expected_upstream,
            "{provider:?} {endpoint:?}"
        );
        assert_eq!(
            translator.upstream_wire_format(),
            expected_upstream,
            "{provider:?} {endpoint:?}"
        );
        assert!(
            translator.supported_params(endpoint, model).supported,
            "{provider:?} {endpoint:?}"
        );
    }
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
        let outcome = result.outcome();
        assert!(
            case.expected_loss.matches_status(&outcome.status),
            "{} expected {}, got {:?}",
            case.name,
            case.expected_loss.label(),
            outcome.status
        );
        if case.expected_loss.requires_reason() {
            assert!(
                outcome
                    .status
                    .reason()
                    .is_some_and(|reason| !reason.trim().is_empty()),
                "{} missing non-lossless transform reason",
                case.name
            );
        }
        if matches!(
            case.expected_loss,
            ProviderConformanceExpectedLoss::Degraded
        ) {
            match &result.loss {
                ProviderTransformLoss::DegradedButSafe { details, .. } => assert!(
                    !details.is_empty(),
                    "{} degraded transform missing audit details",
                    case.name
                ),
                other => panic!(
                    "{} expected degraded transform details, got {:?}",
                    case.name, other
                ),
            }
        }
        if matches!(
            case.expected_loss,
            ProviderConformanceExpectedLoss::Rejected
                | ProviderConformanceExpectedLoss::Unsupported
        ) {
            assert!(
                case.expected_body.is_none(),
                "{} terminal fixture should not declare an expected body",
                case.name
            );
            assert!(
                outcome.value.is_none(),
                "{} terminal transform should not return a body",
                case.name
            );
        }
        if let Some(expected) = &case.expected_body {
            let actual = outcome.value.as_ref().expect("body present");
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
            assert!(
                case.expected_error_cooldown_ms.is_some(),
                "{} missing expected_error_cooldown_ms",
                case.name
            );
            let expected_class = provider_error_class(expected_error_class);
            let actual = translator.classify_error(
                case.error_status,
                case.error_code.as_deref(),
                case.error_text.as_deref(),
            );
            assert_eq!(actual.class, expected_class, "{}", case.name);
            if let Some(expected_cooldown_ms) = case.expected_error_cooldown_ms {
                assert_eq!(actual.cooldown_ms, expected_cooldown_ms, "{}", case.name);
            }
            let body = serde_json::to_vec(&serde_json::json!({
                "error": {
                    "code": case.error_code.as_deref(),
                    "message": case.error_text.as_deref(),
                }
            }))
            .unwrap();
            let actual = classify_provider_error_body(
                case.error_status
                    .unwrap_or_else(|| panic!("{} missing error_status", case.name)),
                &body,
                |status, code, text| translator.classify_error(status, code, text),
            );
            assert_eq!(actual.class, expected_class, "{} body", case.name);
            if let Some(expected_cooldown_ms) = case.expected_error_cooldown_ms {
                assert_eq!(
                    actual.cooldown_ms, expected_cooldown_ms,
                    "{} body",
                    case.name
                );
            }
        }
    }
}

#[test]
fn openai_compatible_passthrough_cases_are_exact_lossless_identity() {
    for case in provider_conformance_cases()
        .iter()
        .filter(|case| matches!(case.provider, ProviderId::OpenAi | ProviderId::Local))
    {
        assert!(
            matches!(
                case.expected_loss,
                ProviderConformanceExpectedLoss::Lossless
            ),
            "{} should be declared lossless",
            case.name
        );
        let translator = provider_translator(case.provider);
        let (result, expected_body) = match case.operation {
            ProviderConformanceOperation::Request => {
                let input = input(case);
                let expected_body = input.body.clone();
                (translator.transform_request(input), expected_body)
            }
            ProviderConformanceOperation::Response => {
                let input = input(case);
                let expected_body = input.body.clone();
                (translator.transform_response(input), expected_body)
            }
            ProviderConformanceOperation::StreamEvent => {
                let expected_body = case
                    .input_body
                    .as_str()
                    .expect("passthrough stream fixture must be raw sse string")
                    .as_bytes()
                    .to_vec();
                (
                    translator.transform_stream_event(ProviderTransformInput {
                        endpoint: case.endpoint,
                        model: case.model.clone(),
                        headers: case.input_headers.clone(),
                        status: None,
                        body: expected_body.clone(),
                    }),
                    expected_body,
                )
            }
        };

        assert!(
            ProviderConformanceExpectedLoss::Lossless.matches_status(&result.status()),
            "{} should transform losslessly",
            case.name
        );
        assert_eq!(
            result.body.as_deref(),
            Some(expected_body.as_slice()),
            "{} should preserve the exact wire body",
            case.name
        );
        assert!(
            result.metadata.is_empty(),
            "{} should not add passthrough metadata",
            case.name
        );
    }
}

#[test]
fn translated_providers_advertise_endpoint_support_limitations_explicitly() {
    for (provider, endpoint) in [
        (ProviderId::DeepSeek, ProviderEndpoint::Embeddings),
        (ProviderId::Gemini, ProviderEndpoint::Audio),
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
            .any(|reason| reason.field == "response_format.type")
    );

    for provider in [ProviderId::Anthropic, ProviderId::Copilot] {
        let support =
            provider_translator(provider).supported_params(ProviderEndpoint::Responses, "test");
        assert!(support.supported, "{provider:?}");
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "input[*].content[type!=text]"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "response_format.type"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "reasoning"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "previous_response_id"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "tools[type!=function]"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "tool_choice[type!=function]"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "parallel_tool_calls=false"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "logprobs/top_logprobs"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "messages"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "metadata"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "safety_identifier"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "web_search_options"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "n>1"),
            "{provider:?}"
        );
        assert!(
            support
                .unsupported
                .iter()
                .any(|reason| reason.field == "stop_sequences"),
            "{provider:?}"
        );
    }
}

#[test]
fn translated_providers_have_explicit_error_mapping_fixtures() {
    for provider in [
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
    ] {
        assert!(
            provider_conformance_cases()
                .iter()
                .any(|case| case.provider == provider && case.expected_error_class.is_some()),
            "missing error mapping fixture for {provider:?}"
        );
    }
}

#[test]
fn provider_error_mapping_fixtures_cover_every_class() {
    for expected in [
        ProviderConformanceExpectedErrorClass::Auth,
        ProviderConformanceExpectedErrorClass::Quota,
        ProviderConformanceExpectedErrorClass::RateLimit,
        ProviderConformanceExpectedErrorClass::Transient,
        ProviderConformanceExpectedErrorClass::NotFound,
        ProviderConformanceExpectedErrorClass::Other,
    ] {
        assert!(
            provider_conformance_cases()
                .iter()
                .any(|case| case.expected_error_class == Some(expected)),
            "missing error mapping fixture for {expected:?}"
        );
    }
}

#[test]
fn translated_providers_have_explicit_usage_fixtures() {
    for provider in [
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
    ] {
        assert!(
            provider_conformance_cases()
                .iter()
                .any(|case| case.provider == provider && case.expected_usage.is_some()),
            "missing usage fixture for {provider:?}"
        );
    }
}

#[test]
fn provider_stream_fixtures_cover_completion_event() {
    for provider in [
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
    ] {
        assert!(
            provider_conformance_cases().iter().any(|case| {
                case.provider == provider
                    && case.operation == ProviderConformanceOperation::StreamEvent
                    && case
                        .expected_body
                        .as_ref()
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|body| body.contains("event: response.completed"))
            }),
            "missing stream completion fixture for {provider:?}"
        );
    }
}

#[test]
fn provider_conformance_fixtures_cover_required_edge_case_buckets() {
    let cases = provider_conformance_cases();
    assert!(
        cases.iter().any(|case| {
            case.name == "gemini-request-multimodal"
                && matches!(
                    case.expected_loss,
                    ProviderConformanceExpectedLoss::Lossless
                )
        }),
        "missing multimodal fixture"
    );
    assert!(
        cases.iter().any(|case| {
            case.name == "deepseek-request-json-schema-degrades"
                && matches!(
                    case.expected_loss,
                    ProviderConformanceExpectedLoss::Degraded
                )
        }),
        "missing degraded mapping fixture"
    );
    assert!(
        cases.iter().any(|case| {
            case.operation == ProviderConformanceOperation::StreamEvent
                && case
                    .expected_body
                    .as_ref()
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|body| body.contains("response.function_call_arguments.delta"))
        }),
        "missing function-call stream fixture"
    );
}

#[test]
fn v1_translated_providers_have_explicit_non_lossless_fixtures() {
    for provider in [
        ProviderId::Anthropic,
        ProviderId::Copilot,
        ProviderId::DeepSeek,
        ProviderId::Gemini,
        ProviderId::Kiro,
    ] {
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
    assert!(cases
        .iter()
        .any(|case| case.name == "deepseek-request-assistant-tool-call-and-tool-output-history"));
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
            .any(|case| case.name == "gemini-request-multimodal")
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
            expected.contains("\ndata: {") || expected.contains("\r\ndata: {"),
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
