mod code_assist;
mod quota;

use crate::create_codex_home_if_missing;
use anyhow::{Context, Result, bail};
pub(crate) use code_assist::gemini_code_assist_endpoint;
pub(crate) use quota::{
    fetch_gemini_quota, fetch_gemini_quota_json, fetch_gemini_quota_with_code_assist_endpoint,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};

pub(crate) const GEMINI_OAUTH_SECRET_FILE: &str = "gemini_oauth.json";
pub(crate) const GEMINI_OAUTH_DISABLED_GUIDANCE: &str = "Google Gemini OAuth profiles are unsupported and disabled. For Codex-fronted Gemini, migrate to a Gemini API key (`--api-key`, `GEMINI_API_KEY`, or `GOOGLE_API_KEY`). For supported Vertex AI authentication, use the native Gemini CLI path (`prodex s gemini --cli gemini`), which owns its authentication.";

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct GeminiOAuthSecret {
    pub(crate) auth_mode: String,
    pub(crate) access_token: String,
    #[serde(default)]
    pub(crate) refresh_token: Option<String>,
    #[serde(default)]
    pub(crate) token_type: Option<String>,
    #[serde(default)]
    pub(crate) scope: Option<String>,
    #[serde(default)]
    pub(crate) expiry_date: Option<i64>,
    pub(crate) email: String,
    #[serde(default)]
    pub(crate) project_id: Option<String>,
}

impl fmt::Debug for GeminiOAuthSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeminiOAuthSecret")
            .field("auth_mode", &"<redacted>")
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "token_type",
                &self.token_type.as_ref().map(|_| "<redacted>"),
            )
            .field("scope", &self.scope.as_ref().map(|_| "<redacted>"))
            .field("expiry_date", &self.expiry_date.map(|_| "<redacted>"))
            .field("email", &"<redacted>")
            .field(
                "project_id",
                &self.project_id.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

pub(crate) fn gemini_oauth_secret_path(codex_home: &Path) -> PathBuf {
    codex_home.join(GEMINI_OAUTH_SECRET_FILE)
}

pub(crate) fn write_gemini_oauth_secret(
    codex_home: &Path,
    secret: &GeminiOAuthSecret,
) -> Result<()> {
    create_codex_home_if_missing(codex_home)?;
    secret_store::ensure_private_directory(codex_home)
        .with_context(|| format!("failed to secure {}", codex_home.display()))?;
    let path = gemini_oauth_secret_path(codex_home);
    let text =
        serde_json::to_string_pretty(secret).context("failed to serialize Gemini OAuth secret")?;
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(&path), &text)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn refresh_gemini_oauth_secret_if_needed(
    _codex_home: &Path,
) -> Result<GeminiOAuthSecret> {
    bail!(GEMINI_OAUTH_DISABLED_GUIDANCE)
}

pub(crate) fn force_refresh_gemini_oauth_secret(_codex_home: &Path) -> Result<GeminiOAuthSecret> {
    bail!(GEMINI_OAUTH_DISABLED_GUIDANCE)
}

pub(crate) fn gemini_oauth_project_from_env() -> Option<String> {
    [
        "GOOGLE_CLOUD_PROJECT",
        "GOOGLE_CLOUD_PROJECT_ID",
        "GCLOUD_PROJECT",
    ]
    .into_iter()
    .find_map(|key| {
        env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn normalize_gemini_project_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_oauth_secret_debug_output_redacts_sensitive_fields() {
        let secret = GeminiOAuthSecret {
            auth_mode: "gemini-oauth-secret-mode".to_string(),
            access_token: "gemini-access-token-secret".to_string(),
            refresh_token: Some("gemini-refresh-token-secret".to_string()),
            token_type: Some("Bearer-secret".to_string()),
            scope: Some("secret-scope".to_string()),
            expiry_date: Some(123_456_789),
            email: "alice@example.test".to_string(),
            project_id: Some("gemini-project-secret".to_string()),
        };
        let rendered = format!("{secret:?}");

        assert!(rendered.contains("GeminiOAuthSecret"));
        assert!(rendered.contains("<redacted>"));
        for raw in [
            "gemini-oauth-secret-mode",
            "gemini-access-token-secret",
            "gemini-refresh-token-secret",
            "Bearer-secret",
            "secret-scope",
            "123456789",
            "alice@example.test",
            "gemini-project-secret",
        ] {
            assert!(!rendered.contains(raw), "{rendered}");
        }
    }

    #[test]
    fn disabled_gemini_oauth_never_refreshes_legacy_credentials() {
        let err = refresh_gemini_oauth_secret_if_needed(Path::new("/synthetic/profile"))
            .expect_err("Gemini OAuth must remain disabled");
        let message = err.to_string();
        assert!(message.contains("unsupported and disabled"));
        assert!(message.contains("Gemini API key"));
        assert!(message.contains("Vertex AI"));
    }

    #[test]
    fn legacy_gemini_oauth_profile_remains_parseable_for_migration() {
        let secret: GeminiOAuthSecret = serde_json::from_value(serde_json::json!({
            "auth_mode": "gemini_oauth",
            "access_token": "synthetic-access-token",
            "refresh_token": "synthetic-refresh-token",
            "email": "synthetic@example.com"
        }))
        .expect("legacy Gemini OAuth profile should remain parseable");
        assert_eq!(secret.auth_mode, "gemini_oauth");
    }

    #[test]
    fn tracked_oauth_credential_reconstruction_source_is_absent() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        for source in [
            "src/runtime_gemini_auth/oauth.rs",
            "src/profile_commands/login/google.rs",
        ] {
            assert!(
                !manifest_dir.join(source).exists(),
                "removed Gemini OAuth source must stay absent: {source}"
            );
        }
    }
}
