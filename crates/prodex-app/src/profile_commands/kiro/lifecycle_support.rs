use super::super::import_export::{
    ProfileAuthUpdate, ProfileLifecycleHomeAction, ProfileLifecyclePlan,
    acquire_profile_lifecycle_lock, cleanup_profile_lifecycle_and_auth_journal,
    lifecycle_profile_state, load_profile_state_with_profile_recovery_locked,
    prepare_existing_profile_lifecycle, write_profile_lifecycle_plan,
};
#[cfg(test)]
use super::read_kiro_auth_secret;
use super::{
    KIRO_AUTH_KEY_PRIORITY, KIRO_BUILDER_START_URL, KIRO_CREDENTIALS_FILE, KIRO_PROFILE_STATE_KEY,
    KIRO_REGION_STATE_KEY, KIRO_START_URL_STATE_KEY, KiroAuthSecret, KiroImportContext,
    discover_kiro_database_path, read_kiro_whoami_json, refresh_kiro_model_catalog_snapshot,
    render_kiro_import_result, write_kiro_auth_secret,
};
use crate::{
    AppPaths, AppState, AppStateIoExt, ImportProfileArgs, ProfileEntry, ProfileProvider,
    activate_profile, audit_log_event, create_codex_home_if_missing, ensure_path_is_unique,
    managed_profile_home_path, prepare_managed_codex_home, prepare_profile_codex_home,
};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde_json::Value;

fn audit_kiro_import(
    state: &AppState,
    profile_name: &str,
    context: &KiroImportContext,
    updated_existing: bool,
) -> Result<()> {
    audit_log_event(
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
    )
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
        crate::profile_name_is_available(paths, state, candidate)
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
    let auth_secret = kiro_auth_secret_from_context(&context);

    let paths = AppPaths::discover()?;
    let _lock = acquire_profile_lifecycle_lock(&paths)?;
    let (mut state, _) = load_profile_state_with_profile_recovery_locked(&paths, true)?;
    let profile_name = if let Some(existing_name) = find_kiro_profile_by_identity(&state, &context)
    {
        let activate = state.active_profile.is_none() || args.activate;
        let profile = state
            .profiles
            .get(&existing_name)
            .with_context(|| format!("profile '{}' is missing", existing_name))?;
        let desired_profile = ProfileEntry {
            email: context.email.clone(),
            provider: provider.clone(),
            ..profile.clone()
        };
        let profile_home = profile.codex_home.clone();
        let (lifecycle_path, auth_journal_path) = prepare_existing_profile_lifecycle(
            &paths,
            "import",
            &state,
            &existing_name,
            &desired_profile,
            if activate {
                Some(existing_name.clone())
            } else {
                state.active_profile.clone()
            },
            ProfileAuthUpdate {
                next_auth_json: None,
                next_provider_json: Some(serde_json::to_string(&desired_profile.provider)?),
                next_secret_files: vec![prodex_profile_export::ImportedExistingProfileFileUpdate {
                    path: KIRO_CREDENTIALS_FILE.to_string(),
                    text: Some(serde_json::to_string_pretty(&auth_secret)?),
                }],
                previous_secret_file_paths: &[KIRO_CREDENTIALS_FILE],
                temporary_home: None,
            },
        )?;
        prepare_profile_codex_home(&paths, profile)?;
        write_kiro_auth_secret(&profile_home, &auth_secret)?;
        let profile = state
            .profiles
            .get_mut(&existing_name)
            .with_context(|| format!("profile '{}' is missing", existing_name))?;
        *profile = desired_profile;
        if activate {
            activate_profile(&mut state, &existing_name);
        }
        state.save(&paths)?;
        let model_catalog_refreshed =
            refresh_kiro_model_catalog_snapshot(&profile_home, &auth_secret).is_ok();
        cleanup_profile_lifecycle_and_auth_journal(&lifecycle_path, &auth_journal_path)?;
        render_kiro_import_result(
            &state,
            &existing_name,
            &context,
            true,
            model_catalog_refreshed,
        )?;
        audit_kiro_import(&state, &existing_name, &context, true)?;
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
    if codex_home.exists() {
        bail!(
            "managed profile home {} already exists",
            codex_home.display()
        );
    }
    let desired_profile = ProfileEntry {
        codex_home: codex_home.clone(),
        managed: true,
        email: context.email.clone(),
        provider: provider.clone(),
    };
    let lifecycle_path = write_profile_lifecycle_plan(
        &paths,
        "import",
        &ProfileLifecyclePlan {
            profile_states: vec![lifecycle_profile_state(
                &profile_name,
                None,
                Some(&desired_profile),
            )?],
            previous_active_profile: state.active_profile.clone(),
            next_active_profile: if activate {
                Some(profile_name.clone())
            } else {
                state.active_profile.clone()
            },
            home_actions: vec![ProfileLifecycleHomeAction::Create {
                path: codex_home.display().to_string(),
            }],
            auth_journal_paths: Vec::new(),
        },
    )?;
    create_codex_home_if_missing(&codex_home)?;
    prepare_managed_codex_home(&paths, &codex_home)?;
    write_kiro_auth_secret(&codex_home, &auth_secret)?;
    state.profiles.insert(profile_name.clone(), desired_profile);
    if activate {
        activate_profile(&mut state, &profile_name);
    }
    state.save(&paths)?;
    let model_catalog_refreshed =
        refresh_kiro_model_catalog_snapshot(&codex_home, &auth_secret).is_ok();
    render_kiro_import_result(
        &state,
        &profile_name,
        &context,
        false,
        model_catalog_refreshed,
    )?;
    audit_kiro_import(&state, &profile_name, &context, false)?;
    prodex_profile_export::cleanup_profile_lifecycle_journal(&lifecycle_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn kiro_lifecycle_recovery_restores_credentials_before_state_consumption() {
        let root = std::env::temp_dir().join(format!(
            "prodex-kiro-lifecycle-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy"),
        };
        create_codex_home_if_missing(&paths.root).unwrap();
        create_codex_home_if_missing(&paths.managed_profiles_root).unwrap();
        let codex_home = paths.managed_profiles_root.join("kiro-main");
        create_codex_home_if_missing(&codex_home).unwrap();
        let old_secret = KiroAuthSecret {
            auth_key: "codewhisperer:odic:token".to_string(),
            auth_kind: "builder-id".to_string(),
            auth_json: serde_json::json!({"access_token":"old-token"}).to_string(),
            email: Some("old@example.com".to_string()),
            profile_arn: None,
            profile_name: Some("old-profile".to_string()),
            start_url: None,
            region: Some("us-east-1".to_string()),
        };
        write_kiro_auth_secret(&codex_home, &old_secret).unwrap();
        let state = AppState {
            active_profile: Some("kiro-main".to_string()),
            profiles: std::collections::BTreeMap::from([(
                "kiro-main".to_string(),
                ProfileEntry {
                    codex_home: codex_home.clone(),
                    managed: true,
                    email: old_secret.email.clone(),
                    provider: ProfileProvider::Kiro {
                        auth_key: old_secret.auth_key.clone(),
                        auth_kind: Some(old_secret.auth_kind.clone()),
                        profile_arn: old_secret.profile_arn.clone(),
                        profile_name: old_secret.profile_name.clone(),
                        start_url: old_secret.start_url.clone(),
                        region: old_secret.region.clone(),
                    },
                },
            )]),
            ..AppState::default()
        };
        state.save(&paths).unwrap();
        let mut desired = state.profiles["kiro-main"].clone();
        desired.email = Some("new@example.com".to_string());
        let new_secret = KiroAuthSecret {
            auth_key: old_secret.auth_key.clone(),
            auth_kind: old_secret.auth_kind.clone(),
            auth_json: serde_json::json!({"access_token":"new-token"}).to_string(),
            email: desired.email.clone(),
            profile_arn: None,
            profile_name: Some("new-profile".to_string()),
            start_url: None,
            region: Some("us-east-1".to_string()),
        };
        let (lifecycle_path, auth_path) =
            crate::profile_commands::import_export::prepare_existing_profile_lifecycle(
                &paths,
                "import",
                &state,
                "kiro-main",
                &desired,
                Some("kiro-main".to_string()),
                ProfileAuthUpdate {
                    next_auth_json: None,
                    next_provider_json: Some(serde_json::to_string(&desired.provider).unwrap()),
                    next_secret_files: vec![
                        prodex_profile_export::ImportedExistingProfileFileUpdate {
                            path: KIRO_CREDENTIALS_FILE.to_string(),
                            text: Some(serde_json::to_string_pretty(&new_secret).unwrap()),
                        },
                    ],
                    previous_secret_file_paths: &[KIRO_CREDENTIALS_FILE],
                    temporary_home: None,
                },
            )
            .unwrap();
        write_kiro_auth_secret(&codex_home, &new_secret).unwrap();
        let (recovered, _) =
            crate::profile_commands::import_export::load_profile_state_with_profile_recovery(
                &paths, true,
            )
            .unwrap();
        assert_eq!(read_kiro_auth_secret(&codex_home).unwrap(), old_secret);
        assert_eq!(
            recovered.profiles["kiro-main"].email,
            Some("old@example.com".to_string())
        );
        assert!(!lifecycle_path.exists());
        assert!(!auth_path.exists());
        let _ = fs::remove_dir_all(root);
    }
}
