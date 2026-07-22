use prodex_profile_export::{
    ImportedExistingProfileAuthUpdateJournal, ImportedExistingProfileFileRollback,
    ProfileImportAuthUpdatePlan, ProfileImportIdentity, ProfileImportPlanAction,
    ProfileImportPlanInput,
};

use super::super::kiro::{
    KIRO_CREDENTIALS_FILE, KIRO_MODEL_CATALOG_FILE, parse_kiro_auth_secret_text,
    parse_kiro_model_catalog_text,
};
use super::super::manage::print_profile_panel;
use super::passwords::read_profile_export_payload;
use super::progress::print_profile_import_progress;
use super::secrets::write_secret_text_file;
use super::*;

pub(crate) fn handle_import_profiles(args: ImportProfileArgs) -> Result<()> {
    if super::super::anthropic::is_claude_import_source(&args.path) {
        return handle_import_claude_profile(&args);
    }
    if super::super::copilot::is_copilot_import_source(&args.path) {
        return handle_import_copilot_profile(&args);
    }
    if super::super::kiro::is_kiro_import_source(&args.path) {
        return handle_import_kiro_profile(&args);
    }
    if args.name.is_some() || args.activate {
        bail!(
            "--name and --activate are only supported for built-in import sources such as `claude`, `copilot`, or `kiro`"
        );
    }

    let bundle_path = absolutize(args.path)?;
    let (payload, encrypted) = read_profile_export_payload(&bundle_path)?;
    print_profile_import_progress(&format!(
        "Importing {} profile(s)...",
        payload.profiles.len()
    ))?;
    let source_active_profile = payload.active_profile.clone();

    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    print_profile_import_progress("Checking existing profiles...")?;
    let recovered_auth_updates = recover_imported_auth_update_journals(&paths, &mut state)?;
    if recovered_auth_updates > 0 {
        print_profile_import_progress("Recovering interrupted profile import...")?;
        state
            .save(&paths)
            .context("failed to save recovered import auth rollback state")?;
    }
    print_profile_import_progress("Staging imported profiles...")?;
    let commit = import_profile_export_payload(&paths, &mut state, &payload)?;
    print_profile_import_progress("Saving imported profiles...")?;
    if let Err(err) = state.save(&paths) {
        rollback_imported_profiles(&mut state, &commit)
            .with_context(|| format!("failed to roll back profile import after: {err:#}"))?;
        return Err(err);
    }
    print_profile_import_progress("Profile import complete.")?;
    prodex_profile_export::cleanup_imported_auth_update_journals(&commit);
    audit_log_event(
        "profile",
        "import",
        "success",
        serde_json::json!({
            "profile_count": payload.profiles.len(),
            "imported_profile_count": commit.imported_names.len(),
            "updated_existing_profile_count": commit.updated_existing_names.len(),
            "updated_existing_profile_names": commit.updated_existing_names.clone(),
            "bundle_path": bundle_path.display().to_string(),
            "encrypted": encrypted,
            "source_active_profile": source_active_profile.clone(),
            "active_profile": state.active_profile.clone(),
        }),
    )?;

    let fields = prodex_profile_export::profile_import_summary_fields(
        prodex_profile_export::ProfileImportSummary {
            imported_count: commit.imported_names.len(),
            updated_existing_count: commit.updated_existing_names.len(),
            path: bundle_path.display().to_string(),
            encrypted,
            source_active_profile,
            active_profile: state.active_profile.clone(),
        },
    );
    print_profile_panel("Profile Import", &fields)?;
    Ok(())
}

pub(crate) fn handle_import_current_profile(args: ImportCurrentArgs) -> Result<()> {
    handle_add_profile(AddProfileArgs {
        name: args.name,
        codex_home: None,
        copy_from: None,
        copy_current: true,
        activate: true,
    })
}

pub(crate) fn count_profile_import_auth_journals(paths: &AppPaths) -> Result<usize> {
    Ok(prodex_profile_export::profile_import_auth_update_journal_paths(&paths.root)?.len())
}

pub(crate) fn repair_profile_import_auth_journals(
    paths: &AppPaths,
    state: &mut AppState,
) -> Result<usize> {
    recover_imported_auth_update_journals(paths, state)
}

pub(in crate::profile_commands) fn import_profile_export_payload(
    paths: &AppPaths,
    state: &mut AppState,
    payload: &ProfileExportPayload,
) -> Result<ImportedProfilesCommit> {
    let prepared = stage_imported_profiles(paths, state, payload)?;
    let mut transaction = ImportedProfilesTransaction::new(
        state.active_profile.clone(),
        prepared.staged_profiles.len(),
        prepared.auth_updates.len() + prepared.existing_profile_updates.len(),
    );

    if let Err(err) = apply_imported_profiles(paths, state, payload, &prepared, &mut transaction) {
        rollback_partial_imported_profiles(state, &transaction).with_context(|| {
            format!("failed to roll back partial profile import after: {err:#}")
        })?;
        return Err(err);
    }

    Ok(transaction.into_commit())
}

fn apply_imported_profiles(
    paths: &AppPaths,
    state: &mut AppState,
    payload: &ProfileExportPayload,
    prepared: &PreparedImportedProfiles,
    transaction: &mut ImportedProfilesTransaction,
) -> Result<()> {
    apply_imported_existing_auth_updates(paths, state, &prepared.auth_updates, transaction)?;
    apply_imported_existing_profile_updates(
        paths,
        state,
        &prepared.existing_profile_updates,
        transaction,
    )?;
    finalize_staged_imported_profiles(state, &prepared.staged_profiles, transaction)?;
    activate_imported_profile_from_payload(state, payload, prepared);
    Ok(())
}

fn apply_imported_existing_auth_updates(
    paths: &AppPaths,
    state: &mut AppState,
    prepared_updates: &[ProfileImportAuthUpdatePlan],
    transaction: &mut ImportedProfilesTransaction,
) -> Result<()> {
    for update in prepared_updates {
        let previous = state
            .profiles
            .get(&update.target_profile_name)
            .with_context(|| format!("profile '{}' is missing", update.target_profile_name))?
            .clone();
        let previous_auth_json = read_auth_json_text(&previous.codex_home).with_context(|| {
            format!(
                "failed to read {}",
                secret_store::auth_json_path(&previous.codex_home).display()
            )
        })?;
        let previous_email = previous.email.clone();
        let mut rollback = ImportedExistingProfileAuthUpdate {
            profile_name: update.target_profile_name.clone(),
            codex_home: previous.codex_home,
            previous_auth_json,
            previous_email,
            journal_path: None,
            restore_auth_json: true,
            previous_provider_json: None,
            previous_secret_files: Vec::new(),
        };
        rollback.journal_path = Some(write_imported_auth_update_journal(paths, &rollback)?);
        let updated = match update_existing_profile_auth(
            paths,
            state,
            &update.target_profile_name,
            update.email.as_deref(),
            &update.auth_json,
            false,
        ) {
            Ok(updated) => updated,
            Err(err) => {
                rollback_imported_auth_updates(state, std::slice::from_ref(&rollback))
                    .with_context(|| {
                        format!(
                            "failed to roll back auth update for '{}' after: {err:#}",
                            update.target_profile_name
                        )
                    })?;
                return Err(err);
            }
        };
        debug_assert_eq!(rollback.profile_name, updated.profile_name);
        debug_assert_eq!(rollback.codex_home, updated.codex_home);
        transaction.record_existing_auth_update(rollback);
    }

    Ok(())
}

fn apply_imported_existing_profile_updates(
    paths: &AppPaths,
    state: &mut AppState,
    prepared_updates: &[PreparedExistingProfileUpdate],
    transaction: &mut ImportedProfilesTransaction,
) -> Result<()> {
    for update in prepared_updates {
        let profile = state
            .profiles
            .get(&update.name)
            .with_context(|| format!("profile '{}' is missing", update.name))?
            .clone();
        prepare_profile_codex_home(paths, &profile)?;
        let previous_secret_files = update
            .secret_files
            .iter()
            .map(|secret_file| {
                validate_exported_secret_file_path(&secret_file.path, &update.name)?;
                Ok(ImportedExistingProfileFileRollback {
                    path: secret_file.path.clone(),
                    previous_text: read_optional_secret_text_file(
                        &profile.codex_home.join(&secret_file.path),
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let previous_provider_json = serde_json::to_string(&profile.provider)
            .context("failed to serialize existing profile provider")?;
        let mut rollback = ImportedExistingProfileAuthUpdate {
            profile_name: update.name.clone(),
            codex_home: profile.codex_home.clone(),
            previous_auth_json: None,
            previous_email: profile.email.clone(),
            journal_path: None,
            restore_auth_json: false,
            previous_provider_json: Some(previous_provider_json),
            previous_secret_files,
        };
        let journal_path = write_imported_auth_update_journal(paths, &rollback)?;
        rollback.journal_path = Some(journal_path.clone());
        let applied = (|| -> Result<()> {
            for secret_file in &update.secret_files {
                write_secret_text_file(
                    &profile.codex_home.join(&secret_file.path),
                    &secret_file.text,
                )?;
            }
            let profile_entry = state
                .profiles
                .get_mut(&update.name)
                .with_context(|| format!("profile '{}' is missing", update.name))?;
            profile_entry.email = update.email.clone();
            profile_entry.provider = update.provider.clone();
            Ok(())
        })();
        if let Err(err) = applied {
            rollback_imported_auth_updates(state, std::slice::from_ref(&rollback)).with_context(
                || {
                    format!(
                        "failed to roll back profile update for '{}' after: {err:#}",
                        update.name
                    )
                },
            )?;
            let _ = fs::remove_file(journal_path);
            return Err(err);
        }
        transaction.record_existing_auth_update(rollback);
    }

    Ok(())
}

fn finalize_staged_imported_profiles(
    state: &mut AppState,
    staged_profiles: &[StagedImportedProfile],
    transaction: &mut ImportedProfilesTransaction,
) -> Result<()> {
    for staged in staged_profiles {
        fs::rename(&staged.staging_home, &staged.final_home).with_context(|| {
            format!(
                "failed to finalize imported profile home {}",
                staged.final_home.display()
            )
        })?;
        transaction.record_imported_profile(staged.name.clone(), staged.final_home.clone());
        state.profiles.insert(
            staged.name.clone(),
            ProfileEntry {
                codex_home: staged.final_home.clone(),
                managed: true,
                email: staged.email.clone(),
                provider: staged.provider.clone(),
            },
        );
    }

    Ok(())
}

fn activate_imported_profile_from_payload(
    state: &mut AppState,
    payload: &ProfileExportPayload,
    prepared: &PreparedImportedProfiles,
) {
    state.active_profile = prodex_profile_export::resolve_imported_active_profile(
        state.active_profile.as_deref(),
        payload.active_profile.as_deref(),
        &prepared.resolved_profile_names,
    );
}

fn rollback_imported_profiles(state: &mut AppState, commit: &ImportedProfilesCommit) -> Result<()> {
    for name in &commit.imported_names {
        state.profiles.remove(name);
        state.last_run_selected_at.remove(name);
        state
            .response_profile_bindings
            .retain(|_, binding| binding.profile_name != *name);
        state
            .session_profile_bindings
            .retain(|_, binding| binding.profile_name != *name);
    }
    rollback_imported_auth_updates(state, &commit.auth_updates)?;
    state.active_profile = commit.previous_active_profile.clone();
    prodex_profile_export::remove_committed_import_homes(&commit.committed_homes);
    Ok(())
}

fn rollback_partial_imported_profiles(
    state: &mut AppState,
    transaction: &ImportedProfilesTransaction,
) -> Result<()> {
    for name in &transaction.imported_names {
        state.profiles.remove(name);
    }
    rollback_imported_auth_updates(state, &transaction.auth_updates)?;
    state.active_profile = transaction.previous_active_profile.clone();
    prodex_profile_export::remove_committed_import_homes(&transaction.committed_homes);
    Ok(())
}

pub(super) fn rollback_imported_auth_updates(
    state: &mut AppState,
    auth_updates: &[ImportedExistingProfileAuthUpdate],
) -> Result<()> {
    for update in auth_updates.iter().rev() {
        let profile = state
            .profiles
            .get_mut(&update.profile_name)
            .with_context(|| format!("profile '{}' is missing", update.profile_name))?;
        profile.email = update.previous_email.clone();
        if let Some(previous_provider_json) = update.previous_provider_json.as_deref() {
            profile.provider = serde_json::from_str(previous_provider_json).with_context(|| {
                format!(
                    "failed to restore provider for profile '{}'",
                    update.profile_name
                )
            })?;
        }
        if update.restore_auth_json {
            restore_optional_secret_text_file(
                &secret_store::auth_json_path(&update.codex_home),
                update.previous_auth_json.as_deref(),
            )?;
        }
        for secret_file in &update.previous_secret_files {
            validate_exported_secret_file_path(&secret_file.path, &update.profile_name)?;
            restore_optional_secret_text_file(
                &update.codex_home.join(&secret_file.path),
                secret_file.previous_text.as_deref(),
            )?;
        }
    }
    Ok(())
}

pub(super) fn recover_imported_auth_update_journals(
    paths: &AppPaths,
    state: &mut AppState,
) -> Result<usize> {
    let journal_root = prodex_profile_export::profile_import_auth_update_journal_root(&paths.root);
    let journal_paths =
        prodex_profile_export::profile_import_auth_update_journal_paths(&paths.root)?;

    let mut journals = Vec::new();
    for journal_path in journal_paths {
        let journal =
            prodex_profile_export::read_profile_import_auth_update_journal(&journal_path)?;
        let profile = state.profiles.get(&journal.profile_name).with_context(|| {
            format!(
                "auth update journal {} references missing profile '{}'",
                journal_path.display(),
                journal.profile_name
            )
        })?;
        let journal_codex_home = PathBuf::from(&journal.codex_home);
        if journal_codex_home != profile.codex_home {
            bail!(
                "auth update journal {} targets {} but profile '{}' uses {}",
                journal_path.display(),
                journal_codex_home.display(),
                journal.profile_name,
                profile.codex_home.display()
            );
        }
        journals.push((journal_path, journal));
    }
    journals.sort_by(|left, right| {
        right
            .1
            .created_at
            .cmp(&left.1.created_at)
            .then_with(|| right.0.cmp(&left.0))
    });

    let mut recovered = 0;
    for (journal_path, journal) in journals {
        rollback_imported_auth_updates(
            state,
            &[ImportedExistingProfileAuthUpdate {
                profile_name: journal.profile_name,
                codex_home: PathBuf::from(journal.codex_home),
                previous_auth_json: journal.previous_auth_json,
                previous_email: journal.previous_email,
                journal_path: Some(journal_path.clone()),
                restore_auth_json: journal.restore_auth_json,
                previous_provider_json: journal.previous_provider_json,
                previous_secret_files: journal.previous_secret_files,
            }],
        )?;
        fs::remove_file(&journal_path)
            .with_context(|| format!("failed to remove {}", journal_path.display()))?;
        recovered += 1;
    }

    let _ = fs::remove_dir(&journal_root);
    Ok(recovered)
}

pub(super) fn stage_imported_profiles(
    paths: &AppPaths,
    state: &mut AppState,
    payload: &ProfileExportPayload,
) -> Result<PreparedImportedProfiles> {
    if payload.profiles.is_empty() {
        bail!("profile export bundle does not contain any profiles");
    }
    for exported in &payload.profiles {
        prodex_profile_identity::validate_profile_name(&exported.name)?;
    }
    prodex_profile_export::validate_profile_import_source_names(
        payload
            .profiles
            .iter()
            .map(|exported| exported.name.as_str()),
    )?;

    ensure_managed_profiles_root(paths)?;

    let mut plan_inputs = Vec::with_capacity(payload.profiles.len());
    for exported in &payload.profiles {
        validate_exported_secret_files(exported)?;
        let supports_codex_runtime = exported.provider.supports_codex_runtime();
        if supports_codex_runtime {
            let _: StoredAuth = serde_json::from_str(&exported.auth_json).with_context(|| {
                format!(
                    "failed to parse exported auth.json for profile '{}'",
                    exported.name
                )
            })?;
        }
        let auth_identity = parse_identity_from_auth_json(&exported.auth_json).unwrap_or_default();
        let resolved_identity = prodex_profile_export::resolve_profile_import_identity(
            ProfileImportIdentity {
                email: auth_identity.email,
                account_id: auth_identity.account_id,
            },
            exported.email.as_deref(),
        );
        plan_inputs.push(ProfileImportPlanInput {
            profile_name: exported.name.clone(),
            identity: resolved_identity,
            supports_codex_runtime,
        });
    }

    let existing_profile_runtime_support = state
        .profiles
        .iter()
        .map(|(name, profile)| (name.clone(), profile.provider.supports_codex_runtime()))
        .collect::<BTreeMap<_, _>>();
    let plan = prodex_profile_export::plan_profile_import(
        &plan_inputs,
        |profile_name| existing_profile_runtime_support.get(profile_name).copied(),
        |identity| {
            find_profile_by_identity(
                state,
                &ProfileIdentity {
                    email: identity.email.clone(),
                    account_id: identity.account_id.clone(),
                },
            )
        },
    )?;

    let mut staged_profiles = Vec::with_capacity(payload.profiles.len());
    let mut auth_updates = Vec::new();
    let mut existing_profile_updates = Vec::new();
    let result = (|| -> Result<()> {
        for action in &plan.actions {
            match action {
                ProfileImportPlanAction::UpdateExisting {
                    source_index,
                    target_profile_name,
                } => {
                    let exported = payload.profiles.get(*source_index).with_context(|| {
                        format!("import plan source index {} is missing", source_index)
                    })?;
                    let existing = state
                        .profiles
                        .get(target_profile_name)
                        .with_context(|| format!("profile '{}' is missing", target_profile_name))?;
                    if existing.provider.label() != exported.provider.label() {
                        bail!(
                            "profile '{}' already exists with provider '{}' and cannot be imported as '{}'",
                            target_profile_name,
                            existing.provider.label(),
                            exported.provider.label(),
                        );
                    }
                    if plan_inputs[*source_index].supports_codex_runtime {
                        prodex_profile_export::queue_profile_import_auth_update(
                            &mut auth_updates,
                            target_profile_name,
                            plan_inputs[*source_index].identity.email.clone(),
                            exported.auth_json.clone(),
                        );
                    } else {
                        existing_profile_updates.push(PreparedExistingProfileUpdate {
                            name: target_profile_name.clone(),
                            email: plan_inputs[*source_index].identity.email.clone(),
                            provider: exported.provider.clone(),
                            secret_files: exported.secret_files.clone(),
                        });
                    }
                }
                ProfileImportPlanAction::StageNew {
                    source_index,
                    staged_index,
                } => {
                    if staged_profiles.len() != *staged_index {
                        bail!(
                            "staged import profile index {} is out of order",
                            staged_index
                        );
                    }
                    let exported = payload.profiles.get(*source_index).with_context(|| {
                        format!("import plan source index {} is missing", source_index)
                    })?;
                    let final_home = managed_profile_home_path(paths, &exported.name)?;
                    ensure_path_is_unique(state, &final_home)?;
                    if final_home.exists() {
                        bail!(
                            "managed profile home {} already exists",
                            final_home.display()
                        );
                    }

                    let staging_home = prodex_profile_export::profile_import_staging_home(
                        &paths.managed_profiles_root,
                        &exported.name,
                        &runtime_random_token("profile")?,
                    );
                    create_codex_home_if_missing(&staging_home)?;
                    prepare_managed_codex_home(paths, &staging_home)?;
                    if plan_inputs[*source_index].supports_codex_runtime {
                        write_secret_text_file(
                            &staging_home.join("auth.json"),
                            &exported.auth_json,
                        )?;
                    }
                    write_exported_secret_files(&staging_home, exported)?;

                    staged_profiles.push(StagedImportedProfile {
                        name: exported.name.clone(),
                        email: plan_inputs[*source_index].identity.email.clone(),
                        staging_home,
                        final_home,
                        provider: exported.provider.clone(),
                    });
                }
                ProfileImportPlanAction::RewriteStagedAuth {
                    source_index,
                    staged_index,
                } => {
                    let exported = payload.profiles.get(*source_index).with_context(|| {
                        format!("import plan source index {} is missing", source_index)
                    })?;
                    let staged = staged_profiles.get_mut(*staged_index).with_context(|| {
                        format!(
                            "staged import profile index {} is missing for '{}'",
                            staged_index, exported.name
                        )
                    })?;
                    write_secret_text_file(
                        &staged.staging_home.join("auth.json"),
                        &exported.auth_json,
                    )?;
                    staged.email = plan_inputs[*source_index].identity.email.clone();
                }
            }
        }
        Ok(())
    })();

    if let Err(err) = result {
        for staged in &staged_profiles {
            let _ = fs::remove_dir_all(&staged.staging_home);
        }
        return Err(err);
    }

    Ok(PreparedImportedProfiles {
        staged_profiles,
        auth_updates,
        existing_profile_updates,
        resolved_profile_names: plan.resolved_profile_names,
    })
}

fn validate_exported_secret_files(exported: &ExportedProfile) -> Result<()> {
    let required_files = required_exported_secret_file_names(&exported.provider);
    let allowed_files = allowed_exported_secret_file_names(&exported.provider);
    let mut seen_paths = BTreeSet::new();

    for secret_file in &exported.secret_files {
        validate_exported_secret_file_path(&secret_file.path, &exported.name)?;
        if !seen_paths.insert(secret_file.path.clone()) {
            bail!(
                "profile export bundle contains duplicate secret file '{}' for profile '{}'",
                secret_file.path,
                exported.name
            );
        }
        if !allowed_files.contains(&secret_file.path.as_str()) {
            bail!(
                "profile export bundle contains unexpected secret file '{}' for profile '{}'",
                secret_file.path,
                exported.name
            );
        }
        validate_exported_secret_file_content(exported, &secret_file.path, &secret_file.text)?;
    }

    for required_file in required_files {
        if !seen_paths.contains(*required_file) {
            bail!(
                "profile export bundle is missing secret file '{}' for profile '{}'",
                required_file,
                exported.name
            );
        }
    }

    Ok(())
}

fn validate_exported_secret_file_path(path: &str, profile_name: &str) -> Result<()> {
    if path.trim().is_empty()
        || Path::new(path).is_absolute()
        || path.contains('/')
        || path.contains('\\')
        || matches!(path, "." | "..")
    {
        bail!(
            "profile export bundle contains unsafe secret file path '{}' for profile '{}'",
            path,
            profile_name
        );
    }
    Ok(())
}

fn validate_exported_secret_file_content(
    exported: &ExportedProfile,
    path: &str,
    text: &str,
) -> Result<()> {
    match &exported.provider {
        ProfileProvider::Gemini { .. } => {
            let _: GeminiOAuthSecret = serde_json::from_str(text).with_context(|| {
                format!(
                    "failed to parse exported secret file '{}' for profile '{}'",
                    path, exported.name
                )
            })?;
        }
        ProfileProvider::Anthropic { .. } => {
            parse_claude_oauth_secret_text(text).with_context(|| {
                format!(
                    "failed to parse exported secret file '{}' for profile '{}'",
                    path, exported.name
                )
            })?;
        }
        ProfileProvider::Kiro { .. } => {
            if path == KIRO_CREDENTIALS_FILE {
                parse_kiro_auth_secret_text(text).with_context(|| {
                    format!(
                        "failed to parse exported secret file '{}' for profile '{}'",
                        path, exported.name
                    )
                })?;
            } else if path == KIRO_MODEL_CATALOG_FILE {
                parse_kiro_model_catalog_text(text).with_context(|| {
                    format!(
                        "failed to parse exported secret file '{}' for profile '{}'",
                        path, exported.name
                    )
                })?;
            }
        }
        ProfileProvider::Openai | ProfileProvider::Copilot { .. } | ProfileProvider::Agy { .. } => {
        }
    }
    Ok(())
}

fn required_exported_secret_file_names(provider: &ProfileProvider) -> &'static [&'static str] {
    match provider {
        ProfileProvider::Gemini { .. } => &[GEMINI_OAUTH_SECRET_FILE],
        ProfileProvider::Anthropic { .. } => &[CLAUDE_CREDENTIALS_FILE],
        ProfileProvider::Kiro { .. } => &[KIRO_CREDENTIALS_FILE],
        ProfileProvider::Openai | ProfileProvider::Copilot { .. } | ProfileProvider::Agy { .. } => {
            &[]
        }
    }
}

fn allowed_exported_secret_file_names(provider: &ProfileProvider) -> &'static [&'static str] {
    match provider {
        ProfileProvider::Gemini { .. } => &[GEMINI_OAUTH_SECRET_FILE],
        ProfileProvider::Anthropic { .. } => &[CLAUDE_CREDENTIALS_FILE],
        ProfileProvider::Kiro { .. } => &[KIRO_CREDENTIALS_FILE, KIRO_MODEL_CATALOG_FILE],
        ProfileProvider::Openai | ProfileProvider::Copilot { .. } | ProfileProvider::Agy { .. } => {
            &[]
        }
    }
}

fn write_exported_secret_files(codex_home: &Path, exported: &ExportedProfile) -> Result<()> {
    for secret_file in &exported.secret_files {
        validate_exported_secret_file_path(&secret_file.path, &exported.name)?;
        write_secret_text_file(&codex_home.join(&secret_file.path), &secret_file.text)?;
    }
    Ok(())
}

fn read_optional_secret_text_file(path: &Path) -> Result<Option<String>> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .read_text(&secret_store::SecretLocation::file(path))
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to read {}", path.display()))
}

fn restore_optional_secret_text_file(path: &Path, previous_text: Option<&str>) -> Result<()> {
    if let Some(previous_text) = previous_text {
        return write_secret_text_file(path, previous_text);
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

fn write_imported_auth_update_journal(
    paths: &AppPaths,
    rollback: &ImportedExistingProfileAuthUpdate,
) -> Result<PathBuf> {
    let journal_path = prodex_profile_export::unique_profile_import_auth_update_journal_path(
        &paths.root,
        &rollback.profile_name,
        &runtime_random_token("auth")?,
    )?;
    let mut journal = ImportedExistingProfileAuthUpdateJournal::new(
        rollback.profile_name.clone(),
        rollback.codex_home.display().to_string(),
        rollback.previous_email.clone(),
        rollback.previous_auth_json.clone(),
        Local::now().to_rfc3339(),
    );
    journal.restore_auth_json = rollback.restore_auth_json;
    journal.previous_provider_json = rollback.previous_provider_json.clone();
    journal.previous_secret_files = rollback.previous_secret_files.clone();
    prodex_profile_export::write_profile_import_auth_update_journal(&journal_path, &journal)?;
    Ok(journal_path)
}
