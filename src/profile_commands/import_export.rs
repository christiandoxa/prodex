use aes_gcm_siv::{
    Aes256GcmSiv, Nonce,
    aead::{Aead, KeyInit},
};
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use std::fs::OpenOptions;
use std::io::IsTerminal;
use std::io::Write as _;

use super::*;

const PROFILE_EXPORT_FORMAT: &str = "prodex_profile_export";
const PROFILE_EXPORT_VERSION: u32 = 1;
pub(super) const PROFILE_EXPORT_CIPHER: &str = "aes_256_gcm_siv";
pub(super) const PROFILE_EXPORT_KDF: &str = "pbkdf2_sha256";
const PROFILE_EXPORT_NONCE_BYTES: usize = 12;
const PROFILE_EXPORT_SALT_BYTES: usize = 16;
const PROFILE_EXPORT_KEY_BYTES: usize = 32;
const PROFILE_EXPORT_PBKDF2_ITERATIONS: u32 = if cfg!(test) { 1_000 } else { 600_000 };
const PROFILE_EXPORT_PASSWORD_ENV: &str = "PRODEX_PROFILE_EXPORT_PASSWORD";
const PROFILE_IMPORT_PASSWORD_ENV: &str = "PRODEX_PROFILE_IMPORT_PASSWORD";

pub(crate) fn handle_export_profiles(args: ExportProfileArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let profile_names = resolve_export_profile_names(&state, &args.profile)?;
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

    let mut fields = vec![
        (
            "Result".to_string(),
            format!("Exported {} profile(s).", profile_names.len()),
        ),
        ("Path".to_string(), output_path.display().to_string()),
        (
            "Encrypted".to_string(),
            if password.is_some() {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        ),
    ];
    if payload.active_profile.is_some() {
        fields.push((
            "Active".to_string(),
            payload.active_profile.unwrap_or_default(),
        ));
    }
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
    let commit = import_profile_export_payload(&paths, &mut state, &payload)?;
    if let Err(err) = state.save(&paths) {
        rollback_imported_profiles(&mut state, &commit);
        return Err(err);
    }
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

    let result_message = match (
        commit.imported_names.len(),
        commit.updated_existing_names.len(),
    ) {
        (0, updated) => format!("Updated {updated} existing profile(s)."),
        (imported, 0) => format!("Imported {imported} profile(s)."),
        (imported, updated) => {
            format!("Imported {imported} profile(s) and updated {updated} existing profile(s).")
        }
    };
    let mut fields = vec![
        ("Result".to_string(), result_message),
        ("Path".to_string(), bundle_path.display().to_string()),
        (
            "Encrypted".to_string(),
            if encrypted {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        ),
        (
            "Imported".to_string(),
            commit.imported_names.len().to_string(),
        ),
        (
            "Updated duplicates".to_string(),
            commit.updated_existing_names.len().to_string(),
        ),
    ];
    if let Some(active_profile) = source_active_profile {
        fields.push(("Source active".to_string(), active_profile));
    }
    if let Some(active_profile) = state.active_profile.clone() {
        fields.push(("Active".to_string(), active_profile));
    }
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

fn resolve_export_profile_names(state: &AppState, requested: &[String]) -> Result<Vec<String>> {
    if state.profiles.is_empty() {
        bail!("no profiles configured");
    }

    if requested.is_empty() {
        return Ok(state.profiles.keys().cloned().collect());
    }

    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for name in requested {
        if !seen.insert(name.clone()) {
            continue;
        }
        if !state.profiles.contains_key(name) {
            bail!("profile '{}' does not exist", name);
        }
        names.push(name.clone());
    }
    Ok(names)
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
    let envelope = match password {
        Some(password) => encrypt_profile_export_payload(payload, password)?,
        None => ProfileExportEnvelope::Plain {
            format: PROFILE_EXPORT_FORMAT.to_string(),
            version: PROFILE_EXPORT_VERSION,
            payload: payload.clone(),
        },
    };
    serde_json::to_vec_pretty(&envelope).context("failed to serialize profile export bundle")
}

fn encrypt_profile_export_payload(
    payload: &ProfileExportPayload,
    password: &str,
) -> Result<ProfileExportEnvelope> {
    let payload_json =
        serde_json::to_vec(payload).context("failed to serialize profile export payload")?;
    let mut salt = [0_u8; PROFILE_EXPORT_SALT_BYTES];
    getrandom::fill(&mut salt)
        .map_err(|err| anyhow::anyhow!("failed to generate export salt: {err}"))?;
    let mut nonce = [0_u8; PROFILE_EXPORT_NONCE_BYTES];
    getrandom::fill(&mut nonce)
        .map_err(|err| anyhow::anyhow!("failed to generate export nonce: {err}"))?;
    let key = derive_profile_export_key(password, &salt, PROFILE_EXPORT_PBKDF2_ITERATIONS);
    let cipher =
        Aes256GcmSiv::new_from_slice(&key).context("failed to initialize export cipher")?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), payload_json.as_ref())
        .map_err(|_| anyhow::anyhow!("failed to encrypt profile export payload"))?;

    Ok(ProfileExportEnvelope::Encrypted {
        format: PROFILE_EXPORT_FORMAT.to_string(),
        version: PROFILE_EXPORT_VERSION,
        cipher: PROFILE_EXPORT_CIPHER.to_string(),
        kdf: PROFILE_EXPORT_KDF.to_string(),
        iterations: PROFILE_EXPORT_PBKDF2_ITERATIONS,
        salt_base64: base64::engine::general_purpose::STANDARD.encode(salt),
        nonce_base64: base64::engine::general_purpose::STANDARD.encode(nonce),
        ciphertext_base64: base64::engine::general_purpose::STANDARD.encode(ciphertext),
    })
}

pub(super) fn derive_profile_export_key(
    password: &str,
    salt: &[u8],
    iterations: u32,
) -> [u8; PROFILE_EXPORT_KEY_BYTES] {
    let mut key = [0_u8; PROFILE_EXPORT_KEY_BYTES];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
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
    match envelope {
        ProfileExportEnvelope::Plain {
            format,
            version,
            payload,
        } => {
            validate_profile_export_header(&format, version)?;
            Ok(payload)
        }
        ProfileExportEnvelope::Encrypted {
            format,
            version,
            cipher,
            kdf,
            iterations,
            salt_base64,
            nonce_base64,
            ciphertext_base64,
        } => {
            validate_profile_export_header(&format, version)?;
            if cipher != PROFILE_EXPORT_CIPHER {
                bail!("unsupported profile export cipher '{}'", cipher);
            }
            if kdf != PROFILE_EXPORT_KDF {
                bail!("unsupported profile export KDF '{}'", kdf);
            }
            let password = resolve_import_password()?;
            let salt = base64::engine::general_purpose::STANDARD
                .decode(salt_base64)
                .context("failed to decode encrypted export salt")?;
            let nonce = base64::engine::general_purpose::STANDARD
                .decode(nonce_base64)
                .context("failed to decode encrypted export nonce")?;
            let ciphertext = base64::engine::general_purpose::STANDARD
                .decode(ciphertext_base64)
                .context("failed to decode encrypted export payload")?;
            if nonce.len() != PROFILE_EXPORT_NONCE_BYTES {
                bail!("invalid encrypted export nonce length");
            }
            let key = derive_profile_export_key(&password, &salt, iterations);
            let cipher =
                Aes256GcmSiv::new_from_slice(&key).context("failed to initialize import cipher")?;
            let plaintext = cipher
                .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
                .map_err(|_| anyhow::anyhow!("failed to decrypt profile export bundle"))?;
            serde_json::from_slice(&plaintext)
                .context("failed to parse decrypted profile export payload")
        }
    }
}

pub(super) fn validate_profile_export_header(format: &str, version: u32) -> Result<()> {
    if format != PROFILE_EXPORT_FORMAT {
        bail!("unsupported profile export format '{}'", format);
    }
    if version != PROFILE_EXPORT_VERSION {
        bail!(
            "unsupported profile export version {} (expected {})",
            version,
            PROFILE_EXPORT_VERSION
        );
    }
    Ok(())
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
        let updated = update_existing_profile_auth(
            paths,
            state,
            &update.target_profile_name,
            update.email.as_deref(),
            &update.auth_json,
            false,
        )?;
        transaction.record_existing_auth_update(ImportedExistingProfileAuthUpdate {
            profile_name: updated.profile_name,
            codex_home: updated.codex_home,
            previous_auth_json,
            previous_email: previous.email,
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
    if state.active_profile.is_none()
        && let Some(active_profile) = payload.active_profile.as_ref()
        && let Some(resolved_profile_name) = prepared.resolved_profile_names.get(active_profile)
    {
        state.active_profile = Some(resolved_profile_name.clone());
    }
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

pub(super) fn remove_committed_import_homes(committed_homes: &[PathBuf]) {
    for home in committed_homes.iter().rev() {
        let _ = fs::remove_dir_all(home);
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
    let mut staged_profiles = Vec::with_capacity(payload.profiles.len());
    let mut auth_updates = Vec::new();
    let mut resolved_profile_names = BTreeMap::new();
    let mut email_targets = BTreeMap::new();
    let result = (|| -> Result<()> {
        for exported in &payload.profiles {
            validate_profile_name(&exported.name)?;
            if !seen_names.insert(exported.name.clone()) {
                bail!(
                    "profile export bundle contains duplicate profile '{}'",
                    exported.name
                );
            }

            let provider = exported.provider.clone();
            if provider.supports_codex_runtime() {
                let _: StoredAuth =
                    serde_json::from_str(&exported.auth_json).with_context(|| {
                        format!(
                            "failed to parse exported auth.json for profile '{}'",
                            exported.name
                        )
                    })?;
            }
            let resolved_email = resolved_exported_profile_email(exported);

            if let Some(existing_profile) = state.profiles.get(&exported.name) {
                if !provider.supports_codex_runtime()
                    || !existing_profile.provider.supports_codex_runtime()
                {
                    bail!("profile '{}' already exists", exported.name);
                }

                queue_existing_profile_auth_update(
                    &mut auth_updates,
                    &exported.name,
                    resolved_email.clone(),
                    exported.auth_json.clone(),
                );
                resolved_profile_names.insert(exported.name.clone(), exported.name.clone());
                if let Some(email) = resolved_email.as_deref() {
                    email_targets.insert(
                        normalize_email(email),
                        ImportEmailTarget::Existing(exported.name.clone()),
                    );
                }
                continue;
            }

            if provider.supports_codex_runtime()
                && let Some(email) = resolved_email.as_deref()
            {
                let normalized_email = normalize_email(email);
                if let Some(target) = email_targets.get(&normalized_email) {
                    match target {
                        ImportEmailTarget::Existing(profile_name) => {
                            queue_existing_profile_auth_update(
                                &mut auth_updates,
                                profile_name,
                                resolved_email.clone(),
                                exported.auth_json.clone(),
                            );
                            resolved_profile_names
                                .insert(exported.name.clone(), profile_name.clone());
                            continue;
                        }
                        ImportEmailTarget::PendingNew(index) => {
                            let staged: &mut StagedImportedProfile =
                                staged_profiles.get_mut(*index).with_context(|| {
                                    format!(
                                        "staged import profile index {} is missing for '{}'",
                                        index, exported.name
                                    )
                                })?;
                            write_secret_text_file(
                                &staged.staging_home.join("auth.json"),
                                &exported.auth_json,
                            )?;
                            staged.email = resolved_email.clone();
                            resolved_profile_names
                                .insert(exported.name.clone(), staged.name.clone());
                            continue;
                        }
                    }
                }

                if let Some(existing_profile_name) = find_profile_by_email(state, email)? {
                    email_targets.insert(
                        normalized_email,
                        ImportEmailTarget::Existing(existing_profile_name.clone()),
                    );
                    queue_existing_profile_auth_update(
                        &mut auth_updates,
                        &existing_profile_name,
                        resolved_email.clone(),
                        exported.auth_json.clone(),
                    );
                    resolved_profile_names.insert(exported.name.clone(), existing_profile_name);
                    continue;
                }
            }

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
            if provider.supports_codex_runtime() {
                write_secret_text_file(&staging_home.join("auth.json"), &exported.auth_json)?;
            }

            let new_index = staged_profiles.len();
            staged_profiles.push(StagedImportedProfile {
                name: exported.name.clone(),
                email: resolved_email.clone(),
                staging_home,
                final_home,
                provider: provider.clone(),
            });
            resolved_profile_names.insert(exported.name.clone(), exported.name.clone());
            if provider.supports_codex_runtime()
                && let Some(email) = resolved_email
            {
                email_targets.insert(
                    normalize_email(&email),
                    ImportEmailTarget::PendingNew(new_index),
                );
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
        resolved_profile_names,
    })
}

fn unique_import_staging_home(paths: &AppPaths, profile_name: &str) -> PathBuf {
    paths.managed_profiles_root.join(format!(
        ".import-{}-{}",
        profile_name,
        runtime_random_token("profile")
    ))
}

pub(super) fn write_secret_text_file(path: &Path, content: &str) -> Result<()> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(path), content)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", path.display()))
}
