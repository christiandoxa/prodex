use super::{StringView, status};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_session_selector_is_full_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_selector_matches_v1(
        abi_version: i64,
        id_address: u64,
        id_length: i64,
        selector_address: u64,
        selector_length: i64,
        exact: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_selector_match_plan_v1(
        abi_version: i64,
        ids_address: u64,
        ids_count: i64,
        selector_address: u64,
        selector_length: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_session_report_query_matches_v1(
        abi_version: i64,
        query_address: u64,
        query_length: i64,
        fields_address: u64,
        fields_count: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_file_name_policy_v1(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
        output_address: u64,
    ) -> i64;
}

const SESSION_SELECTOR_ABI_VERSION: i64 = 1;
const SESSION_REPORT_QUERY_FIELD_COUNT: usize = 6;

#[repr(C)]
#[derive(Clone, Copy)]
struct SessionSelectorIdFfi {
    id: StringView,
}

const _: () = {
    assert!(std::mem::size_of::<SessionSelectorIdFfi>() == 16);
    assert!(std::mem::align_of::<SessionSelectorIdFfi>() == 8);
};

fn view(value: &str) -> StringView {
    value.into()
}

pub fn session_selector_is_full(value: &str) -> Result<bool, MojoError> {
    let mut output = [-1_i64; 1];
    let value = view(value);
    status(unsafe {
        prodex_session_selector_is_full_v1(
            SESSION_SELECTOR_ABI_VERSION,
            value.address,
            i64::try_from(value.length).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn session_selector_matches(id: &str, selector: &str, exact: bool) -> Result<bool, MojoError> {
    let mut output = [-1_i64; 1];
    let id = view(id);
    let selector = view(selector);
    status(unsafe {
        prodex_session_selector_matches_v1(
            SESSION_SELECTOR_ABI_VERSION,
            id.address,
            i64::try_from(id.length).map_err(|_| MojoError::InvalidInput)?,
            selector.address,
            i64::try_from(selector.length).map_err(|_| MojoError::InvalidInput)?,
            i64::from(exact),
            output.as_mut_ptr() as u64,
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Classifies a session filename and returns the suffix-stripped stem length.
///
/// The filename is supplied by the storage adapter; Mojo owns the extension
/// precedence used by both metadata discovery and report ID initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionFileNamePlan {
    pub is_metadata: bool,
    pub stem_length: usize,
}

pub fn session_file_name_plan(name: &str) -> Result<SessionFileNamePlan, MojoError> {
    let mut output = [-1_i64; 2];
    status(unsafe {
        prodex_session_file_name_policy_v1(
            SESSION_SELECTOR_ABI_VERSION,
            name.as_ptr() as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
        )
    })?;
    let is_metadata = match output[0] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let stem_length = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    if stem_length > name.len() || !name.is_char_boundary(stem_length) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(SessionFileNamePlan {
        is_metadata,
        stem_length,
    })
}

/// Match a query against the six normalized fields shown by `prodex session list`.
///
/// Rust applies the standard-library Unicode lowercase mapping before crossing the
/// ABI. Mojo then owns the bounded, allocation-free substring scan over those
/// caller-owned UTF-8 views. The fields are ordered as id, thread name, cwd,
/// profile, model provider, and path; `None` represents an absent optional field.
pub fn session_report_query_matches(
    query: &str,
    fields: &[Option<&str>; SESSION_REPORT_QUERY_FIELD_COUNT],
) -> Result<bool, MojoError> {
    // Own the normalized buffers for the whole FFI call. Applying this at the
    // adapter boundary guarantees the documented contract for every caller.
    let normalized_query = query.to_lowercase();
    let normalized_fields = fields.map(|field| field.map(str::to_lowercase));
    let query = view(&normalized_query);
    let ffi_fields = normalized_fields
        .iter()
        .map(|field| field.as_deref().map_or_else(StringView::default, view))
        .collect::<Vec<_>>();
    let mut output = [-1_i64; 1];
    status(unsafe {
        prodex_session_report_query_matches_v1(
            SESSION_SELECTOR_ABI_VERSION,
            query.address,
            i64::try_from(query.length).map_err(|_| MojoError::InvalidInput)?,
            ffi_fields.as_ptr() as u64,
            i64::try_from(ffi_fields.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Match a selector against report IDs once, preserving input order for both
/// exact and prefix candidates so the caller can apply ambiguity rules.
pub fn session_selector_match_plan(
    ids: &[&str],
    selector: &str,
) -> Result<(Vec<usize>, Vec<usize>), MojoError> {
    if ids.len() > (i64::MAX as usize).saturating_sub(2) / 2 {
        return Err(MojoError::InvalidInput);
    }
    let ffi_ids = ids
        .iter()
        .map(|id| SessionSelectorIdFfi { id: view(id) })
        .collect::<Vec<_>>();
    let output_capacity = 2usize
        .checked_add(ids.len().checked_mul(2).ok_or(MojoError::InvalidInput)?)
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![-1_i64; output_capacity];
    status(unsafe {
        prodex_session_selector_match_plan_v1(
            SESSION_SELECTOR_ABI_VERSION,
            ffi_ids.as_ptr() as u64,
            i64::try_from(ffi_ids.len()).map_err(|_| MojoError::InvalidInput)?,
            selector.as_ptr() as u64,
            i64::try_from(selector.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    })?;
    let count = |slot: usize| -> Result<usize, MojoError> {
        usize::try_from(output[slot]).map_err(|_| MojoError::InvalidOutput)
    };
    let exact_count = count(0)?;
    let prefix_count = count(1)?;
    if exact_count > ids.len() || prefix_count > ids.len() {
        return Err(MojoError::InvalidOutput);
    }
    let indices = |start: usize, count: usize| -> Result<Vec<usize>, MojoError> {
        let end = start.checked_add(count).ok_or(MojoError::InvalidOutput)?;
        let values = output.get(start..end).ok_or(MojoError::InvalidOutput)?;
        let mut previous = None;
        values
            .iter()
            .map(|value| {
                let index = usize::try_from(*value).map_err(|_| MojoError::InvalidOutput)?;
                if index >= ids.len() || previous.is_some_and(|previous| previous >= index) {
                    return Err(MojoError::InvalidOutput);
                }
                previous = Some(index);
                Ok(index)
            })
            .collect()
    };
    let exact = indices(2, exact_count)?;
    let prefix = indices(2 + ids.len(), prefix_count)?;
    if exact_count > prefix_count
        || exact
            .iter()
            .any(|index| prefix.binary_search(index).is_err())
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok((exact, prefix))
}

#[cfg(all(test, prodex_mojo_active))]
mod tests {
    use super::{
        session_file_name_plan, session_report_query_matches, session_selector_match_plan,
    };

    #[test]
    fn file_name_plan_matches_storage_suffix_contract() {
        let compressed = session_file_name_plan("session.jsonl.zst").unwrap();
        assert!(compressed.is_metadata);
        assert_eq!(compressed.stem_length, "session".len());

        let overlapping = session_file_name_plan("session.jsonl.zst.json").unwrap();
        assert!(overlapping.is_metadata);
        assert_eq!(
            &"session.jsonl.zst.json"[..overlapping.stem_length],
            "session.jsonl.zst"
        );

        let other = session_file_name_plan("session.txt").unwrap();
        assert!(!other.is_metadata);
        assert_eq!(other.stem_length, "session.txt".len());
    }

    #[test]
    fn match_plan_is_sensitive_to_selector_and_preserves_candidate_order() {
        let ids = [
            "11111111-1111-1111-1111-111111111111",
            "11111111-1111-1111-1111-222222222222",
        ];
        let exact = session_selector_match_plan(&ids, &ids[0].to_uppercase()).unwrap();
        assert_eq!(exact.0, [0]);
        assert_eq!(exact.1, [0]);

        let prefix = session_selector_match_plan(&ids, "11111111").unwrap();
        assert!(prefix.0.is_empty());
        assert_eq!(prefix.1, [0, 1]);

        let changed = session_selector_match_plan(&ids, "11111112").unwrap();
        assert!(changed.0.is_empty());
        assert!(changed.1.is_empty());
        assert_ne!(prefix.1, changed.1);
    }

    #[test]
    fn report_query_matches_normalized_unicode_fields_and_absent_values() {
        let fields = [
            Some("01900000-0000-7000-8000-000000000301"),
            Some("Issue Triage"),
            Some("/home/test-user/Workspace"),
            None,
            Some("OpenAI"),
            Some("/home/test-user/.codex/sessions/triage.jsonl"),
        ];
        assert!(session_report_query_matches("triage", &fields).unwrap());
        assert!(session_report_query_matches("WORKSPACE", &fields).unwrap());
        assert!(session_report_query_matches("openai", &fields).unwrap());
        assert!(!session_report_query_matches("missing", &fields).unwrap());
        assert!(session_report_query_matches("", &fields).unwrap());

        let unicode = [
            Some("straße"),
            Some("i\u{307}stanbul"),
            None,
            None,
            None,
            Some("/home/test-user/session"),
        ];
        assert!(session_report_query_matches("straße", &unicode).unwrap());
        assert!(session_report_query_matches("i\u{307}", &unicode).unwrap());
    }
}
