use super::super::kiro::{
    KIRO_CREDENTIALS_FILE, KIRO_MODEL_CATALOG_FILE, prepare_kiro_cli_data_dir,
};
use super::super::manage::print_profile_panel;
use super::passwords::{resolve_export_password, resolve_export_password_mode};
use super::*;
use crate::secret_store_support::secret_file_read_error;
use zeroize::Zeroizing;

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
    let encoded = Zeroizing::new(prodex_profile_export::serialize_profile_export_payload(
        &payload,
        password.as_ref().map(|password| password.as_str()),
    )?);
    let output_path = args
        .output
        .map(absolutize)
        .transpose()?
        .unwrap_or_else(default_profile_export_path);
    prodex_profile_export::write_profile_export_bundle(&output_path, &encoded)?;
    audit_log_event(
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
    )?;

    let fields = prodex_profile_export::profile_export_summary_fields(
        prodex_profile_export::ProfileExportSummary {
            profile_count: profile_names.len(),
            path: output_path.display().to_string(),
            encrypted: password.is_some(),
            active_profile: payload.active_profile.clone(),
        },
    );
    print_profile_panel("Profile Export", &fields)?;
    Ok(())
}

pub(in crate::profile_commands) fn build_profile_export_payload(
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
            ProfileProvider::Gemini { .. }
            | ProfileProvider::Anthropic { .. }
            | ProfileProvider::Copilot { .. }
            | ProfileProvider::Kiro { .. }
            | ProfileProvider::Agy { .. } => String::new(),
        };
        let secret_files = exported_provider_secret_files(profile)?;
        profiles.push(ExportedProfile {
            name: name.clone(),
            email: profile.email.clone(),
            source_managed: profile.managed,
            provider: profile.provider.clone(),
            auth_json,
            secret_files,
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

fn exported_provider_secret_files(
    profile: &ProfileEntry,
) -> Result<Vec<prodex_profile_export::ExportedSecretFile>> {
    match &profile.provider {
        ProfileProvider::Openai | ProfileProvider::Copilot { .. } | ProfileProvider::Agy { .. } => {
            Ok(Vec::new())
        }
        ProfileProvider::Kiro { .. } => {
            prepare_kiro_cli_data_dir(&profile.codex_home)?;
            let mut files = vec![read_exported_secret_file(
                &profile.codex_home,
                KIRO_CREDENTIALS_FILE,
            )?];
            let model_catalog_path = profile.codex_home.join(KIRO_MODEL_CATALOG_FILE);
            if model_catalog_path.is_file() {
                files.push(read_exported_secret_file(
                    &profile.codex_home,
                    KIRO_MODEL_CATALOG_FILE,
                )?);
            }
            Ok(files)
        }
        ProfileProvider::Gemini { .. } => {
            let secret_file =
                read_exported_secret_file(&profile.codex_home, GEMINI_OAUTH_SECRET_FILE)?;
            let _: GeminiOAuthSecret =
                serde_json::from_str(&secret_file.text).with_context(|| {
                    format!(
                        "failed to parse {}",
                        profile.codex_home.join(GEMINI_OAUTH_SECRET_FILE).display()
                    )
                })?;
            Ok(vec![secret_file])
        }
        ProfileProvider::Anthropic { .. } => {
            read_claude_oauth_secret(&profile.codex_home)?;
            Ok(vec![read_exported_secret_file(
                &profile.codex_home,
                CLAUDE_CREDENTIALS_FILE,
            )?])
        }
    }
}

fn read_exported_secret_file(
    codex_home: &Path,
    file_name: &str,
) -> Result<prodex_profile_export::ExportedSecretFile> {
    let path = codex_home.join(file_name);
    let text = secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .read_text(&secret_store::SecretLocation::file(&path))
        .map_err(secret_file_read_error)?
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(prodex_profile_export::ExportedSecretFile {
        path: file_name.to_string(),
        text,
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
