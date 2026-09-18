use super::{DoctorPanel, first_line_of_error};
use anyhow::Result;
use redaction::redaction_redact_secret_like_text;
use terminal_ui::{print_blank_line, print_panel, print_stdout_line};

pub(super) fn print_doctor_output(
    panels: &[DoctorPanel],
    suggestion_lines: &[String],
) -> Result<()> {
    for panel in panels {
        print_panel(&panel.title, &panel.fields)?;
    }
    if !suggestion_lines.is_empty() {
        print_blank_line()?;
        for line in suggestion_lines {
            print_stdout_line(line)?;
        }
    }
    Ok(())
}

pub(super) fn doctor_quota_error_summary(err: &str) -> String {
    let redacted = redaction_redact_secret_like_text(err);
    format!("Error ({})", first_line_of_error(&redacted))
}

#[cfg(test)]
mod tests {
    use super::doctor_quota_error_summary;

    #[test]
    fn doctor_quota_error_summary_redacts_secret_like_material() {
        let err =
            "failed: Authorization: Bearer <redacted> url=https://example.test?api_key=<redacted>";

        let summary = doctor_quota_error_summary(err);

        assert!(summary.contains("Authorization: Bearer <redacted>"));
        assert!(summary.contains("api_key=<redacted>"));
        assert!(!summary.contains("fixture-token-123"));
        assert!(!summary.contains("sk-<redacted>"));
    }
}
