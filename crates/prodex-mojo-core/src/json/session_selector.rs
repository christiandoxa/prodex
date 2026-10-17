use super::{StringView, signed, status};
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
    fn prodex_session_selector_path_span_v1(
        abi_version: i64,
        stem_address: u64,
        stem_length: i64,
        selector_address: u64,
        selector_length: i64,
        exact: i64,
        require_full: i64,
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
    fn prodex_session_state_db_row_match_plan_v1(
        abi_version: i64,
        thread_id_address: u64,
        thread_id_length: i64,
        path_stem_address: u64,
        path_stem_length: i64,
        path_stem_present: i64,
        selector_address: u64,
        selector_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_recent_workspace_plan_v1(
        abi_version: i64,
        candidates_address: u64,
        candidates_count: i64,
        max_workspaces: i64,
        output_address: u64,
        output_capacity: i64,
        scratch_address: u64,
        scratch_capacity: i64,
    ) -> i64;
}

const SESSION_SELECTOR_ABI_VERSION: i64 = 1;
const SESSION_REPORT_QUERY_FIELD_COUNT: usize = 6;

#[repr(C)]
#[derive(Clone, Copy)]
struct SessionSelectorIdFfi {
    id: StringView,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SessionRecentWorkspaceCandidateFfi {
    modified_epoch: i64,
    absolute: i64,
    workspace: StringView,
}

const _: () = {
    assert!(std::mem::size_of::<SessionSelectorIdFfi>() == 16);
    assert!(std::mem::align_of::<SessionSelectorIdFfi>() == 8);
    assert!(std::mem::size_of::<SessionRecentWorkspaceCandidateFfi>() == 32);
    assert!(std::mem::align_of::<SessionRecentWorkspaceCandidateFfi>() == 8);
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

/// Find the first selector or full-ID span in a session filename stem.
///
/// Mojo owns whole-stem precedence and the five-component hyphen-window scan;
/// Rust validates and materializes the returned UTF-8 byte span.
pub fn session_selector_path_span(
    stem: &str,
    selector: &str,
    exact: bool,
    require_full: bool,
) -> Result<Option<(usize, usize)>, MojoError> {
    let mut output = [-1_i64; 2];
    let stem_value = stem;
    let stem = view(stem_value);
    let selector = view(selector);
    status(unsafe {
        prodex_session_selector_path_span_v1(
            SESSION_SELECTOR_ABI_VERSION,
            stem.address,
            i64::try_from(stem.length).map_err(|_| MojoError::InvalidInput)?,
            selector.address,
            i64::try_from(selector.length).map_err(|_| MojoError::InvalidInput)?,
            i64::from(exact),
            i64::from(require_full),
            output.as_mut_ptr() as u64,
        )
    })?;
    if output[0] == -1 && output[1] == -1 {
        return Ok(None);
    }
    let start = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    if start > end || end > stem_value.len() {
        return Err(MojoError::InvalidOutput);
    }
    if !stem_value.is_char_boundary(start) || !stem_value.is_char_boundary(end) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some((start, end)))
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

/// The deterministic selector result for one state-database thread row.
///
/// Rust keeps SQLite row acquisition and path/string materialization. Mojo owns
/// exact-over-prefix precedence across the raw thread ID, the `thread_`-stripped
/// ID, and the rollout filename stem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStateDbRowMatchKind {
    None,
    Prefix,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStateDbRowSessionIdSource {
    None,
    NormalizedThreadId,
    ThreadId,
    PathStem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStateDbRowMatchPlan {
    pub match_kind: SessionStateDbRowMatchKind,
    pub session_id_source: SessionStateDbRowSessionIdSource,
}

/// Filesystem facts for one recent-session workspace candidate.
///
/// The session-store caller owns path discovery and JSONL reads. Mojo owns the
/// deterministic newest-first, exact-deduplication, relative-path filtering,
/// and limit plan over these facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionRecentWorkspaceCandidate<'a> {
    pub modified_epoch: i64,
    pub absolute: bool,
    pub workspace: &'a str,
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

/// Plans state-database row matching without touching SQLite or filesystem state.
pub fn session_state_db_row_match_plan(
    thread_id: &str,
    path_stem: Option<&str>,
    selector: &str,
) -> Result<SessionStateDbRowMatchPlan, MojoError> {
    let thread_id = view(thread_id);
    let path_stem_present = path_stem.is_some();
    let path_stem_value = path_stem.unwrap_or("");
    let path_stem = view(path_stem_value);
    let selector = view(selector);
    let mut output = [-1_i64; 2];
    status(unsafe {
        prodex_session_state_db_row_match_plan_v1(
            SESSION_SELECTOR_ABI_VERSION,
            thread_id.address,
            i64::try_from(thread_id.length).map_err(|_| MojoError::InvalidInput)?,
            path_stem.address,
            i64::try_from(path_stem.length).map_err(|_| MojoError::InvalidInput)?,
            i64::from(path_stem_present),
            selector.address,
            i64::try_from(selector.length).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
        )
    })?;
    let match_kind = match output[0] {
        0 => SessionStateDbRowMatchKind::None,
        1 => SessionStateDbRowMatchKind::Prefix,
        2 => SessionStateDbRowMatchKind::Exact,
        _ => return Err(MojoError::InvalidOutput),
    };
    let session_id_source = match output[1] {
        0 => SessionStateDbRowSessionIdSource::None,
        1 => SessionStateDbRowSessionIdSource::NormalizedThreadId,
        2 => SessionStateDbRowSessionIdSource::ThreadId,
        3 => SessionStateDbRowSessionIdSource::PathStem,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(SessionStateDbRowMatchPlan {
        match_kind,
        session_id_source,
    })
}

/// Plans recent workspace candidates without filesystem or session-store access.
pub fn session_recent_workspace_plan(
    candidates: &[SessionRecentWorkspaceCandidate<'_>],
    max_workspaces: usize,
) -> Result<Vec<usize>, MojoError> {
    let candidate_count = signed(candidates.len())?;
    let selected_limit = max_workspaces.min(candidates.len());
    let selected_limit = signed(selected_limit)?;
    let output_capacity = selected_limit
        .checked_add(1)
        .ok_or(MojoError::InvalidInput)?;
    let ffi_candidates = candidates
        .iter()
        .map(|candidate| SessionRecentWorkspaceCandidateFfi {
            modified_epoch: candidate.modified_epoch,
            absolute: i64::from(candidate.absolute),
            workspace: view(candidate.workspace),
        })
        .collect::<Vec<_>>();
    let mut output =
        vec![-1_i64; usize::try_from(output_capacity).map_err(|_| MojoError::InvalidInput)?];
    let mut scratch = vec![-1_i64; candidates.len()];
    status(unsafe {
        prodex_session_recent_workspace_plan_v1(
            SESSION_SELECTOR_ABI_VERSION,
            ffi_candidates.as_ptr() as u64,
            candidate_count,
            selected_limit,
            output.as_mut_ptr() as u64,
            output_capacity,
            scratch.as_mut_ptr() as u64,
            candidate_count,
        )
    })?;
    let selected_count = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if selected_count > selected_limit as usize {
        return Err(MojoError::InvalidOutput);
    }
    let mut selected = Vec::with_capacity(selected_count);
    for value in output.iter().skip(1).take(selected_count) {
        let index = usize::try_from(*value).map_err(|_| MojoError::InvalidOutput)?;
        if index >= candidates.len() || selected.contains(&index) {
            return Err(MojoError::InvalidOutput);
        }
        selected.push(index);
    }
    Ok(selected)
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
        SessionStateDbRowMatchKind, SessionStateDbRowSessionIdSource, session_file_name_plan,
        session_report_query_matches, session_selector_match_plan, session_selector_path_span,
        session_state_db_row_match_plan,
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
    fn path_span_preserves_window_order_empty_components_and_full_ids() {
        let stem = "é--prefix-a-b-c-d";
        assert_eq!(
            session_selector_path_span(stem, "-prefix-a-b", false, false).unwrap(),
            Some((3, 16))
        );
        assert_eq!(
            session_selector_path_span("prefix-a-b-c-d-e", "prefix", false, false).unwrap(),
            Some((0, "prefix-a-b-c-d-e".len()))
        );
        let id = "01900000-0000-7000-8000-000000000301";
        assert_eq!(
            session_selector_path_span(id, "", true, true).unwrap(),
            Some((0, 36))
        );
        assert_eq!(
            session_selector_path_span(&format!("prefix-{id}-suffix"), "", true, true).unwrap(),
            Some((7, 43))
        );
    }

    #[test]
    fn state_db_row_plan_preserves_exact_precedence_and_id_source() {
        let id = "01900000-0000-7000-8000-000000000301";
        assert_eq!(
            session_state_db_row_match_plan(
                &format!("thread_{id}"),
                Some(&format!("rollout-{id}")),
                "01900000",
            )
            .unwrap(),
            super::SessionStateDbRowMatchPlan {
                match_kind: SessionStateDbRowMatchKind::Prefix,
                session_id_source: SessionStateDbRowSessionIdSource::NormalizedThreadId,
            }
        );
        assert_eq!(
            session_state_db_row_match_plan(id, Some("rollout-unrelated"), id)
                .unwrap()
                .match_kind,
            SessionStateDbRowMatchKind::Exact
        );
        assert_eq!(
            session_state_db_row_match_plan("unrelated", Some(&format!("rollout-{id}")), id)
                .unwrap()
                .session_id_source,
            SessionStateDbRowSessionIdSource::PathStem
        );
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
