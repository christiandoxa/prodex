use crate::{create_codex_home_if_missing, print_wrapped_stderr};
use anyhow::{Context, Result, bail};
use dirs::home_dir;
use serde::Deserialize;
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const CLAUDE_CREDENTIALS_FILE: &str = ".credentials.json";
const CLAUDE_OAUTH_EXPIRY_SKEW_MS: i64 = 60_000;

#[derive(Clone)]
pub(crate) struct ClaudeOAuthSecret {
    pub(crate) access_token: String,
    pub(crate) expires_at: Option<i64>,
    pub(crate) account: Option<String>,
    pub(crate) auth_method: Option<String>,
}

impl fmt::Debug for ClaudeOAuthSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaudeOAuthSecret")
            .field("access_token", &"<redacted>")
            .field("expires_at", &self.expires_at.map(|_| "<redacted>"))
            .field("account", &self.account.as_ref().map(|_| "<redacted>"))
            .field(
                "auth_method",
                &self.auth_method.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Clone)]
pub(crate) struct ClaudeAuthStatus {
    pub(crate) logged_in: bool,
    pub(crate) auth_method: Option<String>,
    pub(crate) account: Option<String>,
}

impl fmt::Debug for ClaudeAuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaudeAuthStatus")
            .field("logged_in", &self.logged_in)
            .field(
                "auth_method",
                &self.auth_method.as_ref().map(|_| "<redacted>"),
            )
            .field("account", &self.account.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Deserialize)]
struct ClaudeCredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<ClaudeCredentialsToken>,
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

impl fmt::Debug for ClaudeCredentialsFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaudeCredentialsFile")
            .field(
                "claude_ai_oauth",
                &self.claude_ai_oauth.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_at", &self.expires_at.map(|_| "<redacted>"))
            .field(
                "subscription_type",
                &self.subscription_type.as_ref().map(|_| "<redacted>"),
            )
            .field("email", &self.email.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Clone, Deserialize)]
struct ClaudeCredentialsToken {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

impl fmt::Debug for ClaudeCredentialsToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClaudeCredentialsToken")
            .field("access_token", &"<redacted>")
            .field("expires_at", &self.expires_at.map(|_| "<redacted>"))
            .field(
                "subscription_type",
                &self.subscription_type.as_ref().map(|_| "<redacted>"),
            )
            .field("email", &self.email.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

pub(crate) fn claude_config_dir_from_env_or_default() -> Result<PathBuf> {
    env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".claude")))
        .context("failed to determine Claude config directory")
}

pub(crate) fn claude_credentials_path(config_dir: &Path) -> PathBuf {
    config_dir.join(CLAUDE_CREDENTIALS_FILE)
}

pub(crate) fn read_claude_oauth_secret(config_dir: &Path) -> Result<ClaudeOAuthSecret> {
    let path = claude_credentials_path(config_dir);
    let text = read_claude_credentials_text(&path)?;
    parse_claude_oauth_secret_text(&text)
        .with_context(|| format!("failed to parse {}", path.display()))
}

pub(crate) fn copy_claude_oauth_credentials(
    from_config_dir: &Path,
    to_config_dir: &Path,
) -> Result<()> {
    let from_path = claude_credentials_path(from_config_dir);
    let text = read_claude_credentials_text(&from_path)?;
    parse_claude_oauth_secret_text(&text)
        .with_context(|| format!("failed to parse {}", from_path.display()))?;
    create_codex_home_if_missing(to_config_dir)?;
    let to_path = claude_credentials_path(to_config_dir);
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(&to_path), &text)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", to_path.display()))
}

fn read_claude_credentials_text(path: &Path) -> Result<String> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .read_text(&secret_store::SecretLocation::file(path))
        .map_err(secret_file_read_error)
        .with_context(|| format!("failed to read {}", path.display()))?
        .with_context(|| format!("failed to read {}", path.display()))
}

fn secret_file_read_error(error: secret_store::SecretError) -> anyhow::Error {
    let is_non_regular_file = matches!(
        &error,
        secret_store::SecretError::InvalidLocation { reason }
            if reason.ends_with(" is not a regular secret file")
    );
    let error = anyhow::Error::new(error);
    if is_non_regular_file {
        error.context("not a regular secret file")
    } else {
        error
    }
}

pub(crate) fn login_with_claude_oauth(
    config_dir: &Path,
    email: Option<&str>,
) -> Result<ExitStatus> {
    create_codex_home_if_missing(config_dir)?;
    let mut command = Command::new(claude_binary());
    command
        .arg("auth")
        .arg("login")
        .arg("--claudeai")
        .env("CLAUDE_CONFIG_DIR", config_dir)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some(email) = email.map(str::trim).filter(|email| !email.is_empty()) {
        command.arg("--email").arg(email);
    }
    print_wrapped_stderr("Opening Claude sign-in through Claude Code.");
    command
        .status()
        .with_context(|| format!("failed to execute {}", claude_binary()))
}

pub(crate) fn refresh_claude_oauth_secret_if_needed(
    config_dir: &Path,
) -> Result<ClaudeOAuthSecret> {
    let secret = read_claude_oauth_secret(config_dir)?;
    if !claude_oauth_secret_expired(&secret) {
        return Ok(secret);
    }
    let _ = claude_auth_status(config_dir);
    read_claude_oauth_secret(config_dir)
}

pub(crate) fn claude_auth_status(config_dir: &Path) -> Result<ClaudeAuthStatus> {
    let output = Command::new(claude_binary())
        .arg("auth")
        .arg("status")
        .arg("--json")
        .env("CLAUDE_CONFIG_DIR", config_dir)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .output()
        .with_context(|| format!("failed to execute {}", claude_binary()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            bail!("Claude auth status failed");
        }
        bail!("Claude auth status failed: {stderr}");
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("failed to parse Claude auth status JSON")?;
    Ok(ClaudeAuthStatus {
        logged_in: value
            .get("loggedIn")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        auth_method: json_string_at_any_key(&value, &["authMethod", "method"]),
        account: json_string_at_any_key(
            &value,
            &[
                "email",
                "account",
                "login",
                "username",
                "displayName",
                "organizationName",
            ],
        ),
    })
}

pub(crate) fn claude_oauth_profile_identity(
    config_dir: &Path,
) -> Result<(Option<String>, Option<String>)> {
    let secret = read_claude_oauth_secret(config_dir)?;
    let status = claude_auth_status(config_dir).ok();
    let account = status
        .as_ref()
        .filter(|status| status.logged_in)
        .and_then(|status| status.account.clone())
        .or(secret.account);
    let auth_method = status
        .filter(|status| status.logged_in)
        .and_then(|status| status.auth_method)
        .or(secret.auth_method)
        .or_else(|| Some("claude-ai-oauth".to_string()));
    Ok((account, auth_method))
}

pub(crate) fn parse_claude_oauth_secret_text(text: &str) -> Result<ClaudeOAuthSecret> {
    let file: ClaudeCredentialsFile =
        serde_json::from_str(text).context("invalid Claude credentials JSON")?;
    if let Some(token) = file.claude_ai_oauth {
        let access_token = token.access_token.trim().to_string();
        if access_token.is_empty() {
            bail!("Claude credentials did not include an access token");
        }
        return Ok(ClaudeOAuthSecret {
            access_token,
            expires_at: token.expires_at,
            account: token.email.filter(|value| !value.trim().is_empty()),
            auth_method: Some(claude_auth_method_label(token.subscription_type.as_deref())),
        });
    }
    let access_token = file
        .access_token
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .context("Claude credentials did not include an access token")?;
    Ok(ClaudeOAuthSecret {
        access_token,
        expires_at: file.expires_at,
        account: file.email.filter(|value| !value.trim().is_empty()),
        auth_method: Some(claude_auth_method_label(file.subscription_type.as_deref())),
    })
}

fn claude_auth_method_label(subscription_type: Option<&str>) -> String {
    subscription_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("claude-ai-oauth:{value}"))
        .unwrap_or_else(|| "claude-ai-oauth".to_string())
}

fn claude_oauth_secret_expired(secret: &ClaudeOAuthSecret) -> bool {
    let Some(expires_at) = secret.expires_at else {
        return false;
    };
    expires_at <= current_time_ms().saturating_add(CLAUDE_OAUTH_EXPIRY_SKEW_MS)
}

fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn json_string_at_any_key(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    match value {
        serde_json::Value::Object(object) => {
            for key in keys {
                if let Some(text) = object
                    .get(*key)
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                {
                    return Some(text.to_string());
                }
            }
            object
                .values()
                .find_map(|value| json_string_at_any_key(value, keys))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .find_map(|value| json_string_at_any_key(value, keys)),
        _ => None,
    }
}

fn claude_binary() -> String {
    env::var("CLAUDE_BIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "claude".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude_credentials_text() -> String {
        r#"{
          "claudeAiOauth": {
            "accessToken": "oauth-token",
            "expiresAt": 1900000000000,
            "subscriptionType": "max",
            "email": "user@example.com"
          }
        }"#
        .to_string()
    }

    #[cfg(unix)]
    #[test]
    fn read_claude_oauth_secret_rejects_symlink() {
        let root = std::env::temp_dir().join(format!(
            "prodex-claude-oauth-symlink-{}-{}",
            std::process::id(),
            current_time_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("target.json");
        std::fs::write(&target, claude_credentials_text()).unwrap();
        std::os::unix::fs::symlink(&target, claude_credentials_path(&root)).unwrap();

        let err = read_claude_oauth_secret(&root).expect_err("symlink secret must be rejected");

        assert!(err.to_string().contains("failed to read"));
        assert!(format!("{err:#}").contains("regular secret file"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn claude_oauth_debug_output_redacts_sensitive_fields() {
        let secret = ClaudeOAuthSecret {
            access_token: "claude-access-token-secret".to_string(),
            expires_at: Some(1_900_000_000_000),
            account: Some("alice@example.test".to_string()),
            auth_method: Some("claude-ai-oauth:max-secret".to_string()),
        };
        let status = ClaudeAuthStatus {
            logged_in: true,
            auth_method: Some("claude-ai-oauth:pro-secret".to_string()),
            account: Some("bob@example.test".to_string()),
        };
        let token = ClaudeCredentialsToken {
            access_token: "claude-nested-token-secret".to_string(),
            expires_at: Some(1_900_000_000_001),
            subscription_type: Some("max-secret".to_string()),
            email: Some("carol@example.test".to_string()),
        };
        let credentials = ClaudeCredentialsFile {
            claude_ai_oauth: Some(token.clone()),
            access_token: Some("claude-top-token-secret".to_string()),
            expires_at: Some(1_900_000_000_002),
            subscription_type: Some("team-secret".to_string()),
            email: Some("dave@example.test".to_string()),
        };

        for rendered in [
            format!("{secret:?}"),
            format!("{status:?}"),
            format!("{token:?}"),
            format!("{credentials:?}"),
        ] {
            assert!(rendered.contains("<redacted>"), "{rendered}");
            for raw in [
                "claude-access-token-secret",
                "1900000000000",
                "alice@example.test",
                "claude-ai-oauth:max-secret",
                "claude-ai-oauth:pro-secret",
                "bob@example.test",
                "claude-nested-token-secret",
                "1900000000001",
                "max-secret",
                "carol@example.test",
                "claude-top-token-secret",
                "1900000000002",
                "team-secret",
                "dave@example.test",
            ] {
                assert!(!rendered.contains(raw), "{rendered}");
            }
        }
    }

    #[test]
    fn parses_nested_claude_ai_oauth_credentials() {
        let secret = parse_claude_oauth_secret_text(&claude_credentials_text()).unwrap();

        assert_eq!(secret.access_token, "oauth-token");
        assert_eq!(secret.expires_at, Some(1900000000000));
        assert_eq!(secret.account.as_deref(), Some("user@example.com"));
        assert_eq!(secret.auth_method.as_deref(), Some("claude-ai-oauth:max"));
    }

    #[test]
    fn parses_top_level_claude_oauth_credentials() {
        let secret = parse_claude_oauth_secret_text(
            r#"{
              "accessToken": "top-level-token",
              "expiresAt": 1900000000001,
              "email": "user@example.com"
            }"#,
        )
        .unwrap();

        assert_eq!(secret.access_token, "top-level-token");
        assert_eq!(secret.expires_at, Some(1900000000001));
        assert_eq!(secret.account.as_deref(), Some("user@example.com"));
        assert_eq!(secret.auth_method.as_deref(), Some("claude-ai-oauth"));
    }
}
