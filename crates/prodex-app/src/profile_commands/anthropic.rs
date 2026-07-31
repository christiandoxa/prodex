use anyhow::{Context, Result, bail};
use std::path::Path;

use super::import_export::{
    ProfileAuthUpdate, ProfileLifecycleHomeAction, ProfileLifecyclePlan,
    acquire_profile_lifecycle_lock, cleanup_profile_lifecycle_and_auth_journal,
    lifecycle_profile_state, load_profile_state_with_profile_recovery_locked,
    prepare_existing_profile_lifecycle, read_optional_secret_text_file,
    write_profile_lifecycle_plan,
};
use super::manage::print_profile_panel;
use crate::{
    AppPaths, AppState, AppStateIoExt, ImportProfileArgs, ProfileEntry, ProfileProvider,
    claude_config_dir_from_env_or_default, claude_oauth_profile_identity,
    copy_claude_oauth_credentials, ensure_path_is_unique, managed_profile_home_path,
    prepare_managed_codex_home, prepare_profile_codex_home,
};

pub(super) fn is_claude_import_source(path: &Path) -> bool {
    path.components().count() == 1
        && path
            .to_str()
            .is_some_and(|value| value.eq_ignore_ascii_case("claude"))
        && !path.exists()
}

pub(crate) fn handle_import_claude_profile(args: &ImportProfileArgs) -> Result<()> {
    let source_config_dir = claude_config_dir_from_env_or_default()?;
    let (account, auth_method) = claude_oauth_profile_identity(&source_config_dir).with_context(
        || {
            format!(
                "failed to read Claude Code credentials from {}. Run `claude auth login --claudeai` first, or use `prodex login --with-claude`.",
                source_config_dir.display()
            )
        },
    )?;

    let paths = AppPaths::discover()?;
    let _lock = acquire_profile_lifecycle_lock(&paths)?;
    let (mut state, _) = load_profile_state_with_profile_recovery_locked(&paths, true)?;
    let existing_profile_name = state.profiles.iter().find_map(|(name, profile)| {
        profile
            .provider
            .anthropic_matches(account.as_deref(), auth_method.as_deref())
            .then(|| name.clone())
    });
    let (profile_name, updated_existing) = match existing_profile_name {
        Some(existing) if args.name.is_none() => {
            let profile = state
                .profiles
                .get(&existing)
                .with_context(|| format!("profile '{}' is missing", existing))?
                .clone();
            let credentials = read_optional_secret_text_file(
                &source_config_dir.join(crate::CLAUDE_CREDENTIALS_FILE),
            )?
            .context("Claude credentials file is missing")?;
            let desired_profile = ProfileEntry {
                email: account.clone(),
                provider: ProfileProvider::Anthropic {
                    account: account.clone(),
                    auth_method: auth_method.clone(),
                },
                ..profile.clone()
            };
            let next_active_profile = if args.activate {
                Some(existing.clone())
            } else {
                state.active_profile.clone()
            };
            let (lifecycle_path, auth_journal_path) = prepare_existing_profile_lifecycle(
                &paths,
                "import",
                &state,
                &existing,
                &desired_profile,
                next_active_profile,
                ProfileAuthUpdate {
                    next_auth_json: None,
                    next_provider_json: Some(serde_json::to_string(&desired_profile.provider)?),
                    next_secret_files: vec![
                        prodex_profile_export::ImportedExistingProfileFileUpdate {
                            path: crate::CLAUDE_CREDENTIALS_FILE.to_string(),
                            text: Some(credentials),
                        },
                    ],
                    previous_secret_file_paths: &[crate::CLAUDE_CREDENTIALS_FILE],
                    temporary_home: None,
                },
            )?;
            prepare_profile_codex_home(&paths, &profile)?;
            copy_claude_oauth_credentials(&source_config_dir, &profile.codex_home)?;
            let profile = state
                .profiles
                .get_mut(&existing)
                .with_context(|| format!("profile '{}' is missing", existing))?;
            profile.email = account.clone();
            profile.provider = ProfileProvider::Anthropic {
                account: account.clone(),
                auth_method: auth_method.clone(),
            };
            if args.activate {
                state.active_profile = Some(existing.clone());
            }
            state.save(&paths)?;
            cleanup_profile_lifecycle_and_auth_journal(&lifecycle_path, &auth_journal_path)?;
            (existing, true)
        }
        _ => {
            let profile_name = match args.name.as_deref() {
                Some(name) => {
                    prodex_profile_identity::validate_profile_name(name)?;
                    name.to_string()
                }
                None => default_anthropic_profile_name(&paths, &state, account.as_deref()),
            };
            if state.profiles.contains_key(&profile_name) {
                bail!("profile '{}' already exists", profile_name);
            }
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
                email: account.clone(),
                provider: ProfileProvider::Anthropic {
                    account: account.clone(),
                    auth_method: auth_method.clone(),
                },
            };
            let next_active_profile = if args.activate || state.active_profile.is_none() {
                Some(profile_name.clone())
            } else {
                state.active_profile.clone()
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
                    next_active_profile,
                    home_actions: vec![ProfileLifecycleHomeAction::Create {
                        path: codex_home.display().to_string(),
                    }],
                    auth_journal_paths: Vec::new(),
                },
            )?;
            prepare_managed_codex_home(&paths, &codex_home)?;
            copy_claude_oauth_credentials(&source_config_dir, &codex_home)?;
            state.profiles.insert(profile_name.clone(), desired_profile);
            if args.activate || state.active_profile.is_none() {
                state.active_profile = Some(profile_name.clone());
            }
            state.save(&paths)?;
            prodex_profile_export::cleanup_profile_lifecycle_journal(&lifecycle_path);
            (profile_name, false)
        }
    };

    print_profile_panel(
        if updated_existing {
            "Profile Updated"
        } else {
            "Profile Added"
        },
        &[
            (
                "Result".to_string(),
                if updated_existing {
                    format!("Imported Claude credentials into profile '{profile_name}'.")
                } else {
                    format!("Created Anthropic profile '{profile_name}' from Claude credentials.")
                },
            ),
            (
                "Account".to_string(),
                account.unwrap_or_else(|| "-".to_string()),
            ),
            ("Provider".to_string(), "Anthropic Claude".to_string()),
            ("Profile".to_string(), profile_name),
        ],
    )?;
    Ok(())
}

fn default_anthropic_profile_name(
    paths: &AppPaths,
    state: &AppState,
    account: Option<&str>,
) -> String {
    let base = account
        .map(|account| {
            prodex_profile_identity::profile_name_from_email(&format!("claude-{account}"))
        })
        .unwrap_or_else(|| "claude".to_string());
    prodex_profile_identity::unique_profile_name_from_base(&base, "claude", |candidate| {
        crate::profile_name_is_available(paths, state, candidate)
    })
}
