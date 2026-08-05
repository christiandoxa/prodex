use anyhow::{Context, Result, bail};
use prodex_provider_core::ProviderId;
use serde_json::Value;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
#[path = "kiro_command.rs"]
mod command;
use command::run_kiro_metadata_command;
#[path = "kiro_environment.rs"]
mod environment;
use environment::discover_kiro_database_path;
pub(crate) use environment::kiro_cli_data_dir_env;
#[path = "kiro_store.rs"]
mod store;
pub(crate) use store::prepare_kiro_cli_data_dir;
use store::{KIRO_DATA_DIR, write_kiro_cli_data_dir};
#[path = "kiro/lifecycle_support.rs"]
mod lifecycle_support;
pub(crate) use lifecycle_support::handle_import_kiro_profile;

use super::manage::print_profile_panel;
use super::write_secret_text_file;
#[cfg(test)]
use crate::create_codex_home_if_missing;
use crate::runtime_kiro_acp::{
    runtime_kiro_acp_bootstrap_with_command, runtime_kiro_acp_model_catalog,
};
use crate::secret_store_support::secret_file_read_error;
use crate::{AppState, kiro_bin};
#[cfg(test)]
use rusqlite::{Connection, params};

pub(crate) const KIRO_CREDENTIALS_FILE: &str = "kiro_auth.json";
pub(crate) const KIRO_MODEL_CATALOG_FILE: &str = "kiro_model_catalog.json";
const KIRO_PROFILE_STATE_KEY: &str = "api.codewhisperer.profile";
const KIRO_START_URL_STATE_KEY: &str = "auth.idc.start-url";
const KIRO_REGION_STATE_KEY: &str = "auth.idc.region";
const KIRO_BUILDER_START_URL: &str = "https://view.awsapps.com/start";
const KIRO_AUTH_KEY_PRIORITY: &[&str] = &[
    "kirocli:social:token",
    "kirocli:external-idp:token",
    "codewhisperer:odic:token",
];

struct KiroImportContext {
    auth_key: String,
    auth_kind: String,
    raw_auth_json: String,
    email: Option<String>,
    profile_arn: Option<String>,
    profile_name: Option<String>,
    start_url: Option<String>,
    region: Option<String>,
}

impl fmt::Debug for KiroImportContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KiroImportContext")
            .field("auth_key", &"<redacted>")
            .field("auth_kind", &"<redacted>")
            .field("raw_auth_json", &"<redacted>")
            .field("email", &self.email.as_ref().map(|_| "<redacted>"))
            .field(
                "profile_arn",
                &self.profile_arn.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "profile_name",
                &self.profile_name.as_ref().map(|_| "<redacted>"),
            )
            .field("start_url", &self.start_url.as_ref().map(|_| "<redacted>"))
            .field("region", &self.region.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub(crate) struct KiroAuthSecret {
    pub(crate) auth_key: String,
    pub(crate) auth_kind: String,
    pub(crate) auth_json: String,
    #[serde(default)]
    pub(crate) email: Option<String>,
    #[serde(default)]
    pub(crate) profile_arn: Option<String>,
    #[serde(default)]
    pub(crate) profile_name: Option<String>,
    #[serde(default)]
    pub(crate) start_url: Option<String>,
    #[serde(default)]
    pub(crate) region: Option<String>,
}

impl fmt::Debug for KiroAuthSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KiroAuthSecret")
            .field("auth_key", &"<redacted>")
            .field("auth_kind", &"<redacted>")
            .field("auth_json", &"<redacted>")
            .field("email", &self.email.as_ref().map(|_| "<redacted>"))
            .field(
                "profile_arn",
                &self.profile_arn.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "profile_name",
                &self.profile_name.as_ref().map(|_| "<redacted>"),
            )
            .field("start_url", &self.start_url.as_ref().map(|_| "<redacted>"))
            .field("region", &self.region.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

pub(super) fn is_kiro_import_source(path: &Path) -> bool {
    path.components().count() == 1
        && path
            .to_str()
            .is_some_and(|value| value.eq_ignore_ascii_case("kiro"))
        && !path.exists()
}

fn render_kiro_import_result(
    state: &AppState,
    profile_name: &str,
    context: &KiroImportContext,
    updated_existing: bool,
    model_catalog_refreshed: bool,
) -> Result<()> {
    let mut fields = vec![
        (
            "Result".to_string(),
            if updated_existing {
                format!("Updated imported Kiro profile '{profile_name}'.")
            } else {
                format!("Imported Kiro profile '{profile_name}'.")
            },
        ),
        ("Profile".to_string(), profile_name.to_string()),
        ("Provider".to_string(), "Kiro CLI".to_string()),
        ("Auth".to_string(), context.auth_kind.clone()),
        (
            "Storage".to_string(),
            if updated_existing {
                format!("Credential snapshot stored in {KIRO_CREDENTIALS_FILE}.")
            } else {
                format!("Managed profile home created with {KIRO_CREDENTIALS_FILE}.")
            },
        ),
    ];
    if let Some(email) = context.email.as_deref() {
        fields.push(("Identity".to_string(), email.to_string()));
    }
    if let Some(profile_name) = context.profile_name.as_deref() {
        fields.push(("Kiro profile".to_string(), profile_name.to_string()));
    }
    if let Some(profile_arn) = context.profile_arn.as_deref() {
        fields.push(("Profile ARN".to_string(), profile_arn.to_string()));
    }
    if let Some(start_url) = context.start_url.as_deref() {
        fields.push(("Start URL".to_string(), start_url.to_string()));
    }
    if let Some(region) = context.region.as_deref() {
        fields.push(("Region".to_string(), region.to_string()));
    }
    if state.active_profile.as_deref() == Some(profile_name) {
        fields.push(("Active".to_string(), profile_name.to_string()));
    }
    if let Some(warning) = kiro_model_catalog_warning(model_catalog_refreshed) {
        fields.push(("Models".to_string(), warning.to_string()));
    }
    print_profile_panel(
        if updated_existing {
            "Profile Updated"
        } else {
            "Profile Added"
        },
        &fields,
    )
}

fn kiro_model_catalog_warning(refreshed: bool) -> Option<&'static str> {
    (!refreshed).then_some("Catalog refresh failed; re-import this Kiro profile to retry.")
}

pub(crate) fn parse_kiro_auth_secret_text(text: &str) -> Result<KiroAuthSecret> {
    let secret: KiroAuthSecret =
        serde_json::from_str(text).context("failed to parse Kiro auth secret JSON")?;
    if secret.auth_key.trim().is_empty() {
        bail!("Kiro auth secret is missing auth_key");
    }
    if secret.auth_kind.trim().is_empty() {
        bail!("Kiro auth secret is missing auth_kind");
    }
    if secret.auth_json.trim().is_empty() {
        bail!("Kiro auth secret is missing auth_json");
    }
    let _: Value = serde_json::from_str(&secret.auth_json)
        .context("failed to parse embedded Kiro auth_json")?;
    Ok(secret)
}

pub(crate) fn parse_kiro_model_catalog_text(text: &str) -> Result<Vec<Value>> {
    let value: Value =
        serde_json::from_str(text).context("failed to parse Kiro model catalog JSON")?;
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .context("Kiro model catalog is missing models array")?;
    if models.len() > prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT {
        bail!(
            "Kiro model catalog exceeds the hard limit of {} entries",
            prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT
        );
    }
    prodex_provider_core::merge_provider_model_catalog_json(ProviderId::Kiro, models)
        .map_err(anyhow::Error::new)?;
    Ok(models.clone())
}

pub(crate) fn read_kiro_auth_secret(codex_home: &Path) -> Result<KiroAuthSecret> {
    let path = codex_home.join(KIRO_CREDENTIALS_FILE);
    let text = read_kiro_auth_secret_text(&path)?;
    parse_kiro_auth_secret_text(&text)
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn read_kiro_auth_secret_text(path: &Path) -> Result<String> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .read_text(&secret_store::SecretLocation::file(path))
        .map_err(secret_file_read_error)?
        .with_context(|| format!("failed to read {}", path.display()))
}

fn write_kiro_auth_secret(codex_home: &Path, secret: &KiroAuthSecret) -> Result<()> {
    let path = codex_home.join(KIRO_CREDENTIALS_FILE);
    write_secret_text_file(
        &path,
        &serde_json::to_string_pretty(secret).context("failed to serialize Kiro auth secret")?,
    )
}

fn refresh_kiro_model_catalog_snapshot(codex_home: &Path, secret: &KiroAuthSecret) -> Result<()> {
    write_kiro_model_catalog_snapshot(codex_home, secret)
}

pub(crate) fn write_kiro_model_catalog_snapshot(
    codex_home: &Path,
    secret: &KiroAuthSecret,
) -> Result<()> {
    write_kiro_model_catalog_snapshot_with_command(codex_home, secret, &kiro_bin())
}

fn write_kiro_model_catalog_snapshot_with_command(
    codex_home: &Path,
    secret: &KiroAuthSecret,
    command: &std::ffi::OsStr,
) -> Result<()> {
    let data_dir = codex_home.join(KIRO_DATA_DIR);
    write_kiro_cli_data_dir(&data_dir, secret)?;
    let mut extra_env = kiro_cli_data_dir_env(&data_dir);
    if let Some(region) = secret
        .region
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        extra_env.push((OsString::from("AWS_REGION"), OsString::from(region)));
    }
    let cwd = env::current_dir().unwrap_or_else(|_| codex_home.to_path_buf());
    let models = native_kiro_model_catalog(command, &cwd, &extra_env).or_else(|_| {
        let bootstrap = runtime_kiro_acp_bootstrap_with_command(command, &cwd, &extra_env)?;
        runtime_kiro_acp_model_catalog(&bootstrap.session)
    })?;
    let path = codex_home.join(KIRO_MODEL_CATALOG_FILE);
    if models.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    let contents = serde_json::to_string_pretty(&serde_json::json!({ "models": models }))
        .context("failed to serialize Kiro model catalog")?;
    if contents.len() as u64 > crate::PROVIDER_MODEL_CATALOG_MAX_BYTES {
        bail!(
            "Kiro model catalog exceeds the hard limit of {} bytes",
            crate::PROVIDER_MODEL_CATALOG_MAX_BYTES
        );
    }
    write_secret_text_file(&path, &contents)
}

fn native_kiro_model_catalog(
    command: &std::ffi::OsStr,
    cwd: &Path,
    extra_env: &[(OsString, OsString)],
) -> Result<Vec<Value>> {
    let output = run_kiro_metadata_command(
        command,
        &["chat", "--list-models", "--format", "json"],
        Some(cwd),
        extra_env,
    )
    .context("failed to query the Kiro model catalog")?;
    if !output.status.success() {
        bail!("Kiro model catalog command failed with {}", output.status);
    }
    let value: Value =
        serde_json::from_slice(&output.stdout).context("failed to parse the Kiro model catalog")?;
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .context("Kiro model catalog is missing models")?;
    if models.len() > prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT {
        bail!(
            "Kiro model catalog exceeds the hard limit of {} entries",
            prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT
        );
    }
    let models = models
        .iter()
        .filter_map(|model| {
            let id = model
                .get("model_id")
                .or_else(|| model.get("id"))
                .and_then(Value::as_str)?
                .trim();
            if id.is_empty() {
                return None;
            }
            let name = model
                .get("model_name")
                .or_else(|| model.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or(id);
            let mut normalized = serde_json::json!({
                "id": id,
                "name": name,
                "object": "model",
                "owned_by": "kiro-cli",
            });
            if let Some(description) = model.get("description").and_then(Value::as_str) {
                normalized["description"] = Value::String(description.to_string());
            }
            if let Some(context_window) = model.get("context_window_tokens").and_then(Value::as_u64)
            {
                normalized["context_window_tokens"] = Value::from(context_window);
            }
            Some(normalized)
        })
        .collect::<Vec<_>>();
    if models.is_empty() {
        bail!("Kiro model catalog returned no usable models");
    }
    prodex_provider_core::merge_provider_model_catalog_json(ProviderId::Kiro, &models)
        .map_err(anyhow::Error::new)?;
    Ok(models)
}

fn read_kiro_whoami_json() -> Result<Value> {
    let output = run_kiro_metadata_command(&kiro_bin(), &["whoami", "--format", "json"], None, &[])
        .context("failed to execute Kiro CLI")?;
    if !output.status.success() {
        bail!(
            "Kiro CLI whoami failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("failed to parse Kiro whoami JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    #[test]
    fn read_kiro_auth_secret_rejects_symlink() {
        let root = std::env::temp_dir().join(format!(
            "prodex-kiro-secret-symlink-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let codex_home = root.join("codex-home");
        let outside = root.join("outside");
        fs::create_dir_all(&codex_home).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let outside_secret = outside.join(KIRO_CREDENTIALS_FILE);
        fs::write(
            &outside_secret,
            serde_json::to_string(&KiroAuthSecret {
                auth_key: "codewhisperer:odic:token".to_string(),
                auth_kind: "builder-id".to_string(),
                auth_json: serde_json::json!({"access_token": "outside"}).to_string(),
                email: None,
                profile_arn: None,
                profile_name: None,
                start_url: None,
                region: None,
            })
            .unwrap(),
        )
        .unwrap();
        std::os::unix::fs::symlink(&outside_secret, codex_home.join(KIRO_CREDENTIALS_FILE))
            .unwrap();

        let err = read_kiro_auth_secret(&codex_home).expect_err("symlink secret should reject");

        assert!(
            err.to_string().contains("not a regular secret file"),
            "unexpected error: {err:#}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn kiro_auth_debug_output_redacts_sensitive_fields() {
        let secret = KiroAuthSecret {
            auth_key: "codewhisperer:secret-token-key".to_string(),
            auth_kind: "builder-id-secret".to_string(),
            auth_json: serde_json::json!({
                "access_token": "kiro-access-token-secret",
                "refresh_token": "kiro-refresh-token-secret"
            })
            .to_string(),
            email: Some("kiro-user@example.test".to_string()),
            profile_arn: Some(
                "arn:aws:codewhisperer:us-east-1:123456789012:profile/secret".to_string(),
            ),
            profile_name: Some("builder-profile-secret".to_string()),
            start_url: Some("https://view.awsapps.com/start-secret".to_string()),
            region: Some("us-east-1-secret".to_string()),
        };
        let context = KiroImportContext {
            auth_key: secret.auth_key.clone(),
            auth_kind: secret.auth_kind.clone(),
            raw_auth_json: secret.auth_json.clone(),
            email: secret.email.clone(),
            profile_arn: secret.profile_arn.clone(),
            profile_name: secret.profile_name.clone(),
            start_url: secret.start_url.clone(),
            region: secret.region.clone(),
        };

        for rendered in [format!("{secret:?}"), format!("{context:?}")] {
            assert!(rendered.contains("<redacted>"), "{rendered}");
            for raw in [
                "codewhisperer:secret-token-key",
                "builder-id-secret",
                "kiro-access-token-secret",
                "kiro-refresh-token-secret",
                "kiro-user@example.test",
                "arn:aws:codewhisperer:us-east-1:123456789012:profile/secret",
                "builder-profile-secret",
                "https://view.awsapps.com/start-secret",
                "us-east-1-secret",
            ] {
                assert!(!rendered.contains(raw), "{rendered}");
            }
        }
    }

    #[test]
    fn failed_kiro_model_catalog_refresh_is_user_visible_without_error_details() {
        let warning = kiro_model_catalog_warning(false).unwrap();
        assert!(warning.contains("Catalog refresh failed"));
        assert!(!warning.contains("/tmp/"));
        assert!(!warning.contains("token"));
        assert_eq!(kiro_model_catalog_warning(true), None);
    }

    #[test]
    fn kiro_model_catalog_parser_rejects_oversized_payloads() {
        let text = serde_json::json!({
            "models": (0..=prodex_provider_core::PROVIDER_MODEL_CATALOG_HARD_LIMIT)
                .map(|index| serde_json::json!({"id": format!("model-{index}")}))
                .collect::<Vec<_>>()
        })
        .to_string();

        let error = parse_kiro_model_catalog_text(&text).unwrap_err();

        assert!(error.to_string().contains("hard limit of 1024 entries"));
    }

    fn write_fake_kiro_binary(root: &Path) -> PathBuf {
        crate::test_support::write_test_python_executable(
            root,
            "fake-kiro-cli",
            r#"#!/usr/bin/env python3
import json, os, sys
assert os.environ['KIRO_DATA_DIR'] == os.environ['Q_CLI_DATA_DIR']
if len(sys.argv) > 2 and sys.argv[1] == 'chat' and sys.argv[2] == '--list-models':
    print(json.dumps({"models":[{"model_name":"auto","description":"Models chosen by task","model_id":"auto","context_window_tokens":1000000},{"model_name":"claude-sonnet-4.5","description":"Claude Sonnet 4.5 model","model_id":"claude-sonnet-4.5","context_window_tokens":200000}],"default_model":"auto"}))
    sys.exit(0)
if len(sys.argv) > 1 and sys.argv[1] == 'acp':
    first = json.loads(sys.stdin.readline())
    second = json.loads(sys.stdin.readline())
    assert first["method"] == "initialize"
    assert second["method"] == "session/new"
    print(json.dumps({"jsonrpc":"2.0","result":{"protocolVersion":1,"agentCapabilities":{"loadSession":True,"promptCapabilities":{"image":True,"audio":False,"embeddedContext":False},"mcpCapabilities":{"http":True,"sse":False},"sessionCapabilities":{},"auth":{}},"authMethods":[{"id":"kiro-login","name":"Kiro Login","description":"Run 'kiro-cli login'."}],"agentInfo":{"name":"Kiro CLI Agent","title":"Kiro CLI Agent","version":"2.10.0"}},"id":0}), flush=True)
    print(json.dumps({"jsonrpc":"2.0","result":{"sessionId":"session-1","models":{"currentModelId":"claude-sonnet-4","availableModels":[{"modelId":"claude-sonnet-4","name":"claude-sonnet-4"},{"modelId":"claude-sonnet-4.5","name":"claude-sonnet-4.5"}]}},"id":1}), flush=True)
    sys.exit(0)
if len(sys.argv) > 2 and sys.argv[1] == 'whoami' and sys.argv[2] == '--format':
    print(json.dumps({"email":"kiro-user@example.com"}))
    sys.exit(0)
sys.exit(1)
"#,
        )
    }

    #[test]
    fn write_kiro_cli_data_dir_materializes_auth_and_profile_state() {
        let root = std::env::temp_dir().join(format!(
            "prodex-kiro-data-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let codex_home = root.join("codex-home");
        create_codex_home_if_missing(&codex_home).unwrap();
        let data_dir = codex_home.join(KIRO_DATA_DIR);
        let secret = KiroAuthSecret {
            auth_key: "codewhisperer:odic:token".to_string(),
            auth_kind: "builder-id".to_string(),
            auth_json: serde_json::json!({
                "access_token": "kiro-access-token",
                "expires_at": "2026-01-01T00:00:00Z",
                "region": "us-east-1"
            })
            .to_string(),
            email: Some("kiro-user@example.com".to_string()),
            profile_arn: Some(
                "arn:aws:codewhisperer:us-east-1:123456789012:profile/test".to_string(),
            ),
            profile_name: Some("builder-id-test".to_string()),
            start_url: Some("https://view.awsapps.com/start".to_string()),
            region: Some("us-east-1".to_string()),
        };

        write_kiro_auth_secret(&codex_home, &secret).unwrap();
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(data_dir.join("data.sqlite3"), []).unwrap();
        let (prepared_data_dir, _) =
            prepare_kiro_cli_data_dir(&codex_home).expect("data dir should materialize");
        assert_eq!(prepared_data_dir, data_dir);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&data_dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(data_dir.join("data.sqlite3"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let connection =
            Connection::open(data_dir.join("data.sqlite3")).expect("sqlite db should open");
        let auth_value: String = connection
            .query_row(
                "SELECT value FROM auth_kv WHERE key = ?1",
                params![secret.auth_key],
                |row| row.get(0),
            )
            .expect("auth secret should exist");
        assert_eq!(auth_value, secret.auth_json);
        let profile_value: String = connection
            .query_row(
                "SELECT value FROM state WHERE key = ?1",
                params![KIRO_PROFILE_STATE_KEY],
                |row| row.get(0),
            )
            .expect("profile state should exist");
        let profile_json: Value =
            serde_json::from_str(&profile_value).expect("profile state should parse");
        assert_eq!(
            profile_json["profile_name"].as_str(),
            Some("builder-id-test")
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT value FROM state WHERE key = ?1",
                    params![KIRO_REGION_STATE_KEY],
                    |row| row.get::<_, String>(0),
                )
                .expect("region should exist"),
            "us-east-1"
        );
        assert_eq!(
            connection
                .query_row("SELECT MAX(version) FROM migrations", [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("latest migration should exist"),
            9
        );
        for table in ["conversations_v2", "extracted_kas_versions"] {
            assert_eq!(
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                        params![table],
                        |row| row.get::<_, i64>(0),
                    )
                    .expect("Kiro compatibility table should be queryable"),
                1,
                "missing Kiro compatibility table {table}"
            );
        }

        let refreshed_auth = serde_json::json!({
            "access_token": "kiro-refreshed-access-token",
            "expires_at": "2026-01-02T00:00:00Z",
            "region": "us-east-1"
        })
        .to_string();
        connection
            .execute(
                "UPDATE auth_kv SET value = ?1 WHERE key = ?2",
                params![refreshed_auth, secret.auth_key],
            )
            .unwrap();
        drop(connection);

        let (_, prepared_secret) = prepare_kiro_cli_data_dir(&codex_home).unwrap();
        assert_eq!(prepared_secret.auth_json, refreshed_auth);
        assert_eq!(
            read_kiro_auth_secret(&codex_home).unwrap().auth_json,
            refreshed_auth
        );

        #[cfg(unix)]
        {
            let database_path = data_dir.join("data.sqlite3");
            fs::remove_file(&database_path).unwrap();
            let outside = root.join("outside.sqlite3");
            fs::write(&outside, []).unwrap();
            std::os::unix::fs::symlink(outside, database_path).unwrap();
            let error = prepare_kiro_cli_data_dir(&codex_home).unwrap_err();
            assert!(error.to_string().contains("not a regular Kiro database"));
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn write_kiro_model_catalog_snapshot_from_acp_session_models() {
        let root = std::env::temp_dir().join(format!(
            "prodex-kiro-model-catalog-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("test root should exist");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let codex_home = root.join("codex-home");
        create_codex_home_if_missing(&codex_home).expect("codex home should exist");
        let fake_kiro = write_fake_kiro_binary(&root);

        let secret = KiroAuthSecret {
            auth_key: "codewhisperer:odic:token".to_string(),
            auth_kind: "builder-id".to_string(),
            auth_json: serde_json::json!({
                "access_token": "kiro-access-token",
                "region": "us-east-1"
            })
            .to_string(),
            email: Some("kiro-user@example.com".to_string()),
            profile_arn: Some(
                "arn:aws:codewhisperer:us-east-1:123456789012:profile/test".to_string(),
            ),
            profile_name: Some("builder-id-test".to_string()),
            start_url: Some("https://view.awsapps.com/start".to_string()),
            region: Some("us-east-1".to_string()),
        };
        write_kiro_model_catalog_snapshot_with_command(&codex_home, &secret, fake_kiro.as_os_str())
            .expect("model catalog snapshot should be written");
        let catalog_path = codex_home.join(KIRO_MODEL_CATALOG_FILE);
        let catalog_text = fs::read_to_string(&catalog_path).expect("catalog should exist");
        let value: Value = serde_json::from_str(&catalog_text).expect("catalog json should parse");
        assert_eq!(value["models"][0]["id"], "auto");
        assert_eq!(value["models"][0]["context_window_tokens"], 1_000_000);
        assert_eq!(value["models"][1]["id"], "claude-sonnet-4.5");
        let _ = fs::remove_dir_all(root);
    }
}
