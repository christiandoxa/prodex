use super::*;

#[test]
fn compiled_gemini_consumption_preserves_order_and_unrelated_config_forms() {
    let input = [
        "--config=other=keep",
        "-c",
        "\u{2003}model_thinking_budget\u{a0}=1024",
        "-cmodel_thinking_budget=2048",
        "--config=model_thinking_budget=4096",
        "--config",
        "model_thinking_budget=8192",
        "--",
        "-cmodel_thinking_budget=0",
        "-cmodel_thinking_budget_extra=keep",
        "--config",
        "model=keep",
        "-c",
    ]
    .map(OsString::from);
    assert_eq!(
        gemini_codex_args_without_consumed_overrides(&input),
        [
            "--config=other=keep",
            "--",
            "-cmodel_thinking_budget_extra=keep",
            "--config",
            "model=keep",
            "-c",
        ]
        .map(OsString::from)
    );
    assert_eq!(
        gemini_codex_args_without_consumed_overrides(&[]),
        Vec::<OsString>::new()
    );
}

#[cfg(unix)]
#[test]
fn compiled_gemini_consumption_retains_non_utf8_arguments_byte_for_byte() {
    use std::os::unix::ffi::OsStringExt;

    let opaque_value = OsString::from_vec(b"model_thinking_budget=\xff".to_vec());
    let opaque_inline = OsString::from_vec(b"-cmodel_thinking_budget=\xff".to_vec());
    let opaque_long = OsString::from_vec(b"--config=model_thinking_budget=\xff".to_vec());
    let input = [
        OsString::from("-c"),
        opaque_value.clone(),
        opaque_inline.clone(),
        OsString::from("--config=model_thinking_budget=1024"),
        opaque_long.clone(),
    ];
    assert_eq!(
        gemini_codex_args_without_consumed_overrides(&input),
        [
            OsString::from("-c"),
            opaque_value,
            opaque_inline,
            opaque_long
        ]
    );
}
