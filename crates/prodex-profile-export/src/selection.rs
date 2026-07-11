use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};

pub fn resolve_requested_profile_names(
    available_names: &BTreeSet<String>,
    requested_names: &[String],
) -> Result<Vec<String>> {
    if available_names.is_empty() {
        bail!("no profiles configured");
    }

    if requested_names.is_empty() {
        return Ok(available_names.iter().cloned().collect());
    }

    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for name in requested_names {
        if !seen.insert(name.clone()) {
            continue;
        }
        if !available_names.contains(name) {
            bail!("profile '{}' does not exist", name);
        }
        names.push(name.clone());
    }
    Ok(names)
}

pub fn resolve_profile_export_active_profile<'a>(
    active_profile: Option<&str>,
    selected_profile_names: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let active_profile = active_profile?;
    selected_profile_names
        .into_iter()
        .any(|name| name == active_profile)
        .then(|| active_profile.to_string())
}

pub fn resolve_imported_active_profile(
    existing_active_profile: Option<&str>,
    source_active_profile: Option<&str>,
    resolved_profile_names: &BTreeMap<String, String>,
) -> Option<String> {
    existing_active_profile.map(ToOwned::to_owned).or_else(|| {
        source_active_profile.and_then(|active| resolved_profile_names.get(active).cloned())
    })
}
