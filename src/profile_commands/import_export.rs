use std::io::IsTerminal;

use prodex_profile_export::{
    ImportedExistingProfileAuthUpdateJournal, ProfileImportAuthUpdatePlan, ProfileImportIdentity,
    ProfileImportPlanAction, ProfileImportPlanInput,
};

use super::*;

const PROFILE_EXPORT_PASSWORD_ENV: &str = "PRODEX_PROFILE_EXPORT_PASSWORD";
const PROFILE_IMPORT_PASSWORD_ENV: &str = "PRODEX_PROFILE_IMPORT_PASSWORD";

pub(crate) fn handle_export_profiles(args: ExportProfileArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let available_profile_names = state.profiles.keys().cloned().collect::<BTreeSet<_>>();
    let profile_names = prodex_profile_export::resolve_requested_profile_names(
        &available_profile_names,
        &args.profile,
    )?;
    let payload = build_profile_export_payload(&state, &profile_names)?;
    let password = match resolve_export_password_mode(&args)? {
        true => Some(resolve_export_password()?),
        false => None,
    };
    let encoded =
        prodex_profile_export::serialize_profile_export_payload(&payload, password.as_deref())?;
    let output_path = args
        .output
        .map(absolutize)
        .transpose()?
        .unwrap_or_else(default_profile_export_path);
    prodex_profile_export::write_profile_export_bundle(&output_path, &encoded)?;
    audit_log_event_best_effort(
        "profile",
        "export",
        "success",
        serde_json::json!({
            "profile_count": profile_names.len(),
            "profile_names": profile_names,
            "encrypted": password.is_some(),
            "output_path": output_path.display().to_string(),
            "active_profile": payload.active_profile.clone(),
        }),
    );

    let fields = prodex_profile_export::profile_export_summary_fields(
        prodex_profile_export::ProfileExportSummary {
            profile_count: profile_names.len(),
            path: output_path.display().to_string(),
            encrypted: password.is_some(),
            active_profile: payload.active_profile.clone(),
        },
    );
    print_panel("Profile Export", &fields);
    Ok(())
}

pub(crate) fn handle_import_profiles(args: ImportProfileArgs) -> Result<()> {
    if super::copilot::is_copilot_import_source(&args.path) {
        return handle_import_copilot_profile(&args);
    }
    if args.name.is_some() || args.activate {
        bail!(
            "--name and --activate are only supported for built-in import sources such as `copilot`"
        );
    }

    let bundle_path = absolutize(args.path)?;
    let (payload, encrypted) = read_profile_export_payload(&bundle_path)?;
    let source_active_profile = payload.active_profile.clone();

    let paths = AppPaths::discover()?;
    let mut state = AppState::load(&paths)?;
    let recovered_auth_updates = recover_imported_auth_update_journals(&paths, &mut state)?;
    if recovered_auth_updates > 0 {
        state
            .save(&paths)
            .context("failed to save recovered import auth rollback state")?;
    }
    let commit = import_profile_export_payload(&paths, &mut state, &payload)?;
    if let Err(err) = state.save(&paths) {
        rollback_imported_profiles(&mut state, &commit);
        return Err(err);
    }
    prodex_profile_export::cleanup_imported_auth_update_journals(&commit);
    audit_log_event_best_effort(
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
    );

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
    print_panel("Profile Import", &fields);
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

pub(super) fn build_profile_export_payload(
    state: &AppState,
    profile_names: &[String],
) -> Result<ProfileExportPayload> {
    let mut profiles = Vec::with_capacity(profile_names.len());
    for name in profile_names {
        let profile = state
            .profiles
            .get(name)
            .with_context(|| format!("profile '{}' is missing", name))?;
        let auth_json = match &profile.provider {
            ProfileProvider::Openai => {
                let auth_path = secret_store::auth_json_path(&profile.codex_home);
                let auth_json = read_auth_json_text(&profile.codex_home)
                    .with_context(|| format!("failed to read {}", auth_path.display()))?
                    .with_context(|| format!("failed to read {}", auth_path.display()))?;
                let _: StoredAuth = serde_json::from_str(&auth_json)
                    .with_context(|| format!("failed to parse {}", auth_path.display()))?;
                auth_json
            }
            ProfileProvider::Copilot { .. } => String::new(),
        };
        profiles.push(ExportedProfile {
            name: name.clone(),
            email: profile.email.clone(),
            source_managed: profile.managed,
            provider: profile.provider.clone(),
            auth_json,
        });
    }

    Ok(ProfileExportPayload {
        exported_at: Local::now().to_rfc3339(),
        source_prodex_version: env!("CARGO_PKG_VERSION").to_string(),
        active_profile: prodex_profile_export::resolve_profile_export_active_profile(
            state.active_profile.as_deref(),
            profile_names.iter().map(String::as_str),
        ),
        profiles,
    })
}

fn default_profile_export_path() -> PathBuf {
    let file_name = format!(
        "prodex-profiles-{}.json",
        Local::now().format("%Y%m%d-%H%M%S")
    );
    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(file_name)
}

fn resolve_export_password_mode(args: &ExportProfileArgs) -> Result<bool> {
    if args.password_protect {
        return Ok(true);
    }
    if args.no_password {
        return Ok(false);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Ok(false);
    }
    prompt_yes_no("Password-protect export file? [y/N]: ", false)
}

fn resolve_export_password() -> Result<String> {
    if let Ok(password) = env::var(PROFILE_EXPORT_PASSWORD_ENV)
        && !password.trim().is_empty()
    {
        return Ok(password);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "password protection requested but no interactive terminal is available; set {}",
            PROFILE_EXPORT_PASSWORD_ENV
        );
    }

    let password = rpassword::prompt_password("Export password: ")
        .context("failed to read export password")?;
    if password.is_empty() {
        bail!("export password cannot be empty");
    }
    let confirmation = rpassword::prompt_password("Confirm export password: ")
        .context("failed to read export password confirmation")?;
    if password != confirmation {
        bail!("export passwords did not match");
    }
    Ok(password)
}

fn resolve_import_password() -> Result<String> {
    if let Ok(password) = env::var(PROFILE_IMPORT_PASSWORD_ENV)
        && !password.trim().is_empty()
    {
        return Ok(password);
    }
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!(
            "profile export bundle is password-protected; set {} or rerun in a terminal",
            PROFILE_IMPORT_PASSWORD_ENV
        );
    }

    let password = rpassword::prompt_password("Export password: ")
        .context("failed to read import password")?;
    if password.is_empty() {
        bail!("import password cannot be empty");
    }
    Ok(password)
}

fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    let mut input = String::new();
    loop {
        print_stderr_prompt(prompt)?;
        input.clear();
        io::stdin()
            .read_line(&mut input)
            .context("failed to read prompt response")?;
        match input.trim().to_ascii_lowercase().as_str() {
            "" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {
                print_stderr_line("Please answer yes or no.");
            }
        }
    }
}

fn read_profile_export_payload(path: &Path) -> Result<(ProfileExportPayload, bool)> {
    let (envelope, encrypted) = prodex_profile_export::read_profile_export_envelope(path)?;
    let payload =
        prodex_profile_export::decode_profile_export_envelope(envelope, resolve_import_password)?;
    Ok((payload, encrypted))
}

pub(super) fn import_profile_export_payload(
    paths: &AppPaths,
    state: &mut AppState,
    payload: &ProfileExportPayload,
) -> Result<ImportedProfilesCommit> {
    let prepared = stage_imported_profiles(paths, state, payload)?;
    let mut transaction = ImportedProfilesTransaction::new(
        state.active_profile.clone(),
        prepared.staged_profiles.len(),
        prepared.auth_updates.len(),
    );

    if let Err(err) = apply_imported_profiles(paths, state, payload, &prepared, &mut transaction) {
        rollback_partial_imported_profiles(state, &transaction);
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
        let journal_path = write_imported_auth_update_journal(
            paths,
            &update.target_profile_name,
            &previous.codex_home,
            previous_email.clone(),
            previous_auth_json.clone(),
        )?;
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
                rollback_imported_auth_updates(
                    state,
                    &[ImportedExistingProfileAuthUpdate {
                        profile_name: update.target_profile_name.clone(),
                        codex_home: previous.codex_home,
                        previous_auth_json,
                        previous_email,
                        journal_path: Some(journal_path),
                    }],
                );
                return Err(err);
            }
        };
        transaction.record_existing_auth_update(ImportedExistingProfileAuthUpdate {
            profile_name: updated.profile_name,
            codex_home: updated.codex_home,
            previous_auth_json,
            previous_email,
            journal_path: Some(journal_path),
        });
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

fn rollback_imported_profiles(state: &mut AppState, commit: &ImportedProfilesCommit) {
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
    rollback_imported_auth_updates(state, &commit.auth_updates);
    state.active_profile = commit.previous_active_profile.clone();
    prodex_profile_export::remove_committed_import_homes(&commit.committed_homes);
}

fn rollback_partial_imported_profiles(
    state: &mut AppState,
    transaction: &ImportedProfilesTransaction,
) {
    for name in &transaction.imported_names {
        state.profiles.remove(name);
    }
    rollback_imported_auth_updates(state, &transaction.auth_updates);
    state.active_profile = transaction.previous_active_profile.clone();
    prodex_profile_export::remove_committed_import_homes(&transaction.committed_homes);
}

pub(super) fn rollback_imported_auth_updates(
    state: &mut AppState,
    auth_updates: &[ImportedExistingProfileAuthUpdate],
) {
    for update in auth_updates.iter().rev() {
        if let Some(profile) = state.profiles.get_mut(&update.profile_name) {
            profile.email = update.previous_email.clone();
        }
        if let Some(previous_auth_json) = update.previous_auth_json.as_deref() {
            let _ = write_secret_text_file(
                &secret_store::auth_json_path(&update.codex_home),
                previous_auth_json,
            );
        } else {
            let _ = fs::remove_file(secret_store::auth_json_path(&update.codex_home));
        }
    }
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
        let journal_text = fs::read_to_string(&journal_path)
            .with_context(|| format!("failed to read {}", journal_path.display()))?;
        let journal: ImportedExistingProfileAuthUpdateJournal = serde_json::from_str(&journal_text)
            .with_context(|| format!("failed to parse {}", journal_path.display()))?;
        if let Err(err) =
            prodex_profile_export::validate_import_auth_update_journal_version(journal.version)
        {
            bail!("{err} in {}", journal_path.display());
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
            }],
        );
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
                    prodex_profile_export::queue_profile_import_auth_update(
                        &mut auth_updates,
                        target_profile_name,
                        plan_inputs[*source_index].identity.email.clone(),
                        exported.auth_json.clone(),
                    );
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
                        &runtime_random_token("profile"),
                    );
                    create_codex_home_if_missing(&staging_home)?;
                    prepare_managed_codex_home(paths, &staging_home)?;
                    if plan_inputs[*source_index].supports_codex_runtime {
                        write_secret_text_file(
                            &staging_home.join("auth.json"),
                            &exported.auth_json,
                        )?;
                    }

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
        resolved_profile_names: plan.resolved_profile_names,
    })
}

fn write_imported_auth_update_journal(
    paths: &AppPaths,
    profile_name: &str,
    codex_home: &Path,
    previous_email: Option<String>,
    previous_auth_json: Option<String>,
) -> Result<PathBuf> {
    let journal_path = prodex_profile_export::unique_profile_import_auth_update_journal_path(
        &paths.root,
        profile_name,
        &runtime_random_token("auth"),
    )?;
    let journal = ImportedExistingProfileAuthUpdateJournal::new(
        profile_name.to_string(),
        codex_home.display().to_string(),
        previous_email,
        previous_auth_json,
        Local::now().to_rfc3339(),
    );
    let json = serde_json::to_string_pretty(&journal)
        .context("failed to serialize auth update journal")?;
    write_secret_text_file(&journal_path, &json)?;
    Ok(journal_path)
}

pub(super) fn write_secret_text_file(path: &Path, content: &str) -> Result<()> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(path), content)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", path.display()))
}
