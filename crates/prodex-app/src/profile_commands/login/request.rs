use super::LoginMethod;
use crate::validate_credential_free_http_url;
use anyhow::Result;
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

pub(super) fn gemini_oauth_login_requested(codex_args: &[OsString]) -> bool {
    codex_args
        .iter()
        .any(|arg| arg == "--with-google" || arg == "--google")
}

pub(super) fn normalize_optional_base_url(value: &str) -> Result<Option<String>> {
    if value.is_empty() {
        return Ok(None);
    }
    validate_credential_free_http_url(value, "profile OpenAI-compatible base URL")?;
    Ok(Some(value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_base_url_rejects_secrets_without_echoing_or_stripping() {
        for value in [
            "https://user:login-password-secret-sentinel@example.test/v1",
            "https://example.test/v1?token=login-query-secret-sentinel",
            "https://example.test/v1#login-fragment-secret-sentinel",
            " not-a-url-login-parse-secret-sentinel ",
        ] {
            let error = normalize_optional_base_url(value).unwrap_err().to_string();

            assert!(
                error.contains("no credentials, query, or fragment"),
                "{error}"
            );
            assert!(!error.contains("secret-sentinel"), "{error}");
        }

        assert_eq!(
            normalize_optional_base_url("https://example.test/v1/").unwrap(),
            Some("https://example.test/v1/".to_string())
        );
    }

    #[test]
    fn removed_gemini_oauth_flags_remain_detectable_for_migration_errors() {
        assert!(gemini_oauth_login_requested(&[OsString::from(
            "--with-google"
        )]));
        assert!(gemini_oauth_login_requested(&[OsString::from("--google")]));
        assert!(!gemini_oauth_login_requested(&[OsString::from(
            "--with-api-key"
        )]));
    }
}
