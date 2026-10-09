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
}
