use super::{
    KIRO_PROFILE_STATE_KEY, KIRO_REGION_STATE_KEY, KIRO_START_URL_STATE_KEY, KiroAuthSecret,
    read_kiro_auth_secret, write_kiro_auth_secret,
};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, FixedOffset};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(super) const KIRO_DATA_DIR: &str = "kiro-data";

pub(crate) fn prepare_kiro_cli_data_dir(codex_home: &Path) -> Result<(PathBuf, KiroAuthSecret)> {
    let mut secret = read_kiro_auth_secret(codex_home)?;
    let data_dir = codex_home.join(KIRO_DATA_DIR);
    ensure_private_kiro_data_dir(&data_dir)?;
    let database_path = data_dir.join("data.sqlite3");
    let stored_auth = match std::fs::symlink_metadata(&database_path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            let connection = Connection::open_with_flags(
                &database_path,
                OpenFlags::SQLITE_OPEN_READ_ONLY
                    | OpenFlags::SQLITE_OPEN_NO_MUTEX
                    | OpenFlags::SQLITE_OPEN_NOFOLLOW,
            )
            .with_context(|| format!("failed to open {}", database_path.display()))?;
            connection.busy_timeout(std::time::Duration::from_secs(2))?;
            let auth_table_exists = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'auth_kv')",
                [],
                |row| row.get::<_, bool>(0),
            )?;
            if auth_table_exists {
                connection
                    .query_row(
                        "SELECT value FROM auth_kv WHERE key = ?1",
                        params![secret.auth_key],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
            } else {
                None
            }
        }
        Ok(_) => bail!("{} is not a regular Kiro database", database_path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", database_path.display()));
        }
    };
    if let Some(stored_auth) = stored_auth {
        let _: Value = serde_json::from_str(&stored_auth)
            .context("failed to parse persisted Kiro auth JSON")?;
        if kiro_auth_expiry(&secret.auth_json) > kiro_auth_expiry(&stored_auth) {
            write_kiro_cli_data_dir(&data_dir, &secret)?;
        } else if secret.auth_json != stored_auth {
            secret.auth_json = stored_auth;
            write_kiro_auth_secret(codex_home, &secret)?;
        }
    } else {
        write_kiro_cli_data_dir(&data_dir, &secret)?;
    }
    Ok((data_dir, secret))
}

fn kiro_auth_expiry(auth_json: &str) -> Option<DateTime<FixedOffset>> {
    let value: Value = serde_json::from_str(auth_json).ok()?;
    value
        .get("expires_at")
        .or_else(|| value.get("expiresAt"))
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
}

pub(super) fn write_kiro_cli_data_dir(data_dir: &Path, secret: &KiroAuthSecret) -> Result<()> {
    ensure_private_kiro_data_dir(data_dir)?;
    let database_path = data_dir.join("data.sqlite3");
    if std::fs::symlink_metadata(&database_path)
        .is_ok_and(|metadata| !metadata.file_type().is_file())
    {
        bail!("{} is not a regular Kiro database", database_path.display());
    }
    let connection = Connection::open_with_flags(
        &database_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .with_context(|| format!("failed to open {}", database_path.display()))?;
    connection.busy_timeout(std::time::Duration::from_secs(2))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&database_path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to make {} private", database_path.display()))?;
    }
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
        CREATE TABLE IF NOT EXISTS conversations_v2 (
            key TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            value TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (key, conversation_id)
        );
        CREATE INDEX IF NOT EXISTS idx_conversations_v2_key_updated
            ON conversations_v2(key, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_conversations_v2_updated_at
            ON conversations_v2(updated_at DESC);
        CREATE TABLE IF NOT EXISTS extracted_kas_versions (
            version TEXT PRIMARY KEY,
            last_used_at INTEGER NOT NULL
        );
        "#,
    )?;
    for version in 0..=9_i64 {
        connection.execute(
            "INSERT OR IGNORE INTO migrations (version, migration_time) VALUES (?1, strftime('%s', 'now'))",
            params![version],
        )?;
    }
    connection.execute(
        "DELETE FROM auth_kv WHERE key LIKE '%:token' AND key != ?1",
        params![secret.auth_key],
    )?;
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
    match std::fs::symlink_metadata(data_dir) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => bail!(
            "{} is not a regular Kiro data directory",
            data_dir.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(data_dir)
                .with_context(|| format!("failed to create {}", data_dir.display()))?
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", data_dir.display()));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(data_dir, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("failed to make {} private", data_dir.display()))?;
    }
    Ok(())
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
