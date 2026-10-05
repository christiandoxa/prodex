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
    let selected = selected_profile_names.into_iter().collect::<Vec<_>>();
    prodex_mojo_core::profile_export::profile_export_active_profile_selected(
        active_profile,
        &selected,
    )
    .expect("Mojo profile-export active-profile selection returned invalid output")
    .then(|| {
        active_profile
            .expect("Mojo selected a missing active profile")
            .to_string()
    })
}

pub fn resolve_imported_active_profile(
    existing_active_profile: Option<&str>,
    source_active_profile: Option<&str>,
    resolved_profile_names: &BTreeMap<String, String>,
) -> Option<String> {
    let resolved = resolved_profile_names
        .iter()
        .map(|(source, target)| (source.as_str(), target.as_str()))
        .collect::<Vec<_>>();
    match prodex_mojo_core::profile_export::profile_import_active_profile_plan(
        existing_active_profile,
        source_active_profile,
        &resolved,
    )
    .expect("Mojo profile-import active-profile plan returned invalid output")
    {
        prodex_mojo_core::profile_export::ProfileImportActiveProfilePlan::None => None,
        prodex_mojo_core::profile_export::ProfileImportActiveProfilePlan::Existing => Some(
            existing_active_profile
                .expect("validated Mojo existing active-profile plan")
                .to_string(),
        ),
        prodex_mojo_core::profile_export::ProfileImportActiveProfilePlan::Resolved(index) => Some(
            resolved
                .get(index)
                .expect("validated Mojo resolved active-profile index")
                .1
                .to_string(),
        ),
    }
}
