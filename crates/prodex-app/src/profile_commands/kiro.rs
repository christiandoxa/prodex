use anyhow::{Context, Result, bail};
use dirs::{data_local_dir, home_dir};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde_json::Value;
use std::env;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::manage::print_profile_panel;
use super::write_secret_text_file;
use crate::runtime_kiro_acp::{runtime_kiro_acp_bootstrap, runtime_kiro_acp_model_catalog};
use crate::{
    AppPaths, AppState, AppStateIoExt, ImportProfileArgs, ProfileEntry, ProfileProvider,
    audit_log_event_best_effort, create_codex_home_if_missing, ensure_path_is_unique, kiro_bin,
    managed_profile_home_path, prepare_managed_codex_home,
};

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

#[derive(Debug)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
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

pub(super) fn is_kiro_import_source(path: &Path) -> bool {
    path.components().count() == 1
        && path
            .to_str()
            .is_some_and(|value| value.eq_ignore_ascii_case("kiro"))
        && !path.exists()
}

pub(crate) fn handle_import_kiro_profile(args: &ImportProfileArgs) -> Result<()> {
    let context = resolve_kiro_import_context()?;
    let provider = ProfileProvider::Kiro {
        auth_key: context.auth_key.clone(),
        auth_kind: Some(context.auth_kind.clone()),
        profile_arn: context.profile_arn.clone(),
        profile_name: context.profile_name.clone(),
        start_url: context.start_url.clone(),
        region: context.region.clone(),
    };

    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let profile_name = if let Some(existing_name) = find_kiro_profile_by_identity(&state, &context)
    {
        let activate = state.active_profile.is_none() || args.activate;
        let profile = state
            .profiles
            .get_mut(&existing_name)
            .with_context(|| format!("profile '{}' is missing", existing_name))?;
        write_kiro_auth_secret(
            &profile.codex_home,
            &kiro_auth_secret_from_context(&context),
        )?;
        refresh_kiro_model_catalog_snapshot(
            &profile.codex_home,
            &kiro_auth_secret_from_context(&context),
        );
        profile.provider = provider.clone();
        profile.email = context.email.clone();
        if activate {
            state.active_profile = Some(existing_name.clone());
        }
        state.save(&paths)?;
        render_kiro_import_result(&state, &existing_name, &context, true)?;
        audit_kiro_import(&state, &existing_name, &context, true);
        return Ok(());
    } else {
        let requested = args
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        match requested {
            Some(name) => {
                prodex_profile_identity::validate_profile_name(name)?;
                name.to_string()
            }
            None => default_kiro_profile_name(&paths, &state, &context),
        }
    };

    let activate = state.active_profile.is_none() || args.activate;
    let codex_home = managed_profile_home_path(&paths, &profile_name)?;
    ensure_path_is_unique(&state, &codex_home)?;
    create_codex_home_if_missing(&codex_home)?;
    prepare_managed_codex_home(&paths, &codex_home)?;
    write_kiro_auth_secret(&codex_home, &kiro_auth_secret_from_context(&context))?;
    refresh_kiro_model_catalog_snapshot(&codex_home, &kiro_auth_secret_from_context(&context));
    state.profiles.insert(
        profile_name.clone(),
        ProfileEntry {
            codex_home,
            managed: true,
            email: context.email.clone(),
            provider: provider.clone(),
        },
    );
    if activate {
        state.active_profile = Some(profile_name.clone());
    }
    state.save(&paths)?;
    render_kiro_import_result(&state, &profile_name, &context, false)?;
    audit_kiro_import(&state, &profile_name, &context, false);
    Ok(())
}

fn render_kiro_import_result(
    state: &AppState,
    profile_name: &str,
    context: &KiroImportContext,
    updated_existing: bool,
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
    print_profile_panel(
        if updated_existing {
            "Profile Updated"
        } else {
            "Profile Added"
        },
        &fields,
    )
}

fn audit_kiro_import(
    state: &AppState,
    profile_name: &str,
    context: &KiroImportContext,
    updated_existing: bool,
) {
    audit_log_event_best_effort(
        "profile",
        "import_kiro",
        "success",
        serde_json::json!({
            "profile_name": profile_name,
            "provider": "kiro",
            "auth_key": context.auth_key,
            "auth_kind": context.auth_kind,
            "email": context.email,
            "profile_arn": context.profile_arn,
            "profile_name_upstream": context.profile_name,
            "start_url": context.start_url,
            "region": context.region,
            "activated": state.active_profile.as_deref() == Some(profile_name),
            "updated_existing": updated_existing,
        }),
    );
}

fn default_kiro_profile_name(
    paths: &AppPaths,
    state: &AppState,
    context: &KiroImportContext,
) -> String {
    let base = context
        .email
        .as_deref()
        .map(|email| prodex_profile_identity::profile_name_from_email(&format!("kiro-{email}")))
        .or_else(|| {
            context.profile_name.as_deref().map(|name| {
                prodex_profile_identity::profile_name_from_email(&format!("kiro-{name}"))
            })
        })
        .unwrap_or_else(|| "kiro".to_string());
    prodex_profile_identity::unique_profile_name_from_base(&base, "kiro", |candidate| {
        !state.profiles.contains_key(candidate)
            && !paths.managed_profiles_root.join(candidate).exists()
    })
}

fn find_kiro_profile_by_identity(state: &AppState, context: &KiroImportContext) -> Option<String> {
    state.profiles.iter().find_map(|(name, profile)| {
        profile
            .provider
            .kiro_matches(
                &context.auth_key,
                context.profile_arn.as_deref(),
                context.profile_name.as_deref(),
            )
            .then_some(name.clone())
    })
}

fn resolve_kiro_import_context() -> Result<KiroImportContext> {
    let database_path = discover_kiro_database_path()?;
    let connection = Connection::open_with_flags(
        &database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("failed to open {}", database_path.display()))?;
    let (auth_key, raw_token) = read_kiro_auth_token(&connection)?;
    let profile = read_kiro_profile_state(&connection)?;
    let state_start_url = read_kiro_state_value(&connection, KIRO_START_URL_STATE_KEY)?;
    let state_region = read_kiro_state_value(&connection, KIRO_REGION_STATE_KEY)?;
    let whoami = read_kiro_whoami_json().ok();

    let token_value: Value = serde_json::from_str(&raw_token)
        .with_context(|| format!("failed to parse Kiro auth JSON for key '{auth_key}'"))?;
    let token_start_url = token_value
        .get("start_url")
        .or_else(|| token_value.get("startUrl"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let token_region = token_value
        .get("region")
        .or_else(|| token_value.get("aws_region"))
        .or_else(|| token_value.get("awsRegion"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let start_url = state_start_url.or(token_start_url);
    let region = state_region.or(token_region);
    let auth_kind = parse_kiro_auth_kind(&auth_key, &token_value, start_url.as_deref());
    let email = parse_kiro_email(&token_value)
        .or_else(|| whoami.as_ref().and_then(parse_kiro_email))
        .or_else(|| profile.as_ref().and_then(|profile| profile.user_id.clone()));

    Ok(KiroImportContext {
        auth_key,
        auth_kind,
        raw_auth_json: raw_token,
        email,
        profile_arn: profile.as_ref().map(|profile| profile.arn.clone()),
        profile_name: profile.as_ref().map(|profile| profile.profile_name.clone()),
        start_url,
        region,
    })
}

fn discover_kiro_database_path() -> Result<PathBuf> {
    if let Some(path) = env::var_os("Q_CLI_DATA_DIR") {
        let candidate = PathBuf::from(path).join("data.sqlite3");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    let mut candidates = Vec::new();
    if let Some(data_dir) = data_local_dir() {
        candidates.push(data_dir.join("kiro-cli").join("data.sqlite3"));
        candidates.push(data_dir.join("amazon-q").join("data.sqlite3"));
    }
    if let Some(home) = home_dir() {
        candidates.push(
            home.join(".local")
                .join("share")
                .join("kiro-cli")
                .join("data.sqlite3"),
        );
        candidates.push(
            home.join(".local")
                .join("share")
                .join("amazon-q")
                .join("data.sqlite3"),
        );
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .context("failed to find Kiro auth database; expected ~/.local/share/kiro-cli/data.sqlite3 or ~/.local/share/amazon-q/data.sqlite3")
}

fn read_kiro_auth_token(connection: &Connection) -> Result<(String, String)> {
    for key in KIRO_AUTH_KEY_PRIORITY {
        if let Some(value) = connection
            .query_row(
                "SELECT value FROM auth_kv WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Ok(((*key).to_string(), value));
        }
    }

    let fallback = connection
        .query_row(
            "SELECT key, value FROM auth_kv WHERE key LIKE '%:token' AND trim(value) != '' ORDER BY key LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    fallback.context("no logged-in Kiro credential found in auth_kv")
}

#[derive(Debug, Clone, serde::Deserialize)]
struct KiroProfileState {
    arn: String,
    profile_name: String,
    #[serde(default)]
    user_id: Option<String>,
}

fn read_kiro_profile_state(connection: &Connection) -> Result<Option<KiroProfileState>> {
    read_kiro_state_value(connection, KIRO_PROFILE_STATE_KEY)?
        .map(|value| {
            serde_json::from_str(&value).with_context(|| {
                format!("failed to parse Kiro state key '{KIRO_PROFILE_STATE_KEY}'")
            })
        })
        .transpose()
}

fn read_kiro_state_value(connection: &Connection, key: &str) -> Result<Option<String>> {
    connection
        .query_row(
            "SELECT value FROM state WHERE key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(Into::into)
}

fn parse_kiro_auth_kind(auth_key: &str, token_value: &Value, start_url: Option<&str>) -> String {
    match auth_key {
        "kirocli:social:token" => "social".to_string(),
        "kirocli:external-idp:token" => "external-idp".to_string(),
        _ => {
            let start_url = token_value
                .get("start_url")
                .or_else(|| token_value.get("startUrl"))
                .and_then(Value::as_str)
                .or(start_url);
            if matches!(start_url, Some(url) if !url.trim().is_empty() && url != KIRO_BUILDER_START_URL)
            {
                "identity-center".to_string()
            } else {
                "builder-id".to_string()
            }
        }
    }
}

fn parse_kiro_email(value: &Value) -> Option<String> {
    for key in ["email", "user_email", "userId", "user_id", "username"] {
        let candidate = value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|candidate| !candidate.is_empty())?;
        if candidate.contains('@') || key != "username" {
            return Some(candidate.to_string());
        }
    }
    None
}

fn kiro_auth_secret_from_context(context: &KiroImportContext) -> KiroAuthSecret {
    KiroAuthSecret {
        auth_key: context.auth_key.clone(),
        auth_kind: context.auth_kind.clone(),
        auth_json: context.raw_auth_json.clone(),
        email: context.email.clone(),
        profile_arn: context.profile_arn.clone(),
        profile_name: context.profile_name.clone(),
        start_url: context.start_url.clone(),
        region: context.region.clone(),
    }
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
    Ok(models.clone())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn read_kiro_auth_secret(codex_home: &Path) -> Result<KiroAuthSecret> {
    let path = codex_home.join(KIRO_CREDENTIALS_FILE);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    parse_kiro_auth_secret_text(&text)
        .with_context(|| format!("failed to parse {}", path.display()))
}

pub(crate) fn write_kiro_cli_data_dir(data_dir: &Path, secret: &KiroAuthSecret) -> Result<()> {
    ensure_private_kiro_data_dir(data_dir)?;
    let database_path = data_dir.join("data.sqlite3");
    let connection = Connection::open(&database_path)
        .with_context(|| format!("failed to open {}", database_path.display()))?;
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS migrations (
            id INTEGER PRIMARY KEY,
            version INTEGER NOT NULL,
            migration_time INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY,
            command TEXT,
            shell TEXT,
            pid INTEGER,
            session_id TEXT,
            cwd TEXT,
            start_time INTEGER,
            end_time INTEGER,
            duration INTEGER,
            hostname TEXT,
            exit_code INTEGER
        );
        CREATE TABLE IF NOT EXISTS state (
            key TEXT PRIMARY KEY,
            value BLOB
        );
        CREATE TABLE IF NOT EXISTS auth_kv (
            key TEXT PRIMARY KEY,
            value TEXT
        );
        CREATE TABLE IF NOT EXISTS conversations (
            key TEXT PRIMARY KEY,
            value TEXT
        );
        "#,
    )?;
    for version in 0..=7_i64 {
        connection.execute(
            "INSERT OR IGNORE INTO migrations (version, migration_time) VALUES (?1, strftime('%s', 'now'))",
            params![version],
        )?;
    }
    connection.execute(
        "INSERT OR REPLACE INTO auth_kv(key, value) VALUES(?1, ?2)",
        params![secret.auth_key, secret.auth_json],
    )?;
    if let Some(profile_state) = kiro_profile_state_json(secret)? {
        connection.execute(
            "INSERT OR REPLACE INTO state(key, value) VALUES(?1, ?2)",
            params![KIRO_PROFILE_STATE_KEY, profile_state],
        )?;
    } else {
        connection.execute(
            "DELETE FROM state WHERE key = ?1",
            params![KIRO_PROFILE_STATE_KEY],
        )?;
    }
    write_kiro_state_entry(
        &connection,
        KIRO_START_URL_STATE_KEY,
        secret.start_url.as_deref(),
    )?;
    write_kiro_state_entry(&connection, KIRO_REGION_STATE_KEY, secret.region.as_deref())?;
    Ok(())
}

fn ensure_private_kiro_data_dir(data_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(data_dir)
        .with_context(|| format!("failed to create {}", data_dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(data_dir, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("failed to make {} private", data_dir.display()))?;
    }
    Ok(())
}

fn write_kiro_auth_secret(codex_home: &Path, secret: &KiroAuthSecret) -> Result<()> {
    let path = codex_home.join(KIRO_CREDENTIALS_FILE);
    write_secret_text_file(
        &path,
        &serde_json::to_string_pretty(secret).context("failed to serialize Kiro auth secret")?,
    )
}

fn refresh_kiro_model_catalog_snapshot(codex_home: &Path, secret: &KiroAuthSecret) {
    let _ = write_kiro_model_catalog_snapshot(codex_home, secret);
}

pub(crate) fn write_kiro_model_catalog_snapshot(
    codex_home: &Path,
    secret: &KiroAuthSecret,
) -> Result<()> {
    let overlay_root = create_private_kiro_temp_root("catalog")?;
    let result = (|| {
        let data_dir = overlay_root.join("kiro-data");
        write_kiro_cli_data_dir(&data_dir, secret)?;
        let mut extra_env = vec![(OsString::from("Q_CLI_DATA_DIR"), data_dir.into_os_string())];
        if let Some(region) = secret
            .region
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            extra_env.push((OsString::from("AWS_REGION"), OsString::from(region)));
        }
        let cwd = env::current_dir().unwrap_or_else(|_| codex_home.to_path_buf());
        let bootstrap = runtime_kiro_acp_bootstrap(&cwd, &extra_env)?;
        let models = runtime_kiro_acp_model_catalog(&bootstrap.session);
        let path = codex_home.join(KIRO_MODEL_CATALOG_FILE);
        if models.is_empty() {
            let _ = std::fs::remove_file(&path);
            return Ok(());
        }
        write_secret_text_file(
            &path,
            &serde_json::to_string_pretty(&serde_json::json!({ "models": models }))
                .context("failed to serialize Kiro model catalog")?,
        )
    })();
    let _ = std::fs::remove_dir_all(overlay_root);
    result
}

pub(crate) fn create_private_kiro_temp_root(name: &str) -> Result<PathBuf> {
    for _ in 0..8 {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random)
            .context("failed to generate Kiro snapshot temp directory name")?;
        let path = env::temp_dir().join(format!(
            "prodex-kiro-{name}-{}-{}",
            std::process::id(),
            kiro_hex(&random)
        ));
        match create_private_kiro_temp_root_dir(&path) {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to create private {}", path.display()));
            }
        }
    }
    bail!("failed to create unique Kiro snapshot temp directory")
}

fn create_private_kiro_temp_root_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(path)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir(path)
    }
}

fn kiro_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn read_kiro_whoami_json() -> Result<Value> {
    let output = Command::new(kiro_bin())
        .args(["whoami", "--format", "json"])
        .output()
        .context("failed to execute Kiro CLI")?;
    if !output.status.success() {
        bail!(
            "Kiro CLI whoami failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("failed to parse Kiro whoami JSON")
}

fn kiro_profile_state_json(secret: &KiroAuthSecret) -> Result<Option<String>> {
    let Some(arn) = secret
        .profile_arn
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some(profile_name) = secret
        .profile_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    serde_json::to_string(&serde_json::json!({
        "arn": arn,
        "profile_name": profile_name,
        "user_id": secret.email.as_deref().map(str::trim).filter(|value| !value.is_empty()),
    }))
    .map(Some)
    .context("failed to serialize Kiro profile state")
}

fn write_kiro_state_entry(connection: &Connection, key: &str, value: Option<&str>) -> Result<()> {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        connection.execute(
            "INSERT OR REPLACE INTO state(key, value) VALUES(?1, ?2)",
            params![key, value],
        )?;
    } else {
        connection.execute("DELETE FROM state WHERE key = ?1", params![key])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TestEnvLockGuard, acquire_test_env_lock};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn kiro_snapshot_temp_dir_is_unique_and_private() {
        let first =
            create_private_kiro_temp_root("test").expect("first temp dir should be created");
        let second =
            create_private_kiro_temp_root("test").expect("second temp dir should be created");
        assert_ne!(first, second);
        assert!(first.is_dir());
        assert!(second.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&first).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(&second).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        let _ = fs::remove_dir_all(first);
        let _ = fs::remove_dir_all(second);
    }

    struct EnvGuard {
        _lock: TestEnvLockGuard,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set_kiro_bin(value: &Path) -> Self {
            let lock = acquire_test_env_lock();
            let previous = env::var_os("PRODEX_KIRO_BIN");
            unsafe { env::set_var("PRODEX_KIRO_BIN", value) };
            Self {
                _lock: lock,
                previous,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => unsafe { env::set_var("PRODEX_KIRO_BIN", value) },
                None => unsafe { env::remove_var("PRODEX_KIRO_BIN") },
            }
        }
    }

    fn write_fake_kiro_binary(root: &Path) -> PathBuf {
        let script = root.join("fake-kiro-cli");
        fs::write(
            &script,
            r#"#!/usr/bin/env python3
import json, sys
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
        .expect("fake kiro binary should be written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script, perms).expect("permissions should update");
        }
        script
    }

    #[test]
    fn write_kiro_cli_data_dir_materializes_auth_and_profile_state() {
        let data_dir = std::env::temp_dir().join(format!(
            "prodex-kiro-data-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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

        write_kiro_cli_data_dir(&data_dir, &secret).expect("data dir should materialize");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&data_dir).unwrap().permissions().mode() & 0o777,
                0o700
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

        let _ = std::fs::remove_dir_all(data_dir);
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
        let codex_home = root.join("codex-home");
        fs::create_dir_all(&codex_home).expect("codex home should exist");
        let fake_kiro = write_fake_kiro_binary(&root);
        let _guard = EnvGuard::set_kiro_bin(&fake_kiro);

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

        write_kiro_model_catalog_snapshot(&codex_home, &secret)
            .expect("model catalog snapshot should be written");
        let catalog_path = codex_home.join(KIRO_MODEL_CATALOG_FILE);
        let catalog_text = fs::read_to_string(&catalog_path).expect("catalog should exist");
        let value: Value = serde_json::from_str(&catalog_text).expect("catalog json should parse");
        assert_eq!(value["models"][0]["id"], "claude-sonnet-4");
        assert_eq!(value["models"][1]["id"], "claude-sonnet-4.5");
        let _ = fs::remove_dir_all(root);
    }
}
