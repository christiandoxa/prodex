use super::*;
use std::ffi::OsString;

#[test]
fn runtime_proxy_launch_uses_mojo_mount_path_normalization() {
    assert_eq!(
        prodex_mojo_core::rich::runtime_proxy_mount_path("/v1/").unwrap(),
        "/v1"
    );
    let args = runtime_proxy_codex_args_with_mount_path(
        "127.0.0.1:4455".parse().expect("socket addr"),
        "\u{2003}//v1///\u{3000}",
        &[OsString::from("exec")],
    )
    .into_iter()
    .map(|arg| arg.to_string_lossy().into_owned())
    .collect::<Vec<_>>();

    assert!(args.iter().any(|arg| {
        arg == "model_providers.prodex-openai-governed-http.base_url=\"http://127.0.0.1:4455/v1\""
    }));
}
