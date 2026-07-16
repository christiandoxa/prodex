use anyhow::{Context, Result, bail};
use std::path::Path;

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
    let mut state = AppState::load(&paths)?;
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
                .with_context(|| format!("profile '{}' is missing", existing))?;
            prepare_profile_codex_home(&paths, profile)?;
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
            prepare_managed_codex_home(&paths, &codex_home)?;
            copy_claude_oauth_credentials(&source_config_dir, &codex_home)?;
            state.profiles.insert(
                profile_name.clone(),
                ProfileEntry {
                    codex_home,
                    managed: true,
                    email: account.clone(),
                    provider: ProfileProvider::Anthropic {
                        account: account.clone(),
                        auth_method: auth_method.clone(),
                    },
                },
            );
            if args.activate || state.active_profile.is_none() {
                state.active_profile = Some(profile_name.clone());
            }
            state.save(&paths)?;
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
