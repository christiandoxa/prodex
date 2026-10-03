use std::collections::BTreeMap;

use anyhow::{Result, bail};
use prodex_mojo_core::profile_export::{
    ProfileImportIdentityLookup, ProfileImportPlanAction as MojoImportAction,
    ProfileImportPlanInput as MojoImportInput, ProfileImportPlanStep, ProfileImportPlanTarget,
    profile_import_duplicate_name_index, profile_import_plan_step,
};

use crate::{
    ProfileImportAuthUpdatePlan, ProfileImportIdentity, ProfileImportPlan, ProfileImportPlanAction,
    ProfileImportPlanInput,
};

pub fn plan_profile_import(
    profiles: &[ProfileImportPlanInput],
    existing_profile_supports_codex_runtime: impl Fn(&str) -> Option<bool>,
    mut find_existing_profile_by_identity: impl FnMut(&ProfileImportIdentity) -> Result<Option<String>>,
) -> Result<ProfileImportPlan> {
    let identity_keys = profiles
        .iter()
        .map(|profile| {
            prodex_mojo_core::profile_identity::canonical_profile_identity_key(
                profile.identity.account_id.as_deref(),
                profile.identity.email.as_deref(),
            )
            .map_err(|error| anyhow::anyhow!("Mojo profile-import identity failed: {error:?}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let existing_profiles = profiles
        .iter()
        .map(|profile| existing_profile_supports_codex_runtime(&profile.profile_name))
        .collect::<Vec<_>>();
    let mut identity_lookups = vec![None::<Option<String>>; profiles.len()];

    loop {
        let inputs = profiles
            .iter()
            .enumerate()
            .map(|(index, profile)| {
                let identity_lookup = match identity_lookups[index].as_ref() {
                    Some(Some(profile_name)) => {
                        ProfileImportIdentityLookup::Found(profile_name.as_str())
                    }
                    Some(None) => ProfileImportIdentityLookup::Missing,
                    None if profile.supports_codex_runtime && identity_keys[index].is_some() => {
                        ProfileImportIdentityLookup::Pending
                    }
                    None => ProfileImportIdentityLookup::NotRequested,
                };
                MojoImportInput {
                    profile_name: &profile.profile_name,
                    identity_key: identity_keys[index].as_deref(),
                    supports_codex_runtime: profile.supports_codex_runtime,
                    existing_profile_supports_codex_runtime: existing_profiles[index],
                    identity_lookup,
                }
            })
            .collect::<Vec<_>>();

        match profile_import_plan_step(&inputs)
            .map_err(|error| anyhow::anyhow!("Mojo profile-import planner failed: {error:?}"))?
        {
            ProfileImportPlanStep::Empty => {
                bail!("profile export bundle does not contain any profiles");
            }
            ProfileImportPlanStep::DuplicateName(index) => {
                let profile = &profiles[index];
                bail!(
                    "profile export bundle contains duplicate profile '{}'",
                    profile.profile_name
                );
            }
            ProfileImportPlanStep::ProviderMismatch(index) => {
                let profile = &profiles[index];
                bail!(
                    "profile '{}' already exists with an incompatible provider",
                    profile.profile_name
                );
            }
            ProfileImportPlanStep::LookupIdentity(index) => {
                identity_lookups[index] = Some(find_existing_profile_by_identity(
                    &profiles[index].identity,
                )?);
            }
            ProfileImportPlanStep::Complete(mojo_actions) => {
                return build_profile_import_plan(profiles, &identity_lookups, mojo_actions);
            }
        }
    }
}

fn build_profile_import_plan(
    profiles: &[ProfileImportPlanInput],
    identity_lookups: &[Option<Option<String>>],
    mojo_actions: Vec<MojoImportAction>,
) -> Result<ProfileImportPlan> {
    let mut actions = Vec::with_capacity(mojo_actions.len());
    let mut resolved_profile_names = BTreeMap::new();

    for action in mojo_actions {
        match action {
            MojoImportAction::UpdateExisting {
                source_index,
                target: ProfileImportPlanTarget::SourceProfile(target_index),
            } => {
                let target_profile_name = profiles
                    .get(target_index)
                    .ok_or_else(|| anyhow::anyhow!("Mojo import target index is invalid"))?
                    .profile_name
                    .clone();
                resolved_profile_names.insert(
                    profiles[source_index].profile_name.clone(),
                    target_profile_name.clone(),
                );
                actions.push(ProfileImportPlanAction::UpdateExisting {
                    source_index,
                    target_profile_name,
                });
            }
            MojoImportAction::UpdateExisting {
                source_index,
                target: ProfileImportPlanTarget::ExistingProfileLookup(lookup_index),
            } => {
                let target_profile_name = identity_lookups
                    .get(lookup_index)
                    .and_then(Option::as_ref)
                    .and_then(Option::as_ref)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Mojo import lookup target is missing"))?;
                resolved_profile_names.insert(
                    profiles[source_index].profile_name.clone(),
                    target_profile_name.clone(),
                );
                actions.push(ProfileImportPlanAction::UpdateExisting {
                    source_index,
                    target_profile_name,
                });
            }
            MojoImportAction::StageNew {
                source_index,
                staged_index,
            } => {
                let profile_name = profiles
                    .get(source_index)
                    .ok_or_else(|| anyhow::anyhow!("Mojo staged source index is invalid"))?
                    .profile_name
                    .clone();
                resolved_profile_names.insert(profile_name.clone(), profile_name);
                actions.push(ProfileImportPlanAction::StageNew {
                    source_index,
                    staged_index,
                });
            }
            MojoImportAction::RewriteStagedAuth {
                source_index,
                staged_index,
                target_source_index,
            } => {
                let source_name = profiles
                    .get(source_index)
                    .ok_or_else(|| anyhow::anyhow!("Mojo rewrite source index is invalid"))?
                    .profile_name
                    .clone();
                let target_name = profiles
                    .get(target_source_index)
                    .ok_or_else(|| anyhow::anyhow!("Mojo rewrite target index is invalid"))?
                    .profile_name
                    .clone();
                resolved_profile_names.insert(source_name, target_name);
                actions.push(ProfileImportPlanAction::RewriteStagedAuth {
                    source_index,
                    staged_index,
                });
            }
        }
    }

    Ok(ProfileImportPlan {
        actions,
        resolved_profile_names,
    })
}

pub fn profile_import_identity_target_key(identity: &ProfileImportIdentity) -> Option<String> {
    identity.target_key()
}

pub fn profile_import_identity_parts_target_key(
    account_id: Option<&str>,
    email: Option<&str>,
) -> Option<String> {
    prodex_mojo_core::profile_identity::canonical_profile_identity_key(account_id, email)
        .expect("Mojo profile-import identity-key policy returned invalid output")
}

pub fn validate_profile_import_source_names<'a>(
    profile_names: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    let profile_names = profile_names.into_iter().collect::<Vec<_>>();
    if let Some(index) = profile_import_duplicate_name_index(&profile_names)
        .map_err(|error| anyhow::anyhow!("Mojo profile-import name validation failed: {error:?}"))?
    {
        bail!(
            "profile export bundle contains duplicate profile '{}'",
            profile_names[index]
        );
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
    use prodex_mojo_core::profile_export::ProfileImportAuthUpdateAction;

    let existing_index = auth_updates
        .iter()
        .position(|update| update.target_profile_name == target_profile_name);
    let action = prodex_mojo_core::profile_export::profile_import_auth_update_action(
        existing_index.is_some(),
        email.is_some(),
    )
    .expect("Mojo profile-import auth-update policy returned invalid output");

    match action {
        ProfileImportAuthUpdateAction::Append => {
            auth_updates.push(ProfileImportAuthUpdatePlan {
                target_profile_name: target_profile_name.to_string(),
                email,
                auth_json,
            });
        }
        ProfileImportAuthUpdateAction::ReplaceAuth => {
            auth_updates
                .get_mut(existing_index.expect("Mojo replace action requires existing update"))
                .expect("existing auth-update index remains valid")
                .auth_json = auth_json;
        }
        ProfileImportAuthUpdateAction::ReplaceAuthAndEmail => {
            let existing = auth_updates
                .get_mut(existing_index.expect("Mojo replace action requires existing update"))
                .expect("existing auth-update index remains valid");
            existing.auth_json = auth_json;
            existing.email = email;
        }
    }
}
