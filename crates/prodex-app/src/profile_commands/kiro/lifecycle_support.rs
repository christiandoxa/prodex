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
use prodex_mojo_core::kiro_import_policy::{
    KiroAuthKeySource, KiroAuthKind, KiroImportAction, KiroImportActionError, kiro_auth_key_source,
    kiro_auth_kind, kiro_import_action, kiro_profile_match,
};
use prodex_mojo_core::profile_identity::{
    ProfileNameValidation, first_present_identity_source, is_trimmed_nonempty,
    optional_nonempty_trimmed_casefold_equal, profile_name_candidate, profile_name_from_email,
    should_activate_profile, trimmed_equal, validate_profile_name,
};
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
) -> Result<String> {
    let source =
        first_present_identity_source(&[context.email.is_some(), context.profile_name.is_some()])
            .map_err(|error| anyhow::anyhow!("Mojo Kiro profile-name source failed: {error:?}"))?;
    let base = match source {
        Some(0) => profile_name_from_email(&format!(
            "kiro-{}",
            context.email.as_deref().unwrap_or_default()
        ))
        .map_err(|error| {
            anyhow::anyhow!("Mojo Kiro profile-name normalization failed: {error:?}")
        })?,
        Some(1) => profile_name_from_email(&format!(
            "kiro-{}",
            context.profile_name.as_deref().unwrap_or_default()
        ))
        .map_err(|error| {
            anyhow::anyhow!("Mojo Kiro profile-name normalization failed: {error:?}")
        })?,
        None => "kiro".to_string(),
        Some(_) => return Err(anyhow::anyhow!("Mojo returned an invalid Kiro name source")),
    };
    let mut attempt = 0_u64;
    loop {
        let candidate = profile_name_candidate(&base, "kiro", attempt).map_err(|error| {
            anyhow::anyhow!("Mojo Kiro profile-name candidate failed: {error:?}")
        })?;
        if crate::profile_name_is_available(paths, state, &candidate) {
            return Ok(candidate);
        }
        attempt = attempt
            .checked_add(1)
            .context("Kiro profile-name candidate space exhausted")?;
    }
}

fn find_kiro_profile_by_identity(
    state: &AppState,
    context: &KiroImportContext,
) -> Result<Option<String>> {
    let mut names = Vec::with_capacity(state.profiles.len());
    let mut matches = Vec::with_capacity(state.profiles.len());
    for (name, profile) in &state.profiles {
        names.push(name);
        let flags = match &profile.provider {
            ProfileProvider::Kiro {
                auth_key,
                profile_arn,
                profile_name,
                ..
            } => {
                let auth_key_matches =
                    trimmed_equal(auth_key, &context.auth_key).map_err(|error| {
                        anyhow::anyhow!("Mojo Kiro auth-key match failed: {error:?}")
                    })?;
                let profile_arn_matches = optional_nonempty_trimmed_casefold_equal(
                    profile_arn.as_deref(),
                    context.profile_arn.as_deref(),
                )
                .map_err(|error| anyhow::anyhow!("Mojo Kiro ARN match failed: {error:?}"))?;
                let profile_name_matches = optional_nonempty_trimmed_casefold_equal(
                    profile_name.as_deref(),
                    context.profile_name.as_deref(),
                )
                .map_err(|error| {
                    anyhow::anyhow!("Mojo Kiro profile-name match failed: {error:?}")
                })?;
                i64::from(auth_key_matches)
                    | (i64::from(profile_arn_matches) << 1)
                    | (i64::from(profile_name_matches) << 2)
            }
            ProfileProvider::Openai
            | ProfileProvider::Gemini { .. }
            | ProfileProvider::Anthropic { .. }
            | ProfileProvider::Copilot { .. }
            | ProfileProvider::Agy { .. } => 0,
        };
        matches.push(flags);
    }
    let index = kiro_profile_match(&matches)
        .map_err(|error| anyhow::anyhow!("Mojo Kiro identity match failed: {error:?}"))?;
    Ok(index.and_then(|index| names.get(index).map(|name| (*name).clone())))
}

fn validate_existing_kiro_import_name(
    existing_name: &str,
    requested_name: Option<&str>,
) -> Result<()> {
    let (present, valid, matches) = kiro_requested_name_facts(existing_name, requested_name)?;
    match kiro_import_action(true, present, valid, matches) {
        Ok(KiroImportAction::Update) => Ok(()),
        Err(KiroImportActionError::InvalidRequestedName) => {
            bail!("invalid Kiro profile name")
        }
        Err(KiroImportActionError::ExistingNameMismatch) => {
            bail!(
                "Kiro identity is already imported as profile '{}'",
                existing_name
            )
        }
        Err(KiroImportActionError::Mojo(error)) => {
            Err(anyhow::anyhow!("Mojo Kiro import action failed: {error:?}"))
        }
        Ok(KiroImportAction::Create) => Err(anyhow::anyhow!(
            "Mojo returned create for an existing Kiro identity"
        )),
    }
}

fn kiro_requested_name_facts(
    existing_name: &str,
    requested_name: Option<&str>,
) -> Result<(bool, bool, bool)> {
    let Some(requested_name) = requested_name else {
        return Ok((false, true, true));
    };
    let present = is_trimmed_nonempty(requested_name)
        .map_err(|error| anyhow::anyhow!("Mojo Kiro profile-name presence failed: {error:?}"))?;
    if !present {
        return Ok((false, true, true));
    }
    let requested_name = requested_name.trim();
    let valid = matches!(
        validate_profile_name(requested_name).map_err(|error| anyhow::anyhow!(
            "Mojo Kiro profile-name validation failed: {error:?}"
        ))?,
        ProfileNameValidation::Valid
    );
    let matches = trimmed_equal(requested_name, existing_name)
        .map_err(|error| anyhow::anyhow!("Mojo Kiro profile-name comparison failed: {error:?}"))?;
    Ok((true, valid, matches))
}

fn resolve_kiro_import_context() -> Result<KiroImportContext> {
    let database_path = discover_kiro_database_path()?;
    let connection = Connection::open_with_flags(
        &database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("failed to open {}", database_path.display()))?;
    let (auth_key, raw_token, auth_source) = read_kiro_auth_token(&connection)?;
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
    let start_url = first_kiro_value([state_start_url, token_start_url])?;
    let region = first_kiro_value([state_region, token_region])?;
    let start_url_is_builder = match start_url.as_deref() {
        Some(value) => trimmed_equal(value, KIRO_BUILDER_START_URL).map_err(|error| {
            anyhow::anyhow!("Mojo Kiro start URL classification failed: {error:?}")
        })?,
        None => false,
    };
    let auth_kind = kiro_auth_kind(auth_source, start_url.is_some(), start_url_is_builder)
        .map_err(|error| anyhow::anyhow!("Mojo Kiro auth-kind classification failed: {error:?}"))?;
    let whoami_email = whoami.as_ref().map(parse_kiro_email).transpose()?.flatten();
    let profile_email = profile.as_ref().and_then(|profile| profile.user_id.clone());
    let email = first_kiro_value([
        first_kiro_value([parse_kiro_email(&token_value)?, whoami_email])?,
        profile_email,
    ])?;

    Ok(KiroImportContext {
        auth_key,
        auth_kind: match auth_kind {
            KiroAuthKind::Social => "social",
            KiroAuthKind::ExternalIdp => "external-idp",
            KiroAuthKind::IdentityCenter => "identity-center",
            KiroAuthKind::BuilderId => "builder-id",
        }
        .to_string(),
        raw_auth_json: raw_token,
        email,
        profile_arn: profile.as_ref().map(|profile| profile.arn.clone()),
        profile_name: profile.as_ref().map(|profile| profile.profile_name.clone()),
        start_url,
        region,
    })
}

fn first_kiro_value(values: [Option<String>; 2]) -> Result<Option<String>> {
    let mut normalized = [const { None }; 2];
    for (index, value) in values.into_iter().enumerate() {
        if let Some(value) = value {
            if !value.trim().is_empty() {
                normalized[index] = Some(value.trim().to_string());
            }
        }
    }
    let values = normalized;
    let present = values.each_ref().map(Option::is_some);
    let index = first_present_identity_source(&present)
        .map_err(|error| anyhow::anyhow!("Mojo Kiro source precedence failed: {error:?}"))?;
    Ok(index.and_then(|index| values.into_iter().nth(index).flatten()))
}

fn read_kiro_auth_token(
    connection: &Connection,
) -> Result<(String, String, Option<KiroAuthKeySource>)> {
    let mut values = [const { None }; 3];
    for (index, key) in KIRO_AUTH_KEY_PRIORITY.iter().enumerate() {
        values[index] = connection
            .query_row(
                "SELECT value FROM auth_kv WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|value| {
                let value = value.trim().to_string();
                (!value.is_empty()).then_some(value)
            });
    }
    let source = kiro_auth_key_source(values.each_ref().map(Option::is_some))
        .map_err(|error| anyhow::anyhow!("Mojo Kiro auth-key precedence failed: {error:?}"))?;
    if let Some(source) = source {
        let index = match source {
            KiroAuthKeySource::Social => 0,
            KiroAuthKeySource::ExternalIdp => 1,
            KiroAuthKeySource::OtherPriority => 2,
        };
        let value = values[index]
            .take()
            .context("Mojo selected a missing Kiro credential")?;
        return Ok((
            KIRO_AUTH_KEY_PRIORITY[index].to_string(),
            value,
            Some(source),
        ));
    }

    let fallback = connection
        .query_row(
            "SELECT key, value FROM auth_kv WHERE key LIKE '%:token' AND trim(value) != '' ORDER BY key LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    fallback
        .map(|(key, value)| (key, value, None))
        .context("no logged-in Kiro credential found in auth_kv")
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

fn parse_kiro_email(value: &Value) -> Result<Option<String>> {
    let keys = ["email", "user_email", "userId", "user_id", "username"];
    let mut candidates = [const { None }; 5];
    let mut present = [false; 5];
    for (index, key) in keys.iter().enumerate() {
        let candidate = value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|candidate| !candidate.is_empty())
            .filter(|candidate| *key != "username" || candidate.contains('@'))
            .map(str::to_string);
        present[index] = candidate.is_some();
        candidates[index] = candidate;
    }
    let index = first_present_identity_source(&present).map_err(|error| {
        anyhow::anyhow!("Mojo Kiro identity-source precedence failed: {error:?}")
    })?;
    Ok(index.and_then(|index| candidates.into_iter().nth(index).flatten()))
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
    let existing_name = find_kiro_profile_by_identity(&state, &context)?;
    let requested_facts = kiro_requested_name_facts(
        existing_name.as_deref().unwrap_or_default(),
        args.name.as_deref(),
    )?;
    let action = kiro_import_action(
        existing_name.is_some(),
        requested_facts.0,
        requested_facts.1,
        requested_facts.2,
    )
    .map_err(|error| match error {
        KiroImportActionError::InvalidRequestedName => {
            anyhow::anyhow!("Kiro profile name is invalid")
        }
        KiroImportActionError::ExistingNameMismatch => anyhow::anyhow!(
            "Kiro identity is already imported as profile '{}'",
            existing_name.as_deref().unwrap_or_default()
        ),
        KiroImportActionError::Mojo(error) => {
            anyhow::anyhow!("Mojo Kiro import action failed: {error:?}")
        }
    })?;
    let profile_name = match action {
        KiroImportAction::Update => {
            let existing_name = existing_name
                .as_deref()
                .context("Mojo selected Kiro update without a matching profile")?;
            let activate = should_activate_profile(state.active_profile.is_some(), args.activate)
                .map_err(|error| {
                anyhow::anyhow!("Mojo Kiro activation decision failed: {error:?}")
            })?;
            let profile = state
                .profiles
                .get(existing_name)
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
                existing_name,
                &desired_profile,
                if activate {
                    Some(existing_name.to_string())
                } else {
                    state.active_profile.clone()
                },
                ProfileAuthUpdate {
                    next_auth_json: None,
                    next_provider_json: Some(serde_json::to_string(&desired_profile.provider)?),
                    next_secret_files: vec![
                        prodex_profile_export::ImportedExistingProfileFileUpdate {
                            path: KIRO_CREDENTIALS_FILE.to_string(),
                            text: Some(serde_json::to_string_pretty(&auth_secret)?),
                        },
                    ],
                    previous_secret_file_paths: &[KIRO_CREDENTIALS_FILE],
                    temporary_home: None,
                },
            )?;
            prepare_profile_codex_home(&paths, profile)?;
            write_kiro_auth_secret(&profile_home, &auth_secret)?;
            let profile = state
                .profiles
                .get_mut(existing_name)
                .with_context(|| format!("profile '{}' is missing", existing_name))?;
            *profile = desired_profile;
            if activate {
                activate_profile(&mut state, existing_name);
            }
            state.save(&paths)?;
            let model_catalog_refreshed =
                refresh_kiro_model_catalog_snapshot(&profile_home, &auth_secret).is_ok();
            cleanup_profile_lifecycle_and_auth_journal(&lifecycle_path, &auth_journal_path)?;
            render_kiro_import_result(
                &state,
                existing_name,
                &context,
                true,
                model_catalog_refreshed,
            )?;
            audit_kiro_import(&state, existing_name, &context, true)?;
            existing_name.to_string()
        }
        KiroImportAction::Create => {
            let requested = args
                .name
                .as_deref()
                .filter(|_| requested_facts.0)
                .map(str::trim);
            match requested {
                Some(name) => name.to_string(),
                None => default_kiro_profile_name(&paths, &state, &context)?,
            }
        }
    };

    let activate = should_activate_profile(state.active_profile.is_some(), args.activate)
        .map_err(|error| anyhow::anyhow!("Mojo Kiro activation decision failed: {error:?}"))?;
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
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn kiro_import_rejects_a_different_name_for_an_existing_identity() {
        let error = validate_existing_kiro_import_name("kiro-main", Some("kiro-alt"))
            .expect_err("a differing requested name must not be ignored");

        assert!(
            error
                .to_string()
                .contains("already imported as profile 'kiro-main'")
        );
        assert!(validate_existing_kiro_import_name("kiro-main", Some("kiro-main")).is_ok());
    }

    #[test]
    fn kiro_import_new_identity_selects_create_action_for_a_valid_name() {
        let facts = kiro_requested_name_facts("", Some("kiro-new")).unwrap();
        assert_eq!(
            kiro_import_action(false, facts.0, facts.1, facts.2).unwrap(),
            KiroImportAction::Create
        );
    }

    #[test]
    fn kiro_import_identity_sources_skip_empty_fields_and_keep_unicode_precedence() {
        let value = serde_json::json!({
            "email": "",
            "user_email": "  ",
            "userId": "ユーザー",
            "username": "later@example.com"
        });
        assert_eq!(
            parse_kiro_email(&value).unwrap(),
            Some("ユーザー".to_string())
        );
        assert_eq!(
            parse_kiro_email(&serde_json::json!({"username": "later@example.com"})).unwrap(),
            Some("later@example.com".to_string())
        );
        assert_eq!(
            first_kiro_value([Some(" \u{2003}".to_string()), Some("chosen".to_string())]).unwrap(),
            Some("chosen".to_string())
        );
    }

    #[test]
    fn kiro_import_identity_match_rejects_conflicting_profile_facts() {
        let state = AppState {
            profiles: std::collections::BTreeMap::from([
                (
                    "wrong-arn".to_string(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/home/test-user/wrong"),
                        managed: true,
                        email: None,
                        provider: ProfileProvider::Kiro {
                            auth_key: "same-key".to_string(),
                            auth_kind: None,
                            profile_arn: Some("arn:wrong".to_string()),
                            profile_name: Some("same-profile".to_string()),
                            start_url: None,
                            region: None,
                        },
                    },
                ),
                (
                    "matching".to_string(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/home/test-user/matching"),
                        managed: true,
                        email: None,
                        provider: ProfileProvider::Kiro {
                            auth_key: " same-key ".to_string(),
                            auth_kind: None,
                            profile_arn: Some("ARN:EXAMPLE".to_string()),
                            profile_name: Some("same-profile".to_string()),
                            start_url: None,
                            region: None,
                        },
                    },
                ),
            ]),
            ..AppState::default()
        };
        let context = KiroImportContext {
            auth_key: "same-key".to_string(),
            auth_kind: "builder-id".to_string(),
            raw_auth_json: "{}".to_string(),
            email: None,
            profile_arn: Some("arn:example".to_string()),
            profile_name: Some("same-profile".to_string()),
            start_url: None,
            region: None,
        };
        assert_eq!(
            find_kiro_profile_by_identity(&state, &context).unwrap(),
            Some("matching".to_string())
        );
    }

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
