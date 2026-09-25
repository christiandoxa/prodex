use super::*;
use std::ffi::OsString;

#[test]
fn launch_plans_use_compiled_mojo() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };

    assert_eq!(
        normalize_codex_profile_args(&[
            "--profile-v2=work".into(),
            "--".into(),
            "--profile-v2=literal".into(),
        ]),
        [
            OsString::from("--profile=work"),
            OsString::from("--"),
            OsString::from("--profile-v2=literal"),
        ]
    );
}

#[cfg(unix)]
#[test]
fn launch_plans_preserve_non_utf8_arguments_byte_for_byte() {
    use std::os::unix::ffi::OsStringExt;

    let opaque = OsString::from_vec(vec![b'm', 0xff, 0x80]);
    let (args, include_code_review) = prepare_codex_launch_args(
        &[
            "exec".into(),
            "--model".into(),
            opaque.clone(),
            "--profile-v2=work".into(),
            "--".into(),
            "--full-access".into(),
        ],
        false,
    );

    assert_eq!(
        args,
        [
            OsString::from("exec"),
            OsString::from("--model"),
            opaque,
            OsString::from("--profile=work"),
            OsString::from("--"),
            OsString::from("--full-access"),
        ]
    );
    assert!(!include_code_review);
}

#[cfg(windows)]
#[test]
fn launch_plans_preserve_unpaired_utf16_arguments() {
    use std::os::windows::ffi::OsStringExt;

    let opaque = OsString::from_wide(&[0xd800, 0x61]);
    let (args, _) =
        prepare_codex_launch_args(&["exec".into(), "--model".into(), opaque.clone()], false);

    assert_eq!(
        args,
        [OsString::from("exec"), OsString::from("--model"), opaque,]
    );
}

#[test]
fn config_rewrites_keep_option_pairs_precedence_and_separator_tail() {
    let address = "127.0.0.1:12345".parse().unwrap();
    let args = runtime_proxy_local_model_provider_codex_args(
        address,
        "/test/",
        "test",
        &[
            "exec".into(),
            "--config".into(),
            "model_providers.test.base_url=\"https://example.com/old-one\"".into(),
            "--config=model_providers.test.base_url=\"https://example.com/old-two\"".into(),
            "-cmodel_providers.test.base_url=\"https://example.com/old-three\"".into(),
            "--".into(),
            "--config=model_providers.test.base_url=\"https://example.com/literal\"".into(),
        ],
    );

    assert_eq!(
        args,
        [
            OsString::from("exec"),
            OsString::from("--config"),
            OsString::from("model_providers.test.base_url=\"http://127.0.0.1:12345/test\""),
            OsString::from(
                "--config=model_providers.test.base_url=\"http://127.0.0.1:12345/test\""
            ),
            OsString::from("-cmodel_providers.test.base_url=\"http://127.0.0.1:12345/test\""),
            OsString::from("--"),
            OsString::from(
                "--config=model_providers.test.base_url=\"https://example.com/literal\""
            ),
        ]
    );
}
