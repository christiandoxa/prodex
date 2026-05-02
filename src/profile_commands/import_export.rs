use std::fs::OpenOptions;
use std::io::IsTerminal;
use std::io::Write as _;

use prodex_profile_export::{
    ImportedExistingProfileAuthUpdateJournal, ProfileImportIdentity, ProfileImportPlanAction,
    ProfileImportPlanProfile,
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
    let encoded = serialize_profile_export_payload(&payload, password.as_deref())?;
    let output_path = args
        .output
        .map(absolutize)
        .transpose()?
        .unwrap_or_else(default_profile_export_path);
    write_profile_export_bundle(&output_path, &encoded)?;
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
    cleanup_imported_auth_update_journals(&commit);
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
    Ok(imported_auth_update_journal_paths(paths)?.len())
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
        active_profile: state
            .active_profile
            .clone()
            .filter(|active| profile_names.iter().any(|name| name == active)),
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

pub(super) fn serialize_profile_export_payload(
    payload: &ProfileExportPayload,
    password: Option<&str>,
) -> Result<Vec<u8>> {
    prodex_profile_export::serialize_profile_export_payload(payload, password)
}

#[allow(dead_code)]
pub(super) fn derive_profile_export_key(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    prodex_profile_export::derive_profile_export_key(password, salt, iterations)
}

fn read_profile_export_payload(path: &Path) -> Result<(ProfileExportPayload, bool)> {
    let content = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let envelope: ProfileExportEnvelope = serde_json::from_slice(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let encrypted = matches!(envelope, ProfileExportEnvelope::Encrypted { .. });
    let payload = decode_profile_export_envelope(envelope)?;
    Ok((payload, encrypted))
}

pub(super) fn decode_profile_export_envelope(
    envelope: ProfileExportEnvelope,
) -> Result<ProfileExportPayload> {
    prodex_profile_export::decode_profile_export_envelope(envelope, resolve_import_password)
}

#[allow(dead_code)]
pub(super) fn validate_profile_export_header(format: &str, version: u32) -> Result<()> {
    prodex_profile_export::validate_profile_export_header(format, version)
}

pub(super) fn write_profile_export_bundle(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let temp_path = unique_state_temp_file_path(path);
    write_profile_export_temp_file(&temp_path, content)?;
    fs::rename(&temp_path, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, permissions)
            .with_context(|| format!("failed to secure {}", path.display()))?;
    }
    Ok(())
}

fn write_profile_export_temp_file(path: &Path, content: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(content)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
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
        transaction.rollback_partial(state);
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
    prepared_updates: &[PreparedImportedProfileAuthUpdate],
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
    remove_committed_import_homes(&commit.committed_homes);
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
    let journal_root = imported_auth_update_journal_root(paths);
    let journal_paths = imported_auth_update_journal_paths(paths)?;

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

fn imported_auth_update_journal_paths(paths: &AppPaths) -> Result<Vec<PathBuf>> {
    let journal_root = imported_auth_update_journal_root(paths);
    let entries = match fs::read_dir(&journal_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", journal_root.display()));
        }
    };
    let mut journal_paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .with_context(|| format!("failed to read entry in {}", journal_root.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    journal_paths.retain(|path| path.is_file());
    journal_paths.sort();
    Ok(journal_paths)
}

pub(super) fn cleanup_imported_auth_update_journals(commit: &ImportedProfilesCommit) {
    for update in &commit.auth_updates {
        let Some(journal_path) = update.journal_path.as_deref() else {
            continue;
        };
        let _ = fs::remove_file(journal_path);
        if let Some(parent) = journal_path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

pub(super) fn remove_committed_import_homes(committed_homes: &[PathBuf]) {
    for home in committed_homes.iter().rev() {
        let _ = fs::remove_dir_all(home);
    }
}

struct ExportedProfileImportPlanInput<'a> {
    exported: &'a ExportedProfile,
    identity: ProfileImportIdentity,
    supports_codex_runtime: bool,
}

impl ProfileImportPlanProfile for ExportedProfileImportPlanInput<'_> {
    fn profile_name(&self) -> &str {
        &self.exported.name
    }

    fn supports_codex_runtime(&self) -> bool {
        self.supports_codex_runtime
    }

    fn import_identity(&self) -> ProfileImportIdentity {
        self.identity.clone()
    }
}

pub(super) fn stage_imported_profiles(
    paths: &AppPaths,
    state: &mut AppState,
    payload: &ProfileExportPayload,
) -> Result<PreparedImportedProfiles> {
    if payload.profiles.is_empty() {
        bail!("profile export bundle does not contain any profiles");
    }

    ensure_managed_profiles_root(paths)?;

    let mut seen_names = BTreeSet::new();
    let mut plan_inputs = Vec::with_capacity(payload.profiles.len());
    for exported in &payload.profiles {
        validate_profile_name(&exported.name)?;
        if !seen_names.insert(exported.name.clone()) {
            bail!(
                "profile export bundle contains duplicate profile '{}'",
                exported.name
            );
        }

        let supports_codex_runtime = exported.provider.supports_codex_runtime();
        if supports_codex_runtime {
            let _: StoredAuth = serde_json::from_str(&exported.auth_json).with_context(|| {
                format!(
                    "failed to parse exported auth.json for profile '{}'",
                    exported.name
                )
            })?;
        }
        let resolved_identity = resolved_exported_profile_identity(exported);
        plan_inputs.push(ExportedProfileImportPlanInput {
            exported,
            identity: ProfileImportIdentity {
                email: resolved_identity.email,
                account_id: resolved_identity.account_id,
            },
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
                    queue_existing_profile_auth_update(
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

                    let staging_home = unique_import_staging_home(paths, &exported.name);
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

fn unique_import_staging_home(paths: &AppPaths, profile_name: &str) -> PathBuf {
    paths.managed_profiles_root.join(format!(
        ".import-{}-{}",
        profile_name,
        runtime_random_token("profile")
    ))
}

fn write_imported_auth_update_journal(
    paths: &AppPaths,
    profile_name: &str,
    codex_home: &Path,
    previous_email: Option<String>,
    previous_auth_json: Option<String>,
) -> Result<PathBuf> {
    let journal_path = unique_imported_auth_update_journal_path(paths, profile_name)?;
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

fn unique_imported_auth_update_journal_path(
    paths: &AppPaths,
    profile_name: &str,
) -> Result<PathBuf> {
    let journal_root = ensure_imported_auth_update_journal_root(paths)?;
    Ok(journal_root.join(format!(
        "{}-{}.json",
        profile_name,
        runtime_random_token("auth")
    )))
}

fn ensure_imported_auth_update_journal_root(paths: &AppPaths) -> Result<PathBuf> {
    let journal_root = imported_auth_update_journal_root(paths);
    fs::create_dir_all(&journal_root)
        .with_context(|| format!("failed to create {}", journal_root.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o700);
        fs::set_permissions(&journal_root, permissions)
            .with_context(|| format!("failed to secure {}", journal_root.display()))?;
    }
    Ok(journal_root)
}

pub(super) fn imported_auth_update_journal_root(paths: &AppPaths) -> PathBuf {
    paths
        .root
        .join(prodex_profile_export::IMPORT_AUTH_UPDATE_JOURNAL_DIR)
}

pub(super) fn write_secret_text_file(path: &Path, content: &str) -> Result<()> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(path), content)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", path.display()))
}
