use super::{StringView, signed, status};
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const HEADER_WORDS: usize = 5;

#[repr(C)]
#[derive(Clone, Copy)]
struct SessionRepairLineFfi {
    text: StringView,
    blank: i64,
    valid_json: i64,
    starts_resume_metadata: i64,
    matches_selector: i64,
    starts_codex_metadata: i64,
}

const _: () = {
    assert!(std::mem::size_of::<SessionRepairLineFfi>() == 56);
    assert!(std::mem::align_of::<SessionRepairLineFfi>() == 8);
};

unsafe extern "C" {
    fn prodex_session_repair_synthetic_plan_v1(
        abi_version: i64,
        selector_address: u64,
        selector_length: i64,
        lines_address: u64,
        lines_count: i64,
        path_id_present: i64,
        selector_is_full: i64,
        output_address: u64,
    ) -> i64;

    fn prodex_session_repair_synthetic_json_v1(
        abi_version: i64,
        input_address: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;

    fn prodex_session_repair_plan_v1(
        abi_version: i64,
        selector_address: u64,
        selector_length: i64,
        lines_address: u64,
        lines_count: i64,
        synthesize_missing: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SessionSyntheticMetadataInputFfi {
    session_id: StringView,
    timestamp: StringView,
    cwd: StringView,
    model_provider: StringView,
    model_provider_present: i64,
    cli_version: StringView,
}

const _: () = {
    assert!(std::mem::size_of::<SessionSyntheticMetadataInputFfi>() == 88);
    assert!(std::mem::align_of::<SessionSyntheticMetadataInputFfi>() == 8);
};

/// Rust-observed facts for one decoded JSONL line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionRepairLine<'a> {
    pub text: &'a str,
    pub blank: bool,
    pub valid_json: bool,
    pub starts_resume_metadata: bool,
    pub matches_selector: bool,
    pub starts_codex_metadata: bool,
}

/// The complete rewrite decision for one session repair transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRepairPlan {
    pub should_repair: bool,
    pub selected_line: Option<usize>,
    pub first_content_line: Option<usize>,
    pub synthesize_metadata: bool,
    pub first_line_matches_selector: bool,
    pub keep_lines: Vec<bool>,
}

/// A JSON string token selected from one observed session line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSyntheticSpan {
    pub line_index: usize,
    pub start: usize,
    pub end: usize,
}

/// Source chosen for the repaired session ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionSyntheticSessionIdSource {
    Line(SessionSyntheticSpan),
    Path,
    Selector,
}

/// Deterministic field precedence for synthetic session metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSyntheticMetadataPlan {
    pub session_id: SessionSyntheticSessionIdSource,
    pub timestamp: Option<SessionSyntheticSpan>,
    pub cwd: Option<SessionSyntheticSpan>,
    pub model_provider: Option<SessionSyntheticSpan>,
}

fn synthetic_span(
    output: &[i64; 13],
    offset: usize,
    lines: &[SessionRepairLine<'_>],
) -> Result<Option<SessionSyntheticSpan>, MojoError> {
    let line_index = output[offset];
    let start = output[offset + 1];
    let end = output[offset + 2];
    if line_index == -1 && start == -1 && end == -1 {
        return Ok(None);
    }
    let line_index = usize::try_from(line_index).map_err(|_| MojoError::InvalidOutput)?;
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
    let line = lines.get(line_index).ok_or(MojoError::InvalidOutput)?;
    let token = line.text.get(start..end).ok_or(MojoError::InvalidOutput)?;
    if end <= start || !token.starts_with('"') || !token.ends_with('"') {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(SessionSyntheticSpan {
        line_index,
        start,
        end,
    }))
}

/// Plan synthetic session metadata from Rust-observed JSONL facts.
pub fn session_synthetic_metadata_plan(
    selector: &str,
    lines: &[SessionRepairLine<'_>],
    path_id_present: bool,
    selector_is_full: bool,
) -> Result<Option<SessionSyntheticMetadataPlan>, MojoError> {
    if selector.is_empty() || lines.len() > i64::MAX as usize {
        return Err(MojoError::InvalidInput);
    }
    let ffi_lines = lines
        .iter()
        .map(|line| {
            Ok(SessionRepairLineFfi {
                text: line.text.into(),
                blank: i64::from(line.blank),
                valid_json: i64::from(line.valid_json),
                starts_resume_metadata: i64::from(line.starts_resume_metadata),
                matches_selector: i64::from(line.matches_selector),
                starts_codex_metadata: i64::from(line.starts_codex_metadata),
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let mut output = [-1_i64; 13];
    status(unsafe {
        prodex_session_repair_synthetic_plan_v1(
            ABI_VERSION,
            selector.as_ptr() as u64,
            signed(selector.len())?,
            ffi_lines.as_ptr() as u64,
            signed(ffi_lines.len())?,
            i64::from(path_id_present),
            i64::from(selector_is_full),
            output.as_mut_ptr() as u64,
        )
    })?;

    let session_id = match output[0] {
        0 => return Ok(None),
        1 => SessionSyntheticSessionIdSource::Line(
            synthetic_span(&output, 1, lines)?.ok_or(MojoError::InvalidOutput)?,
        ),
        2 if path_id_present => SessionSyntheticSessionIdSource::Path,
        3 if selector_is_full => SessionSyntheticSessionIdSource::Selector,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(Some(SessionSyntheticMetadataPlan {
        session_id,
        timestamp: synthetic_span(&output, 4, lines)?,
        cwd: synthetic_span(&output, 7, lines)?,
        model_provider: synthetic_span(&output, 10, lines)?,
    }))
}

/// Encode the repaired metadata line through the Mojo JSON sink.
pub fn session_synthetic_metadata_json(
    session_id: &str,
    timestamp: &str,
    cwd: &str,
    model_provider: Option<&str>,
    cli_version: &str,
) -> Result<String, MojoError> {
    let input = SessionSyntheticMetadataInputFfi {
        session_id: session_id.into(),
        timestamp: timestamp.into(),
        cwd: cwd.into(),
        model_provider: model_provider.unwrap_or_default().into(),
        model_provider_present: i64::from(model_provider.is_some()),
        cli_version: cli_version.into(),
    };
    let mut written = -1_i64;
    status(unsafe {
        prodex_session_repair_synthetic_json_v1(
            ABI_VERSION,
            (&input as *const SessionSyntheticMetadataInputFfi) as u64,
            0,
            0,
            (&mut written as *mut i64) as u64,
        )
    })?;
    let required = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(required)
        .map_err(|_| MojoError::Capacity)?;
    output.resize(required, 0);
    written = -1;
    status(unsafe {
        prodex_session_repair_synthetic_json_v1(
            ABI_VERSION,
            (&input as *const SessionSyntheticMetadataInputFfi) as u64,
            output.as_mut_ptr() as u64,
            signed(output.len())?,
            (&mut written as *mut i64) as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

pub fn session_repair_plan(
    selector: &str,
    lines: &[SessionRepairLine<'_>],
    synthesize_metadata: bool,
) -> Result<SessionRepairPlan, MojoError> {
    if selector.is_empty() || lines.len() > (i64::MAX as usize).saturating_sub(HEADER_WORDS) {
        return Err(MojoError::InvalidInput);
    }
    let ffi_lines = lines
        .iter()
        .map(|line| {
            Ok(SessionRepairLineFfi {
                text: line.text.into(),
                blank: i64::from(line.blank),
                valid_json: i64::from(line.valid_json),
                starts_resume_metadata: i64::from(line.starts_resume_metadata),
                matches_selector: i64::from(line.matches_selector),
                starts_codex_metadata: i64::from(line.starts_codex_metadata),
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let output_capacity = HEADER_WORDS
        .checked_add(lines.len())
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![-1_i64; output_capacity];
    status(unsafe {
        prodex_session_repair_plan_v1(
            ABI_VERSION,
            selector.as_ptr() as u64,
            signed(selector.len())?,
            ffi_lines.as_ptr() as u64,
            signed(ffi_lines.len())?,
            i64::from(synthesize_metadata),
            output.as_mut_ptr() as u64,
            signed(output.len())?,
        )
    })?;

    let bool_value = |value: i64| match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let index = |value: i64| match value {
        -1 => Ok(None),
        value if value >= 0 => usize::try_from(value)
            .ok()
            .filter(|index| *index < lines.len())
            .map(Some)
            .ok_or(MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    };
    let should_repair = bool_value(output[0])?;
    let selected_line = index(output[1])?;
    let first_content_line = index(output[2])?;
    let synthesize_metadata = bool_value(output[3])?;
    let first_line_matches_selector = bool_value(output[4])?;
    if synthesize_metadata && selected_line.is_some() {
        return Err(MojoError::InvalidOutput);
    }
    if should_repair && !synthesize_metadata && selected_line.is_none() {
        return Err(MojoError::InvalidOutput);
    }
    if !should_repair && (synthesize_metadata || selected_line.is_some()) {
        return Err(MojoError::InvalidOutput);
    }
    let keep_lines = output[HEADER_WORDS..]
        .iter()
        .map(|value| bool_value(*value))
        .collect::<Result<Vec<_>, MojoError>>()?;
    Ok(SessionRepairPlan {
        should_repair,
        selected_line,
        first_content_line,
        synthesize_metadata,
        first_line_matches_selector,
        keep_lines,
    })
}

#[cfg(all(test, prodex_mojo_active))]
mod tests {
    use super::*;

    fn line(
        text: &'static str,
        blank: bool,
        valid_json: bool,
        starts_resume_metadata: bool,
        matches_selector: bool,
        starts_codex_metadata: bool,
    ) -> SessionRepairLine<'static> {
        SessionRepairLine {
            text,
            blank,
            valid_json,
            starts_resume_metadata,
            matches_selector,
            starts_codex_metadata,
        }
    }

    #[test]
    fn repair_plan_golden_promotes_late_metadata_and_drops_corrupt_duplicates() {
        let lines = [
            line("event", false, true, false, false, false),
            line("{broken", false, false, false, false, false),
            line("metadata", false, true, true, true, true),
            line("duplicate", false, true, true, true, false),
            line("chat", false, true, false, false, false),
        ];
        let plan = session_repair_plan("session", &lines, false).unwrap();
        assert!(plan.should_repair);
        assert_eq!(plan.selected_line, Some(2));
        assert_eq!(plan.first_content_line, Some(0));
        assert!(!plan.synthesize_metadata);
        assert_eq!(plan.keep_lines, [true, false, false, false, true]);
    }

    #[test]
    fn clean_prefix_is_a_noop_and_preserves_unicode_line_boundaries() {
        let lines = [line(
            "\u{3000}metadata\u{00a0}",
            false,
            true,
            true,
            true,
            true,
        )];
        let plan = session_repair_plan("session", &lines, true).unwrap();
        assert!(!plan.should_repair);
        assert_eq!(plan.first_content_line, Some(0));
        assert_eq!(plan.keep_lines, [false]);
    }

    #[test]
    fn missing_metadata_needs_explicit_synthesis_and_keeps_valid_chat() {
        let lines = [line("chat", false, true, false, false, false)];
        let no_synthesis = session_repair_plan("session", &lines, false).unwrap();
        assert!(!no_synthesis.should_repair);
        let synthesis = session_repair_plan("session", &lines, true).unwrap();
        assert!(synthesis.should_repair);
        assert!(synthesis.synthesize_metadata);
        assert_eq!(synthesis.selected_line, None);
        assert_eq!(synthesis.keep_lines, [true]);
    }

    #[test]
    fn synthetic_plan_keeps_id_precedence_and_first_valid_repair_fields() {
        let lines = [
            line(
                r#"{"type":"session_meta","timestamp":" ","payload":{"id":"line-id","timestamp":" ","cwd":" /first ","model_provider":" "}}"#,
                false,
                true,
                true,
                true,
                false,
            ),
            line(
                r#"{"type":"event","timestamp":"first-time","payload":{"cwd":"/second","model_provider":"provider-one"}}"#,
                false,
                true,
                false,
                false,
                false,
            ),
            line(
                r#"{"type":"event","payload":{"model_provider":"provider-two"}}"#,
                false,
                true,
                false,
                false,
                false,
            ),
        ];
        let plan = session_synthetic_metadata_plan("line", &lines, true, true)
            .unwrap()
            .unwrap();
        assert!(matches!(
            plan.session_id,
            SessionSyntheticSessionIdSource::Line(SessionSyntheticSpan { line_index: 0, .. })
        ));
        assert_eq!(plan.timestamp.map(|span| span.line_index), Some(1));
        assert_eq!(plan.cwd.map(|span| span.line_index), Some(0));
        assert_eq!(plan.model_provider.map(|span| span.line_index), Some(1));

        let no_line_match = [line(
            "{\"type\":\"event\"}",
            false,
            true,
            false,
            false,
            false,
        )];
        assert!(matches!(
            session_synthetic_metadata_plan("line", &no_line_match, true, true)
                .unwrap()
                .unwrap()
                .session_id,
            SessionSyntheticSessionIdSource::Path
        ));
        assert!(matches!(
            session_synthetic_metadata_plan(
                "01900000-0000-7000-8000-000000000001",
                &no_line_match,
                false,
                true
            )
            .unwrap()
            .unwrap()
            .session_id,
            SessionSyntheticSessionIdSource::Selector
        ));
    }

    #[test]
    fn synthetic_json_escapes_values_and_omits_missing_provider() {
        let with_provider = session_synthetic_metadata_json(
            "id\"\\",
            "2026-06-13T02:04:31Z\n",
            "/tmp/workspace\tname",
            Some("provider"),
            "0.1\"",
        )
        .unwrap();
        assert_eq!(
            with_provider,
            r#"{"payload":{"cli_version":"0.1\"","cwd":"/tmp/workspace\tname","id":"id\"\\","model_provider":"provider","originator":"prodex-repair","session_id":"id\"\\","source":"cli","timestamp":"2026-06-13T02:04:31Z\n"},"timestamp":"2026-06-13T02:04:31Z\n","type":"session_meta"}"#
        );

        let without_provider =
            session_synthetic_metadata_json("id", "time", "/tmp/workspace", None, "0.1").unwrap();
        assert_eq!(
            without_provider,
            r#"{"payload":{"cli_version":"0.1","cwd":"/tmp/workspace","id":"id","originator":"prodex-repair","session_id":"id","source":"cli","timestamp":"time"},"timestamp":"time","type":"session_meta"}"#
        );
    }
}
