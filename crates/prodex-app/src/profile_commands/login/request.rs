use super::LoginMethod;
use crate::validate_credential_free_http_url;
use anyhow::Result;
use std::ffi::OsString;

fn login_argument_plan(codex_args: &[OsString]) -> prodex_mojo_core::launch::LoginArgumentPlan {
    let arguments = codex_args
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Vec<_>>();
    prodex_mojo_core::launch::login_argument_plan(&arguments)
        .expect("Mojo login argument policy returned invalid output")
}

pub(super) fn infer_login_method(codex_args: &[OsString]) -> LoginMethod {
    match login_argument_plan(codex_args).method {
        prodex_mojo_core::launch::LoginArgumentMethod::ChatGpt => LoginMethod::ChatGpt,
        prodex_mojo_core::launch::LoginArgumentMethod::DeviceCode => LoginMethod::DeviceCode,
        prodex_mojo_core::launch::LoginArgumentMethod::ApiKey => LoginMethod::ApiKey,
        prodex_mojo_core::launch::LoginArgumentMethod::AccessToken => LoginMethod::AccessToken,
        prodex_mojo_core::launch::LoginArgumentMethod::Claude => LoginMethod::Claude,
        prodex_mojo_core::launch::LoginArgumentMethod::Antigravity => LoginMethod::Antigravity,
        prodex_mojo_core::launch::LoginArgumentMethod::Status => LoginMethod::Status,
    }
}

pub(super) fn gemini_oauth_login_requested(codex_args: &[OsString]) -> bool {
    login_argument_plan(codex_args).removed_gemini_oauth
}

pub(super) fn login_method_allows_base_url(codex_args: &[OsString]) -> bool {
    login_argument_plan(codex_args).base_url_allowed
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
