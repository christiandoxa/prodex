use crate::{ProfileExportSummary, ProfileImportSummary, SummaryFields};

pub fn profile_export_summary_fields(summary: ProfileExportSummary) -> SummaryFields {
    let mut fields = vec![
        (
            "Result".to_string(),
            format!("Exported {} profile(s).", summary.profile_count),
        ),
        ("Path".to_string(), summary.path),
        ("Encrypted".to_string(), yes_no(summary.encrypted)),
    ];
    if let Some(active_profile) = summary.active_profile {
        fields.push(("Active".to_string(), active_profile));
    }
    fields
}

pub fn profile_import_summary_fields(summary: ProfileImportSummary) -> SummaryFields {
    let mut fields = vec![
        (
            "Result".to_string(),
            profile_import_result_message(summary.imported_count, summary.updated_existing_count),
        ),
        ("Path".to_string(), summary.path),
        ("Encrypted".to_string(), yes_no(summary.encrypted)),
        ("Imported".to_string(), summary.imported_count.to_string()),
        (
            "Updated duplicates".to_string(),
            summary.updated_existing_count.to_string(),
        ),
    ];
    if let Some(active_profile) = summary.source_active_profile {
        fields.push(("Source active".to_string(), active_profile));
    }
    if let Some(active_profile) = summary.active_profile {
        fields.push(("Active".to_string(), active_profile));
    }
    fields
}

pub fn profile_import_result_message(
    imported_count: usize,
    updated_existing_count: usize,
) -> String {
    match (imported_count, updated_existing_count) {
        (0, updated) => format!("Updated {updated} existing profile(s)."),
        (imported, 0) => format!("Imported {imported} profile(s)."),
        (imported, updated) => {
            format!("Imported {imported} profile(s) and updated {updated} existing profile(s).")
        }
    }
}

fn yes_no(value: bool) -> String {
    if value {
        "Yes".to_string()
    } else {
        "No".to_string()
    }
}
