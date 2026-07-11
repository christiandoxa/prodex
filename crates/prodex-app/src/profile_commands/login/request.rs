use super::LoginMethod;
use std::ffi::OsString;

pub(super) fn infer_login_method(codex_args: &[OsString]) -> LoginMethod {
    if codex_args
        .first()
        .and_then(|arg| arg.to_str())
        .is_some_and(|arg| arg == "status")
    {
        return LoginMethod::Status;
    }
    if codex_args.iter().any(|arg| arg == "--with-api-key") {
        return LoginMethod::ApiKey;
    }
    if codex_args
        .iter()
        .any(|arg| arg == "--with-google" || arg == "--google")
    {
        return LoginMethod::Google;
    }
    if codex_args
        .iter()
        .any(|arg| arg == "--with-claude" || arg == "--claude")
    {
        return LoginMethod::Claude;
    }
    if codex_args.iter().any(|arg| {
        arg == "--with-antigravity"
            || arg == "--antigravity"
            || arg == "--with-agy"
            || arg == "--agy"
    }) {
        return LoginMethod::Antigravity;
    }
    if codex_args.iter().any(|arg| arg == "--with-access-token") {
        return LoginMethod::AccessToken;
    }
    if codex_args.iter().any(|arg| arg == "--device-auth") {
        return LoginMethod::DeviceCode;
    }
    LoginMethod::ChatGpt
}
