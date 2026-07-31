use prodex_profile_export::{
    ImportedExistingProfileFileRollback, ImportedExistingProfileFileUpdate,
    ProfileImportAuthUpdatePlan, ProfileImportIdentity, ProfileImportPlan, ProfileImportPlanAction,
    ProfileImportPlanInput,
};

use super::super::manage::print_profile_panel;
use super::lifecycle::{update_profile_lifecycle_plan, write_profile_lifecycle_plan};
use super::passwords::read_profile_export_payload;
use super::progress::print_profile_import_progress;
use super::secrets::{
    profile_import_exported_profile, read_optional_secret_text_file,
    restore_optional_secret_text_file, validate_exported_secret_file_path,
    validate_exported_secret_files, write_exported_secret_files,
    write_imported_auth_update_journal, write_secret_text_file,
};
use super::*;

pub(super) mod lifecycle_support;
use self::lifecycle_support::build_import_lifecycle_plan;

struct StagedImportCleanup<'a> {
    staged_profiles: &'a [StagedImportedProfile],
}

impl Drop for StagedImportCleanup<'_> {
    fn drop(&mut self) {
        for staged in self.staged_profiles {
            let _ = fs::remove_dir_all(&staged.staging_home);
        }
    }
}

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
    let _lock = super::acquire_profile_lifecycle_lock(&paths)?;
    let (mut state, _) = load_profile_state_with_profile_recovery_locked(&paths, true)?;
    print_profile_import_progress("Checking existing profiles...")?;
    print_profile_import_progress("Staging imported profiles...")?;
    let commit = import_profile_export_payload(&paths, &mut state, &payload)?;
    print_profile_import_progress("Saving imported profiles...")?;
    if let Err(err) = state.save(&paths) {
        rollback_imported_profiles(&mut state, &commit)
            .with_context(|| format!("failed to roll back profile import after: {err:#}"))?;
        state
            .save_with_removed_profiles(&paths, &commit.imported_names)
            .with_context(|| format!("failed to persist profile import rollback after: {err:#}"))?;
        return Err(err);
    }
    print_profile_import_progress("Profile import complete.")?;
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
    prodex_profile_export::cleanup_imported_auth_update_journals(&commit);
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
    let _lock = super::acquire_profile_lifecycle_lock(paths)?;
    Ok(prodex_profile_export::profile_import_auth_update_journal_paths(&paths.root)?.len())
}

pub(in crate::profile_commands) fn import_profile_export_payload(
    paths: &AppPaths,
    state: &mut AppState,
    payload: &ProfileExportPayload,
) -> Result<ImportedProfilesCommit> {
    let prepared = stage_imported_profiles(paths, state, payload)?;
    let _staging_cleanup = StagedImportCleanup {
        staged_profiles: &prepared.staged_profiles,
    };
    let mut lifecycle_plan = build_import_lifecycle_plan(state, payload, &prepared)?;
    let lifecycle_path = write_profile_lifecycle_plan(paths, "import", &lifecycle_plan)?;
    let mut transaction = ImportedProfilesTransaction::new(
        state.active_profile.clone(),
        prepared.staged_profiles.len(),
        prepared.auth_updates.len() + prepared.existing_profile_updates.len(),
    );
    transaction.set_lifecycle_journal_path(lifecycle_path.clone());

    if let Err(err) = apply_imported_profiles(paths, state, payload, &prepared, &mut transaction) {
        rollback_partial_imported_profiles(state, &transaction).with_context(|| {
            format!("failed to roll back partial profile import after: {err:#}")
        })?;
        cleanup_rolled_back_import_journals(Some(&lifecycle_path), &transaction.auth_updates);
        return Err(err);
    }

    lifecycle_plan.auth_journal_paths = transaction
        .auth_updates
        .iter()
        .filter_map(|update| update.journal_path.as_ref())
        .map(|path| path.display().to_string())
        .collect();
    if let Err(err) =
        update_profile_lifecycle_plan(paths, &lifecycle_path, "import", &lifecycle_plan)
    {
        rollback_partial_imported_profiles(state, &transaction).with_context(|| {
            format!("failed to roll back profile import after lifecycle update failed: {err:#}")
        })?;
        cleanup_rolled_back_import_journals(Some(&lifecycle_path), &transaction.auth_updates);
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
        rollback.journal_path = Some(write_imported_auth_update_journal(
            paths,
            &rollback,
            update.email.clone(),
            Some(update.auth_json.clone()),
            Some(serde_json::to_string(&previous.provider)?),
            Vec::new(),
            None,
        )?);
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
                if let Some(path) = rollback.journal_path.as_deref() {
                    prodex_profile_export::cleanup_profile_import_auth_update_journal(path);
                }
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
        let journal_path = write_imported_auth_update_journal(
            paths,
            &rollback,
            update.email.clone(),
            None,
            Some(serde_json::to_string(&update.provider)?),
            update
                .secret_files
                .iter()
                .map(|secret_file| ImportedExistingProfileFileUpdate {
                    path: secret_file.path.clone(),
                    text: Some(secret_file.text.clone()),
                })
                .collect(),
            None,
        )?;
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
            prodex_profile_export::cleanup_profile_import_auth_update_journal(journal_path);
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

fn cleanup_rolled_back_import_journals(
    lifecycle_path: Option<&Path>,
    auth_updates: &[ImportedExistingProfileAuthUpdate],
) {
    if let Some(path) = lifecycle_path {
        super::lifecycle::cleanup_profile_lifecycle_journal(path);
    }
    for update in auth_updates {
        if let Some(path) = update.journal_path.as_deref() {
            prodex_profile_export::cleanup_profile_import_auth_update_journal(path);
        }
    }
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

pub(super) fn stage_imported_profiles(
    paths: &AppPaths,
    state: &mut AppState,
    payload: &ProfileExportPayload,
) -> Result<PreparedImportedProfiles> {
    validate_stage_import_payload(paths, payload)?;
    let plan_inputs = build_profile_import_plan_inputs(payload)?;
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

    let (staged_profiles, auth_updates, existing_profile_updates) =
        stage_import_plan_actions(paths, state, payload, &plan, &plan_inputs)?;
    Ok(PreparedImportedProfiles {
        staged_profiles,
        auth_updates,
        existing_profile_updates,
        resolved_profile_names: plan.resolved_profile_names,
    })
}

fn validate_stage_import_payload(paths: &AppPaths, payload: &ProfileExportPayload) -> Result<()> {
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
    ensure_managed_profiles_root(paths)
}

fn build_profile_import_plan_inputs(
    payload: &ProfileExportPayload,
) -> Result<Vec<ProfileImportPlanInput>> {
    payload
        .profiles
        .iter()
        .map(|exported| {
            validate_exported_secret_files(exported)?;
            let supports_codex_runtime = exported.provider.supports_codex_runtime();
            if supports_codex_runtime {
                let _: StoredAuth =
                    serde_json::from_str(&exported.auth_json).with_context(|| {
                        format!(
                            "failed to parse exported auth.json for profile '{}'",
                            exported.name
                        )
                    })?;
            }
            let auth_identity =
                parse_identity_from_auth_json(&exported.auth_json).unwrap_or_default();
            let identity = prodex_profile_export::resolve_profile_import_identity(
                ProfileImportIdentity {
                    email: auth_identity.email,
                    account_id: auth_identity.account_id,
                },
                exported.email.as_deref(),
            );
            Ok(ProfileImportPlanInput {
                profile_name: exported.name.clone(),
                identity,
                supports_codex_runtime,
            })
        })
        .collect()
}

fn stage_import_plan_actions(
    paths: &AppPaths,
    state: &mut AppState,
    payload: &ProfileExportPayload,
    plan: &ProfileImportPlan,
    plan_inputs: &[ProfileImportPlanInput],
) -> Result<(
    Vec<StagedImportedProfile>,
    Vec<prodex_profile_export::ProfileImportAuthUpdatePlan>,
    Vec<PreparedExistingProfileUpdate>,
)> {
    let mut staged_profiles = Vec::with_capacity(payload.profiles.len());
    let mut auth_updates = Vec::new();
    let mut existing_profile_updates = Vec::new();
    let result = (|| -> Result<()> {
        for action in &plan.actions {
            stage_import_plan_action(
                action,
                StageImportPlanActionContext {
                    paths,
                    state,
                    payload,
                    plan_inputs,
                    staged_profiles: &mut staged_profiles,
                    auth_updates: &mut auth_updates,
                    existing_profile_updates: &mut existing_profile_updates,
                },
            )?;
        }
        Ok(())
    })();

    if let Err(err) = result {
        for staged in &staged_profiles {
            let _ = fs::remove_dir_all(&staged.staging_home);
        }
        return Err(err);
    }

    Ok((staged_profiles, auth_updates, existing_profile_updates))
}

struct StageImportPlanActionContext<'a> {
    paths: &'a AppPaths,
    state: &'a mut AppState,
    payload: &'a ProfileExportPayload,
    plan_inputs: &'a [ProfileImportPlanInput],
    staged_profiles: &'a mut Vec<StagedImportedProfile>,
    auth_updates: &'a mut Vec<prodex_profile_export::ProfileImportAuthUpdatePlan>,
    existing_profile_updates: &'a mut Vec<PreparedExistingProfileUpdate>,
}

fn stage_import_plan_action(
    action: &ProfileImportPlanAction,
    context: StageImportPlanActionContext<'_>,
) -> Result<()> {
    let StageImportPlanActionContext {
        paths,
        state,
        payload,
        plan_inputs,
        staged_profiles,
        auth_updates,
        existing_profile_updates,
    } = context;
    match action {
        ProfileImportPlanAction::UpdateExisting {
            source_index,
            target_profile_name,
        } => stage_existing_profile_update(
            state,
            payload,
            plan_inputs,
            *source_index,
            target_profile_name,
            auth_updates,
            existing_profile_updates,
        ),
        ProfileImportPlanAction::StageNew {
            source_index,
            staged_index,
        } => stage_new_profile(
            paths,
            state,
            payload,
            plan_inputs,
            *source_index,
            *staged_index,
            staged_profiles,
        ),
        ProfileImportPlanAction::RewriteStagedAuth {
            source_index,
            staged_index,
        } => rewrite_staged_profile_auth(
            payload,
            plan_inputs,
            *source_index,
            *staged_index,
            staged_profiles,
        ),
    }
}

fn stage_existing_profile_update(
    state: &AppState,
    payload: &ProfileExportPayload,
    plan_inputs: &[ProfileImportPlanInput],
    source_index: usize,
    target_profile_name: &str,
    auth_updates: &mut Vec<prodex_profile_export::ProfileImportAuthUpdatePlan>,
    existing_profile_updates: &mut Vec<PreparedExistingProfileUpdate>,
) -> Result<()> {
    let exported = profile_import_exported_profile(payload, source_index)?;
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
    if plan_inputs[source_index].supports_codex_runtime {
        prodex_profile_export::queue_profile_import_auth_update(
            auth_updates,
            target_profile_name,
            plan_inputs[source_index].identity.email.clone(),
            exported.auth_json.clone(),
        );
    } else {
        existing_profile_updates.push(PreparedExistingProfileUpdate {
            name: target_profile_name.to_string(),
            email: plan_inputs[source_index].identity.email.clone(),
            provider: exported.provider.clone(),
            secret_files: exported.secret_files.clone(),
        });
    }
    Ok(())
}

fn stage_new_profile(
    paths: &AppPaths,
    state: &AppState,
    payload: &ProfileExportPayload,
    plan_inputs: &[ProfileImportPlanInput],
    source_index: usize,
    staged_index: usize,
    staged_profiles: &mut Vec<StagedImportedProfile>,
) -> Result<()> {
    if staged_profiles.len() != staged_index {
        bail!(
            "staged import profile index {} is out of order",
            staged_index
        );
    }
    let exported = profile_import_exported_profile(payload, source_index)?;
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
    staged_profiles.push(StagedImportedProfile {
        name: exported.name.clone(),
        email: plan_inputs[source_index].identity.email.clone(),
        staging_home: staging_home.clone(),
        final_home: final_home.clone(),
        provider: exported.provider.clone(),
    });
    create_codex_home_if_missing(&staging_home)?;
    prepare_managed_codex_home(paths, &staging_home)?;
    if plan_inputs[source_index].supports_codex_runtime {
        write_secret_text_file(&staging_home.join("auth.json"), &exported.auth_json)?;
    }
    write_exported_secret_files(&staging_home, exported)?;
    Ok(())
}

fn rewrite_staged_profile_auth(
    payload: &ProfileExportPayload,
    plan_inputs: &[ProfileImportPlanInput],
    source_index: usize,
    staged_index: usize,
    staged_profiles: &mut [StagedImportedProfile],
) -> Result<()> {
    let exported = profile_import_exported_profile(payload, source_index)?;
    let staged = staged_profiles.get_mut(staged_index).with_context(|| {
        format!(
            "staged import profile index {} is missing for '{}'",
            staged_index, exported.name
        )
    })?;
    write_secret_text_file(&staged.staging_home.join("auth.json"), &exported.auth_json)?;
    staged.email = plan_inputs[source_index].identity.email.clone();
    Ok(())
}
