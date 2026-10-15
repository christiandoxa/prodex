#![cfg(feature = "mojo-rich")]

use prodex_mojo_core::rich::{DeepSeekKernelInput, DeepSeekKernelOperation, deepseek_kernel};

fn classify(source: Option<&str>) -> Vec<u8> {
    let mut input = DeepSeekKernelInput::new(DeepSeekKernelOperation::StreamErrorClassification);
    input.input = source;
    deepseek_kernel(input).expect("DeepSeek stream error kernel")
}

#[test]
fn stream_error_classification_preserves_provider_precedence_and_unicode() {
    assert_eq!(
        classify(Some(r#""boom \u6771\u4eac""#)),
        br#"{"code":"provider_stream_error","message":"boom \u6771\u4eac"}"#.to_vec()
    );
    assert_eq!(
        classify(Some(
            r#"{"type":"rate_limit","code":500,"message":"retry \ud83d\ude00","detail":"ignored"}"#,
        )),
        br#"{"code":"rate_limit","message":"retry \ud83d\ude00"}"#.to_vec()
    );
    assert_eq!(
        classify(Some(r#"{"type":500,"code":401,"detail":"unauthorized"}"#)),
        br#"{"code":"500","message":"unauthorized"}"#.to_vec()
    );
    assert_eq!(
        classify(Some(r#"{"type":-9223372036854775808}"#)),
        br#"{"code":"-9223372036854775808","message":"Provider stream returned an embedded error"}"#.to_vec()
    );
    assert_eq!(
        classify(Some(r#"{"type":9223372036854775808}"#)),
        br#"{"code":"provider_stream_error","message":"Provider stream returned an embedded error"}"#.to_vec()
    );
    assert_eq!(
        classify(Some(r#"{"type":1.5}"#)),
        br#"{"code":"provider_stream_error","message":"Provider stream returned an embedded error"}"#.to_vec()
    );
}

#[test]
fn stream_error_classification_keeps_defaults_and_absence() {
    assert_eq!(classify(None), b"null".to_vec());
    assert_eq!(
        classify(Some(r#"{}"#)),
        br#"{"code":"provider_stream_error","message":"Provider stream returned an embedded error"}"#.to_vec()
    );
    assert_eq!(
        classify(Some(r#"{"message":false,"detail":"ignored"}"#)),
        br#"{"code":"provider_stream_error","message":"Provider stream returned an embedded error"}"#.to_vec()
    );
}
