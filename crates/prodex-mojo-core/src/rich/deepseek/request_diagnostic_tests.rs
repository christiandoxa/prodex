use super::*;
use DeepSeekRequestPolicyOperation::*;

#[test]
fn compiled_request_diagnostics_cover_every_rejection_and_success_tag() {
    let cases = [
        (
            RequestFields,
            1,
            "frequency_penalty is deprecated and is not forwarded by Prodex",
        ),
        (
            RequestFields,
            2,
            "presence_penalty is deprecated and is not forwarded by Prodex",
        ),
        (
            RequestFields,
            10,
            "n is not supported by this Responses adapter",
        ),
        (
            RequestFields,
            11,
            "seed is not supported by this Responses adapter",
        ),
        (
            RequestFields,
            12,
            "service_tier is not supported by this Responses adapter",
        ),
        (
            RequestFields,
            13,
            "prediction is not supported by this Responses adapter",
        ),
        (
            RequestFields,
            14,
            "logit_bias is not supported by this Responses adapter",
        ),
        (
            RequestFields,
            15,
            "functions is not supported by this Responses adapter",
        ),
        (
            RequestFields,
            16,
            "function_call is not supported by this Responses adapter",
        ),
        (RequestFields, 20, "include must be an array"),
        (RequestFields, 21, "store must be a boolean"),
        (
            RequestFields,
            22,
            "background responses are not supported by this Responses adapter",
        ),
        (RequestFields, 23, "background must be a boolean"),
        (
            RequestFields,
            24,
            "truncation=auto is not supported by this Responses adapter",
        ),
        (RequestFields, 25, "truncation `東京🔥` is not supported"),
        (RequestFields, 26, "truncation must be a string"),
        (
            RequestFields,
            27,
            "max_tool_calls is not supported by this Responses adapter",
        ),
        (RequestFields, 28, "text must be an object"),
        (
            RequestFields,
            29,
            "text.東京🔥 is not supported by this Responses adapter",
        ),
        (
            RequestFields,
            30,
            "does not expose a compatible parallel_tool_calls=false control",
        ),
        (RequestFields, 31, "parallel_tool_calls must be a boolean"),
        (RequestFields, 32, "stream_options must be an object"),
        (RequestFields, 33, "stream_options requires stream=true"),
        (RequestFields, 34, "stream_options.東京🔥 is not supported"),
        (
            RequestFields,
            35,
            "streaming adapter requires stream_options.include_usage=true",
        ),
        (
            RequestFields,
            36,
            "stream_options.include_usage must be a boolean",
        ),
        (RequestFields, 37, "modalities must be an array"),
        (
            RequestFields,
            38,
            "Responses adapter only supports text modality; audio/image/video modalities are not supported",
        ),
        (
            RequestFields,
            39,
            "Responses adapter does not support audio output",
        ),
        (
            BetaFields,
            1,
            "chat prefix completion requires the beta chat endpoint, which is outside this Responses adapter",
        ),
        (
            BetaFields,
            2,
            "FIM suffix completion requires the beta /completions endpoint, which is outside this Responses adapter",
        ),
        (
            BetaFields,
            3,
            "prompt completions require the beta /completions endpoint, which is outside this Responses adapter",
        ),
        (ReasoningShape, 1, "reasoning must be an object"),
        (
            ReasoningShape,
            2,
            "reasoning.東京🔥 is not supported by this Responses adapter",
        ),
        (ReasoningShape, 3, "reasoning.effort must be a string"),
        (ReasoningShape, 4, "reasoning_effort must be a string"),
        (PrimitiveCore, 1, "temperature must be a number"),
        (PrimitiveCore, 2, "top_p must be a number"),
        (
            PrimitiveCore,
            3,
            "max_output_tokens must be a positive integer",
        ),
        (PrimitiveCore, 4, "max_tokens must be a positive integer"),
        (
            PrimitiveCore,
            5,
            "max_completion_tokens must be a positive integer",
        ),
        (PrimitiveCore, 6, "logprobs must be a boolean"),
        (TopLogprobs, 1, "top_logprobs must be an integer"),
        (TopLogprobs, 2, "top_logprobs must be <= 20"),
        (TopLogprobs, 3, "top_logprobs requires logprobs=true"),
        (Stop, 1, "stop must be a string or array of strings"),
        (Stop, 2, "supports at most 16 stop sequences"),
        (Stop, 3, "stop sequences must be strings"),
        (
            ToolChoice,
            1,
            "tool_choice string `東京🔥` is not supported",
        ),
        (ToolChoice, 2, "tool_choice must be a string or object"),
        (ToolChoice, 3, "named tool_choice requires a function name"),
        (ToolChoice, 4, "tool_choice type `東京🔥` is not supported"),
        (
            ResponseFormatShape,
            3,
            "response_format must include a type",
        ),
        (
            ResponseFormatShape,
            4,
            "response_format type `東京🔥` is not supported",
        ),
        (MetadataShape, 1, "request metadata must be an object"),
        (MetadataShape, 2, "client_metadata must be an object"),
        (MetadataShape, 3, "prompt_cache_key must be a string"),
        (MetadataShape, 4, "prompt_cache_retention must be a string"),
    ];
    for provider in ["DeepSeek", "Kiro", "Gemini", "Provider🧭"] {
        for &(operation, tag, message) in &cases {
            assert_eq!(
                deepseek_request_diagnostic(operation, tag, provider, "東京🔥").unwrap(),
                Some(format!("{provider} {message}")),
                "{operation:?}/{tag}/{provider}",
            );
        }
        for operation in [
            RequestFields,
            BetaFields,
            ReasoningShape,
            PrimitiveCore,
            TopLogprobs,
            Stop,
            ToolChoice,
            ResponseFormatShape,
            MetadataShape,
        ] {
            assert_eq!(
                deepseek_request_diagnostic(operation, 0, provider, "").unwrap(),
                None
            );
            assert_eq!(
                deepseek_request_diagnostic(operation, -1, provider, ""),
                Err(MojoError::InvalidInput)
            );
            assert_eq!(
                deepseek_request_diagnostic(operation, i64::MAX, provider, ""),
                Err(MojoError::InvalidInput)
            );
        }
    }
    for (operation, tag) in [
        (ReasoningShape, 10),
        (ResponseFormatShape, 1),
        (ResponseFormatShape, 2),
        (MetadataShape, 256),
    ] {
        assert_eq!(
            deepseek_request_diagnostic(operation, tag, "Kiro", "high").unwrap(),
            None
        );
    }
    assert_eq!(
        deepseek_request_diagnostic(ToolsShape, 0, "Kiro", ""),
        Err(MojoError::InvalidInput)
    );
    let detail = "\"\\\n\0🧭".repeat(300);
    assert_eq!(
        deepseek_request_diagnostic(ToolChoice, 1, "Gemini", &detail).unwrap(),
        Some(format!(
            "Gemini tool_choice string `{detail}` is not supported"
        )),
    );
}

#[test]
fn compiled_request_diagnostic_abi_is_atomic_on_malformed_inputs_and_short_buffers() {
    let provider = b"Kiro";
    let detail = b"xml";
    let mut output = [57_u8; 256];
    let mut metadata = [89_i64; 3];
    let valid = [
        1,
        ToolChoice as i64,
        1,
        provider.as_ptr() as usize as i64,
        provider.len() as i64,
        detail.as_ptr() as usize as i64,
        detail.len() as i64,
        output.as_mut_ptr() as usize as i64,
        output.len() as i64,
        metadata.as_mut_ptr() as usize as i64,
        metadata.len() as i64,
    ];
    let call = |input: [i64; 11]| unsafe {
        prodex_deepseek_request_diagnostic_v1(
            input[0],
            input[1],
            input[2],
            input[3] as u64,
            input[4],
            input[5] as u64,
            input[6],
            input[7] as u64,
            input[8],
            input[9] as u64,
            input[10],
        )
    };
    let invalid = [0xff_u8];
    for (index, value, expected) in [
        (0, 2, 4),
        (1, 99, 1),
        (2, 99, 1),
        (2, -1, 1),
        (3, 0, 1),
        (4, -1, 1),
        (5, 0, 1),
        (6, -1, 1),
        (7, 0, 1),
        (8, -1, 1),
        (8, 1, 3),
        (9, 0, 1),
        (10, 1, 1),
        (3, invalid.as_ptr() as usize as i64, 1),
        (5, invalid.as_ptr() as usize as i64, 1),
    ] {
        let mut input = valid;
        input[index] = value;
        if index == 3 && value != 0 {
            input[4] = 1;
        }
        if index == 5 && value != 0 {
            input[6] = 1;
        }
        assert_eq!(call(input), expected, "argument {index}={value}");
        assert_eq!(output, [57; 256]);
        assert_eq!(metadata, [89; 3]);
    }
    let expected = "Kiro tool_choice string `xml` is not supported";
    let mut input = valid;
    input[8] = expected.len() as i64;
    assert_eq!(call(input), 0);
    assert_eq!(metadata, [1, expected.len() as i64, 89]);
    assert_eq!(&output[..expected.len()], expected.as_bytes());
    assert!(output[expected.len()..].iter().all(|byte| *byte == 57));
}
