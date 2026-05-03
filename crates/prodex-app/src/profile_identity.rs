pub(crate) use super::shared_codex_fs::{persist_login_home, remove_dir_if_exists};
use super::*;

pub(crate) use prodex_profile_identity::{ProfileIdentity, parse_identity_from_auth_json};
#[cfg(test)]
pub(crate) use prodex_profile_identity::{parse_email_from_id_token, profile_name_from_email};

#[derive(Debug)]
struct ProfileIdentityLookupJob {
    name: String,
    codex_home: PathBuf,
    cached_email: Option<String>,
}

pub(crate) fn fetch_profile_email(codex_home: &Path) -> Result<String> {
    let auth_email_error = match read_profile_identity_from_auth(codex_home) {
        Ok(identity) if identity.email.is_some() => {
            return Ok(identity.email.unwrap_or_default());
        }
        Ok(_) => None,
        Err(err) => Some(err),
    };

    if let Some(model_provider) = codex_non_openai_model_provider(codex_home, None) {
        if let Some(auth_error) = auth_email_error {
            bail!(
                "failed to read account email from stored auth secret ({auth_error:#}); quota email fallback is unavailable for model_provider '{}'",
                model_provider.provider_id,
            );
        }
        bail!(
            "stored auth secret did not contain an account email and quota email fallback is unavailable for model_provider '{}'",
            model_provider.provider_id,
        );
    }

    match fetch_profile_email_from_usage(codex_home) {
        Ok(email) => Ok(email),
        Err(usage_error) => {
            if let Some(auth_error) = auth_email_error {
                bail!(
                    "failed to read account email from stored auth secret ({auth_error:#}) and quota endpoint ({usage_error:#})"
                );
            }
            Err(usage_error)
        }
    }
}

pub(crate) fn fetch_profile_identity(codex_home: &Path) -> Result<ProfileIdentity> {
    let (mut identity, auth_identity_error) = match read_profile_identity_from_auth(codex_home) {
        Ok(identity) => (identity, None),
        Err(err) => (ProfileIdentity::default(), Some(err)),
    };

    if identity.has_email() && identity.has_account_id() {
        return Ok(identity);
    }

    if let Some(model_provider) = codex_non_openai_model_provider(codex_home, None) {
        if let Some(auth_error) = auth_identity_error {
            bail!(
                "failed to read account identity from stored auth secret ({auth_error:#}); quota identity fallback is unavailable for model_provider '{}'",
                model_provider.provider_id,
            );
        }
        return Ok(identity);
    }

    if !identity.has_email() {
        match fetch_profile_email_from_usage(codex_home) {
            Ok(email) => identity.email = Some(email),
            Err(usage_error) => {
                if let Some(auth_error) = auth_identity_error {
                    bail!(
                        "failed to read account identity from stored auth secret ({auth_error:#}) and quota endpoint ({usage_error:#})"
                    );
                }
            }
        }
    }

    Ok(identity)
}

pub(crate) fn read_profile_account_id_from_auth(codex_home: &Path) -> Result<Option<String>> {
    Ok(read_profile_identity_from_auth(codex_home)?.account_id)
}

fn read_profile_identity_from_auth(codex_home: &Path) -> Result<ProfileIdentity> {
    let auth_location = secret_store::auth_json_path(codex_home);

    let Some(content) = read_auth_json_text(codex_home)
        .with_context(|| format!("failed to read {}", auth_location.display()))?
    else {
        return Ok(ProfileIdentity::default());
    };
    parse_identity_from_auth_json(&content).with_context(|| {
        format!(
            "failed to parse account identity from {}",
            auth_location.display()
        )
    })
}

fn fetch_profile_email_from_usage(codex_home: &Path) -> Result<String> {
    let usage = fetch_usage(codex_home, None)?;
    let email = usage
        .email
        .as_deref()
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .context("quota endpoint did not return an email")?;
    Ok(email.to_string())
}

pub(crate) fn find_profile_by_identity(
    state: &mut AppState,
    target: &ProfileIdentity,
) -> Result<Option<String>> {
    let discovered = collect_profile_identities(state);
    Ok(prodex_profile_identity::find_matching_profile_identity(
        &discovered,
        target,
    ))
}

fn collect_profile_identities(state: &mut AppState) -> Vec<(String, ProfileIdentity)> {
    let jobs = state
        .profiles
        .iter()
        .filter_map(|(name, profile)| {
            if !profile.provider.supports_codex_runtime() {
                return None;
            }
            Some(ProfileIdentityLookupJob {
                name: name.clone(),
                codex_home: profile.codex_home.clone(),
                cached_email: profile.email.clone(),
            })
        })
        .collect();

    let discovered = map_parallel(jobs, |job| {
        let mut identity = fetch_profile_identity(&job.codex_home).unwrap_or_default();
        if identity.email.is_none() {
            identity.email = job.cached_email;
        }
        (job.name, identity)
    });

    for (name, identity) in &discovered {
        if let Some(email) = identity.email.as_ref()
            && let Some(profile) = state.profiles.get_mut(name)
        {
            profile.email = Some(email.clone());
        }
    }

    discovered
}

#[allow(dead_code)]
pub(crate) fn find_profile_by_email(state: &mut AppState, email: &str) -> Result<Option<String>> {
    find_profile_by_identity(
        state,
        &ProfileIdentity {
            email: Some(email.to_string()),
            account_id: None,
        },
    )
}

pub(crate) fn unique_profile_name_for_email(
    paths: &AppPaths,
    state: &AppState,
    email: &str,
) -> String {
    prodex_profile_identity::unique_profile_name_for_email(email, |candidate| {
        reclaim_stale_managed_profile_path(paths, state, candidate);
        is_available_profile_name(paths, state, candidate)
    })
}

fn is_available_profile_name(paths: &AppPaths, state: &AppState, candidate: &str) -> bool {
    !state.profiles.contains_key(candidate) && !paths.managed_profiles_root.join(candidate).exists()
}

fn reclaim_stale_managed_profile_path(paths: &AppPaths, state: &AppState, candidate: &str) {
    if state.profiles.contains_key(candidate) {
        return;
    }
    let candidate_path = paths.managed_profiles_root.join(candidate);
    if candidate_path.exists() {
        let _ = remove_dir_if_exists(&candidate_path);
    }
}

#[cfg(test)]
#[path = "../tests/src/profile_identity.rs"]
mod tests;
