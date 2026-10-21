use super::*;
use prodex_mojo_core::codex_config::ConfigOverrideRemoval;

#[test]
fn compiled_override_removal_keeps_launch_pairs_distinct_from_frontend_inline_forms() {
    let input = [
        "--config=model=inline",
        "-c",
        " model =first",
        "--",
        "-cmodel=short",
        "--config",
        "model=second",
        "other",
    ];
    let mut args = input.map(OsString::from).to_vec();
    assert!(remove_first_codex_config_override_pair(&mut args, "model"));
    assert_eq!(
        args,
        [
            "--config=model=inline",
            "--",
            "-cmodel=short",
            "--config",
            "model=second",
            "other"
        ]
        .map(OsString::from)
    );
    assert!(remove_codex_config_overrides(
        &mut args,
        "model",
        ConfigOverrideRemoval::AllForms
    ));
    assert_eq!(args, ["--", "other"].map(OsString::from));
    assert!(!remove_first_codex_config_override_pair(&mut args, "model"));
}

#[cfg(unix)]
#[test]
fn compiled_override_removal_preserves_original_non_utf8_platform_arguments() {
    use std::os::unix::ffi::OsStringExt;
    let invalid_value = OsString::from_vec(b"model=\xff".to_vec());
    let invalid_inline = OsString::from_vec(b"-cmodel=\xff".to_vec());
    let opaque = OsString::from_vec(b"opaque\xff".to_vec());
    let mut args = vec![
        "-c".into(),
        invalid_value.clone(),
        invalid_inline,
        opaque.clone(),
    ];
    assert!(!remove_first_codex_config_override_pair(&mut args, "model"));
    assert!(remove_codex_config_overrides(
        &mut args,
        "model",
        ConfigOverrideRemoval::AllForms
    ));
    assert_eq!(args, vec![OsString::from("-c"), invalid_value, opaque]);
}
