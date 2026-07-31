use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};

use crate::data_model::ImportIdentityTarget;
use crate::{
    ProfileImportAuthUpdatePlan, ProfileImportIdentity, ProfileImportPlan, ProfileImportPlanAction,
    ProfileImportPlanProfile,
};

pub fn plan_profile_import<P>(
    profiles: &[P],
    existing_profile_supports_codex_runtime: impl Fn(&str) -> Option<bool>,
    mut find_existing_profile_by_identity: impl FnMut(&ProfileImportIdentity) -> Result<Option<String>>,
) -> Result<ProfileImportPlan>
where
    P: ProfileImportPlanProfile,
{
    if profiles.is_empty() {
        bail!("profile export bundle does not contain any profiles");
    }

    let mut plan = ProfileImportPlanBuilder::new(profiles.len());

    for (source_index, profile) in profiles.iter().enumerate() {
        plan.add_profile(
            source_index,
            profile,
            &existing_profile_supports_codex_runtime,
            &mut find_existing_profile_by_identity,
        )?;
    }

    Ok(plan.finish())
}

struct ProfileImportPlanBuilder {
    seen_names: BTreeSet<String>,
    identity_targets: BTreeMap<String, ImportIdentityTarget>,
    actions: Vec<ProfileImportPlanAction>,
    resolved_profile_names: BTreeMap<String, String>,
    staged_profile_names: Vec<String>,
}

impl ProfileImportPlanBuilder {
    fn new(capacity: usize) -> Self {
        Self {
            seen_names: BTreeSet::new(),
            identity_targets: BTreeMap::new(),
            actions: Vec::with_capacity(capacity),
            resolved_profile_names: BTreeMap::new(),
            staged_profile_names: Vec::new(),
        }
    }

    fn add_profile<P, F, G>(
        &mut self,
        source_index: usize,
        profile: &P,
        existing_profile_supports_codex_runtime: &F,
        find_existing_profile_by_identity: &mut G,
    ) -> Result<()>
    where
        P: ProfileImportPlanProfile,
        F: Fn(&str) -> Option<bool>,
        G: FnMut(&ProfileImportIdentity) -> Result<Option<String>>,
    {
        let source_profile_name = profile.profile_name();
        if !self.seen_names.insert(source_profile_name.to_string()) {
            bail!(
                "profile export bundle contains duplicate profile '{}'",
                source_profile_name
            );
        }
        let provider_supports_codex_runtime = profile.supports_codex_runtime();
        let resolved_identity = profile.import_identity();
        let identity_key = resolved_identity.target_key();

        if let Some(existing_supports_codex_runtime) =
            existing_profile_supports_codex_runtime(source_profile_name)
        {
            if existing_supports_codex_runtime != provider_supports_codex_runtime {
                bail!(
                    "profile '{}' already exists with an incompatible provider",
                    source_profile_name
                );
            }
            self.record_update(
                source_index,
                source_profile_name,
                source_profile_name,
                identity_key,
            );
            return Ok(());
        }

        if provider_supports_codex_runtime
            && (self.resolve_known_identity(source_index, source_profile_name, &identity_key)?
                || self.resolve_external_identity(
                    source_index,
                    source_profile_name,
                    &identity_key,
                    &resolved_identity,
                    find_existing_profile_by_identity,
                )?)
        {
            return Ok(());
        }

        self.stage_new(
            source_index,
            source_profile_name,
            provider_supports_codex_runtime,
            identity_key,
        );
        Ok(())
    }

    fn resolve_known_identity(
        &mut self,
        source_index: usize,
        source_profile_name: &str,
        identity_key: &Option<String>,
    ) -> Result<bool> {
        let Some(identity_key) = identity_key.as_deref() else {
            return Ok(false);
        };
        let Some(target) = self.identity_targets.get(identity_key).cloned() else {
            return Ok(false);
        };
        match target {
            ImportIdentityTarget::Existing(profile_name) => {
                self.record_update(source_index, source_profile_name, &profile_name, None);
            }
            ImportIdentityTarget::PendingNew(staged_index) => {
                self.actions
                    .push(ProfileImportPlanAction::RewriteStagedAuth {
                        source_index,
                        staged_index,
                    });
                let target_profile_name = self
                    .staged_profile_names
                    .get(staged_index)
                    .cloned()
                    .with_context(|| {
                        format!(
                            "staged import profile index {} is missing for '{}'",
                            staged_index, source_profile_name
                        )
                    })?;
                self.resolved_profile_names
                    .insert(source_profile_name.to_string(), target_profile_name);
            }
        }
        Ok(true)
    }

    fn resolve_external_identity<G>(
        &mut self,
        source_index: usize,
        source_profile_name: &str,
        identity_key: &Option<String>,
        resolved_identity: &ProfileImportIdentity,
        find_existing_profile_by_identity: &mut G,
    ) -> Result<bool>
    where
        G: FnMut(&ProfileImportIdentity) -> Result<Option<String>>,
    {
        let Some(identity_key) = identity_key else {
            return Ok(false);
        };
        let Some(existing_profile_name) = find_existing_profile_by_identity(resolved_identity)?
        else {
            return Ok(false);
        };
        self.record_update(
            source_index,
            source_profile_name,
            &existing_profile_name,
            Some(identity_key.clone()),
        );
        Ok(true)
    }

    fn stage_new(
        &mut self,
        source_index: usize,
        source_profile_name: &str,
        provider_supports_codex_runtime: bool,
        identity_key: Option<String>,
    ) {
        let staged_index = self.staged_profile_names.len();
        self.staged_profile_names
            .push(source_profile_name.to_string());
        self.actions.push(ProfileImportPlanAction::StageNew {
            source_index,
            staged_index,
        });
        self.resolved_profile_names.insert(
            source_profile_name.to_string(),
            source_profile_name.to_string(),
        );
        if provider_supports_codex_runtime && let Some(identity_key) = identity_key {
            self.identity_targets
                .insert(identity_key, ImportIdentityTarget::PendingNew(staged_index));
        }
    }

    fn record_update(
        &mut self,
        source_index: usize,
        source_profile_name: &str,
        target_profile_name: &str,
        identity_key: Option<String>,
    ) {
        self.actions.push(ProfileImportPlanAction::UpdateExisting {
            source_index,
            target_profile_name: target_profile_name.to_string(),
        });
        self.resolved_profile_names.insert(
            source_profile_name.to_string(),
            target_profile_name.to_string(),
        );
        if let Some(identity_key) = identity_key {
            self.identity_targets.insert(
                identity_key,
                ImportIdentityTarget::Existing(target_profile_name.to_string()),
            );
        }
    }

    fn finish(self) -> ProfileImportPlan {
        ProfileImportPlan {
            actions: self.actions,
            resolved_profile_names: self.resolved_profile_names,
        }
    }
}

pub fn profile_import_identity_target_key(identity: &ProfileImportIdentity) -> Option<String> {
    identity.target_key()
}

pub fn profile_import_identity_parts_target_key(
    account_id: Option<&str>,
    email: Option<&str>,
) -> Option<String> {
    let account_id = account_id
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty());
    let email = email
        .map(str::trim)
        .filter(|email| !email.is_empty())
        .map(str::to_ascii_lowercase);

    match (account_id, email) {
        (Some(account_id), Some(email)) => Some(format!("account:{account_id}|email:{email}")),
        (Some(account_id), None) => Some(format!("account:{account_id}")),
        (None, Some(email)) => Some(format!("email:{email}")),
        (None, None) => None,
    }
}

pub fn validate_profile_import_source_names<'a>(
    profile_names: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    let mut seen_names = BTreeSet::new();
    for profile_name in profile_names {
        if !seen_names.insert(profile_name.to_string()) {
            bail!(
                "profile export bundle contains duplicate profile '{}'",
                profile_name
            );
        }
    }
    Ok(())
}

pub fn resolve_profile_import_identity(
    mut auth_identity: ProfileImportIdentity,
    fallback_email: Option<&str>,
) -> ProfileImportIdentity {
    if auth_identity.email.is_none() {
        auth_identity.email = fallback_email
            .map(str::trim)
            .filter(|email| !email.is_empty())
            .map(ToOwned::to_owned);
    }
    auth_identity
}

pub fn queue_profile_import_auth_update(
    auth_updates: &mut Vec<ProfileImportAuthUpdatePlan>,
    target_profile_name: &str,
    email: Option<String>,
    auth_json: String,
) {
    if let Some(existing) = auth_updates
        .iter_mut()
        .find(|update| update.target_profile_name == target_profile_name)
    {
        existing.auth_json = auth_json;
        if email.is_some() {
            existing.email = email;
        }
        return;
    }

    auth_updates.push(ProfileImportAuthUpdatePlan {
        target_profile_name: target_profile_name.to_string(),
        email,
        auth_json,
    });
}
