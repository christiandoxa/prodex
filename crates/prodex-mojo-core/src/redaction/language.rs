//! Compiled Presidio language selection over borrowed text and candidate names.
use super::{DryRunPathView, status};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_redaction_language_plan_v1(
        abi: i64,
        text: u64,
        length: i64,
        candidates: u64,
        count: i64,
        output: u64,
    ) -> i64;
}

/// Selects one configured candidate, preserving substring scores and tie priority.
pub fn presidio_language_plan(
    text: &str,
    candidates: &[String],
) -> Result<Option<usize>, MojoError> {
    let views = candidates
        .iter()
        .map(|value| DryRunPathView {
            address: value.as_ptr() as usize as u64,
            length: value.len() as u64,
        })
        .collect::<Vec<_>>();
    let mut selected = -2_i64;
    status(unsafe {
        prodex_redaction_language_plan_v1(
            1,
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
            views.as_ptr() as usize as u64,
            i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut selected as *mut i64) as usize as u64,
        )
    })?;
    match selected {
        -1 if candidates.is_empty() => Ok(None),
        value if value >= 0 && (value as usize) < candidates.len() => Ok(Some(value as usize)),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_presidio_language_selects_configured_candidates_and_unicode_expansions() {
        let candidates = vec!["en".to_string(), "id".to_string()];
        for (text, expected) in [
            ("NAMA DAN ALAMAT SAYA", 1),
            ("my name and phone address", 0),
            ("Dİ", 1),
            ("KE", 1),
            ("🧭東京", 0),
            ("İD", 0),
        ] {
            assert_eq!(
                presidio_language_plan(text, &candidates),
                Ok(Some(expected)),
                "{text}"
            );
        }
        assert_eq!(presidio_language_plan("anything", &[]), Ok(None));
        assert_eq!(
            presidio_language_plan("DAN ALAMAT", &["fr".to_string()]),
            Ok(Some(0))
        );
        assert_eq!(
            presidio_language_plan("DAN ALAMAT", &["fr".to_string(), "en".to_string()]),
            Ok(Some(0))
        );
    }

    #[test]
    fn compiled_presidio_language_matches_rust_unicode_ascii_lowercase_expansions() {
        let exceptions = (128..=0x10ffff)
            .filter_map(char::from_u32)
            .filter(|ch| ch.to_lowercase().any(|lower| lower.is_ascii()))
            .collect::<Vec<_>>();
        assert_eq!(exceptions, vec!['\u{0130}', '\u{212a}']);
    }

    #[test]
    fn compiled_presidio_language_rejects_abi_and_bad_views_before_writing() {
        let mut selected = 71_i64;
        let output = (&mut selected as *mut i64) as usize as u64;
        assert_eq!(
            unsafe { prodex_redaction_language_plan_v1(2, 0, 0, 0, 0, output) },
            4
        );
        assert_eq!(selected, 71);
        assert_eq!(
            unsafe { prodex_redaction_language_plan_v1(1, 0, 1, 0, 0, output) },
            1
        );
        assert_eq!(selected, 71);
    }
}
