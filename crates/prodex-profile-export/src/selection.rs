use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};

pub fn resolve_requested_profile_names(
    available_names: &BTreeSet<String>,
    requested_names: &[String],
) -> Result<Vec<String>> {
    let available = available_names
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let requested = requested_names
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    match prodex_mojo_core::profile_export::profile_export_selection_plan(&available, &requested)
        .map_err(|error| anyhow::anyhow!("Mojo profile-export selection failed: {error:?}"))?
    {
        prodex_mojo_core::profile_export::ProfileExportSelectionPlan::Selected(indices) => {
            Ok(indices
                .into_iter()
                .map(|index| {
                    available
                        .get(index)
                        .expect("validated Mojo profile-selection index")
                        .to_string()
                })
                .collect())
        }
        prodex_mojo_core::profile_export::ProfileExportSelectionPlan::NoProfiles => {
            bail!("no profiles configured")
        }
        prodex_mojo_core::profile_export::ProfileExportSelectionPlan::MissingRequested(index) => {
            let name = requested_names
                .get(index)
                .expect("validated Mojo missing-request index");
            bail!("profile '{}' does not exist", name)
        }
    }
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
