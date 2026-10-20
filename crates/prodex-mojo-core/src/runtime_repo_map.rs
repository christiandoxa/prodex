use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 512;

unsafe extern "C" {
    fn prodex_runtime_repo_duplicate_groups_v1(
        abi_version: i64,
        rows_address: u64,
        count: i64,
        max_groups: i64,
        max_occurrences: i64,
        indices_address: u64,
        indices_capacity: i64,
        groups_address: u64,
        groups_capacity: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_runtime_repo_map_path_set_v1(
        abi_version: i64,
        rows_address: u64,
        count: i64,
        output_address: u64,
        output_capacity: i64,
        selected_count_address: u64,
    ) -> i64;
    fn prodex_runtime_repo_nearest_path_v1(
        abi_version: i64,
        rows_address: u64,
        row_count: i64,
        target_line: i64,
        result_address: u64,
    ) -> i64;
    fn prodex_runtime_repo_artifact_eviction_v1(
        abi_version: i64,
        rows_address: u64,
        count: i64,
        max_count: i64,
        total_bytes: u64,
        max_bytes: u64,
        scratch_address: u64,
        scratch_capacity: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_runtime_repo_artifact_order_v1(
        abi_version: i64,
        rows_address: u64,
        count: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_runtime_repo_chunk_plan_v1(
        abi_version: i64,
        current_count: i64,
        max_count: i64,
        byte_len: i64,
        text_len: i64,
        hash_matches: i64,
    ) -> i64;
    fn prodex_runtime_repo_path_distance_v1(
        abi_version: i64,
        start: i64,
        end: i64,
        line: i64,
    ) -> i64;
    fn prodex_runtime_repo_symbol_kind_v1(
        abi_version: i64,
        label_address: u64,
        label_length: i64,
        text_address: u64,
        text_length: i64,
    ) -> i64;
    fn prodex_runtime_repo_entry_replace_v1(
        abi_version: i64,
        incoming_order: u64,
        current_order: u64,
        incoming_id_address: u64,
        incoming_id_length: i64,
        current_id_address: u64,
        current_id_length: i64,
    ) -> i64;
    fn prodex_runtime_repo_module_path_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Borrowed artifact identity and source ordering facts. The caller retains
/// artifact ownership; the compiled repo-map kernel alone chooses priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoArtifactOrderInput<'a> {
    pub order: u64,
    pub id: &'a str,
}

/// Newest artifact order first, bytewise ID ascending on equal order, and
/// stable original index for exact equality. No per-compare Rust fallback.
pub fn repo_artifact_order(inputs: &[RepoArtifactOrderInput<'_>]) -> Result<Vec<usize>, MojoError> {
    if inputs.len() > 65_536 {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<u64>::new();
    rows.try_reserve_exact(inputs.len().checked_mul(3).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for input in inputs {
        rows.extend([
            input.order,
            input.id.as_ptr() as usize as u64,
            u64::try_from(input.id.len()).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let mut output = vec![-1_i64; inputs.len()];
    match unsafe {
        prodex_runtime_repo_artifact_order_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let mut seen = vec![false; inputs.len()];
    let mut indices = Vec::with_capacity(inputs.len());
    for value in output {
        let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
        let slot = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
        if *slot {
            return Err(MojoError::InvalidOutput);
        }
        *slot = true;
        indices.push(index);
    }
    Ok(indices)
}

/// Original persisted artifact and tracked size. The host only supplies
/// borrowed facts and later removes the Mojo-selected original records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoArtifactEvictionInput<'a> {
    pub order: u64,
    pub id: &'a str,
    pub byte_len: usize,
}

/// The compiled repo-map owner selects the oldest artifacts to evict until
/// both the size and count limits are satisfied. No Rust min_by policy.
pub fn repo_artifact_eviction_plan(
    inputs: &[RepoArtifactEvictionInput<'_>],
    total_bytes: usize,
    max_count: usize,
    max_bytes: usize,
) -> Result<Vec<usize>, MojoError> {
    if inputs.len() > 1_048_576 {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<u64>::new();
    rows.try_reserve_exact(inputs.len().checked_mul(4).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for input in inputs {
        rows.extend([
            input.order,
            input.id.as_ptr() as usize as u64,
            u64::try_from(input.id.len()).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(input.byte_len).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let mut scratch = vec![-1_i64; inputs.len()];
    let mut output = vec![-1_i64; inputs.len()];
    let mut written = -1_i64;
    match unsafe {
        prodex_runtime_repo_artifact_eviction_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_count).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(total_bytes).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
            scratch.as_mut_ptr() as usize as u64,
            i64::try_from(scratch.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > inputs.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; inputs.len()];
    let mut indices = Vec::with_capacity(count);
    for value in output.into_iter().take(count) {
        let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
        let used = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
        if *used {
            return Err(MojoError::InvalidOutput);
        }
        *used = true;
        indices.push(index);
    }
    Ok(indices)
}

/// Borrowed source-range geometry. Path ownership stays with the host
/// while compiled Mojo selects the nearest valid original record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoNearestPathInput {
    pub start: usize,
    pub end: usize,
    pub has_path: bool,
}

/// Select the smallest compiled path-distance, then earliest range start,
/// then first encountered original candidate on an exact tie.
pub fn repo_nearest_path_index(
    candidates: &[RepoNearestPathInput],
    target_line: usize,
) -> Result<Option<usize>, MojoError> {
    if candidates.len() > 1_048_576 {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<i64>::new();
    rows.try_reserve_exact(candidates.len().checked_mul(3).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for candidate in candidates {
        let (start, end) = if candidate.has_path {
            (
                i64::try_from(candidate.start).map_err(|_| MojoError::InvalidInput)?,
                i64::try_from(candidate.end).map_err(|_| MojoError::InvalidInput)?,
            )
        } else {
            (0, 0)
        };
        rows.extend([start, end, i64::from(candidate.has_path)]);
    }
    let mut result = -7_i64;
    match unsafe {
        prodex_runtime_repo_nearest_path_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(candidates.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(target_line).map_err(|_| MojoError::InvalidInput)?,
            (&mut result as *mut i64) as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    if result == -1 {
        return Ok(None);
    }
    let index = usize::try_from(result).map_err(|_| MojoError::InvalidOutput)?;
    if !candidates.get(index).is_some_and(|row| row.has_path) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(index))
}

/// Choose the original indices of sorted unique repository-map paths,
/// excluding absent and Rust-trim-empty strings entirely in compiled Mojo.
/// The caller retains the original optional String references.
pub fn repo_map_unique_path_indices(paths: &[Option<&str>]) -> Result<Vec<usize>, MojoError> {
    if paths.len() > 1_048_576 {
        return Err(MojoError::InvalidInput);
    }
    let mut fields = Vec::<u64>::new();
    fields
        .try_reserve_exact(paths.len().checked_mul(3).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for path in paths {
        match path {
            Some(path) => fields.extend([
                1,
                path.as_ptr() as usize as u64,
                u64::try_from(path.len()).map_err(|_| MojoError::InvalidInput)?,
            ]),
            None => fields.extend([0, 0, 0]),
        }
    }
    let mut indices = vec![-1_i64; paths.len()];
    let mut selected = -1_i64;
    match unsafe {
        prodex_runtime_repo_map_path_set_v1(
            ABI_VERSION,
            fields.as_ptr() as usize as u64,
            i64::try_from(paths.len()).map_err(|_| MojoError::InvalidInput)?,
            indices.as_mut_ptr() as usize as u64,
            i64::try_from(indices.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut selected as *mut i64) as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(selected).map_err(|_| MojoError::InvalidOutput)?;
    if count > paths.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; paths.len()];
    indices
        .into_iter()
        .take(count)
        .map(|value| {
            let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
            let slot = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
            if *slot || paths[index].is_none() {
                return Err(MojoError::InvalidOutput);
            }
            *slot = true;
            Ok(index)
        })
        .collect()
}

/// Original semantic chunk metadata used only to group identical hashes
/// and byte lengths; compiled Mojo owns grouping and retention limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepoDuplicateChunkInput<'a> {
    pub hash: &'a str,
    pub byte_len: usize,
}

/// Source indices for the original first N occurrences in one selected
/// sorted fingerprint group, preserving original chunk traversal order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoDuplicateChunkGroup {
    pub first_source: usize,
    pub occurrence_count: usize,
    pub occurrence_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoDuplicateChunkGroupsPlan {
    pub groups: Vec<RepoDuplicateChunkGroup>,
    pub complete: bool,
}

/// Sort and group all fingerprint observations in one compiled call.
///
/// This replaces per-group Rust BTreeMap construction, the Rust duplicate
/// selection loop, and per-group semantic FFI transitions.
pub fn repo_duplicate_chunk_groups(
    chunks: &[RepoDuplicateChunkInput<'_>],
    max_groups: usize,
    max_occurrences: usize,
) -> Result<RepoDuplicateChunkGroupsPlan, MojoError> {
    if chunks.len() > 65_536 || max_groups > 65_536 || max_occurrences > 65_536 {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<u64>::new();
    rows.try_reserve_exact(chunks.len().checked_mul(3).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for chunk in chunks {
        rows.extend([
            chunk.hash.as_ptr() as usize as u64,
            u64::try_from(chunk.hash.len()).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(chunk.byte_len).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let mut sorted = vec![-1_i64; chunks.len()];
    let mut groups = Vec::<i64>::new();
    groups
        .try_reserve_exact(max_groups.checked_mul(3).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    groups.resize(max_groups * 3, -1);
    let mut result = [-1_i64; 2];
    match unsafe {
        prodex_runtime_repo_duplicate_groups_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(chunks.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_groups).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_occurrences).map_err(|_| MojoError::InvalidInput)?,
            sorted.as_mut_ptr() as usize as u64,
            i64::try_from(sorted.len()).map_err(|_| MojoError::InvalidInput)?,
            groups.as_mut_ptr() as usize as u64,
            i64::try_from(groups.len()).map_err(|_| MojoError::InvalidInput)?,
            result.as_mut_ptr() as usize as u64,
        )
    } {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let group_count = usize::try_from(result[0]).map_err(|_| MojoError::InvalidOutput)?;
    if group_count > max_groups || group_count > chunks.len() {
        return Err(MojoError::InvalidOutput);
    }
    let complete = match result[1] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut ordered = Vec::with_capacity(sorted.len());
    let mut seen = vec![false; chunks.len()];
    for value in sorted {
        let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
        let used = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
        if *used {
            return Err(MojoError::InvalidOutput);
        }
        *used = true;
        ordered.push(index);
    }
    let mut selected = Vec::with_capacity(group_count);
    let mut prior_end = 0usize;
    for raw in groups.chunks_exact(3).take(group_count) {
        let start = usize::try_from(raw[0]).map_err(|_| MojoError::InvalidOutput)?;
        let occurrences = usize::try_from(raw[1]).map_err(|_| MojoError::InvalidOutput)?;
        let kept = usize::try_from(raw[2]).map_err(|_| MojoError::InvalidOutput)?;
        let end = start
            .checked_add(occurrences)
            .ok_or(MojoError::InvalidOutput)?;
        if start < prior_end
            || end > ordered.len()
            || occurrences < 2
            || kept > occurrences
            || kept > max_occurrences
        {
            return Err(MojoError::InvalidOutput);
        }
        prior_end = end;
        selected.push(RepoDuplicateChunkGroup {
            first_source: ordered[start],
            occurrence_count: occurrences,
            occurrence_indices: ordered[start..start + kept].to_vec(),
        });
    }
    Ok(RepoDuplicateChunkGroupsPlan {
        groups: selected,
        complete,
    })
}

fn input_len(value: &str) -> Result<i64, MojoError> {
    if value.len() > MAX_INPUT_BYTES {
        return Err(MojoError::InvalidInput);
    }
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoChunkPlan {
    Accept,
    RejectCapacity,
    RejectInvalid,
}

pub fn repo_chunk_plan(
    current_count: usize,
    max_count: usize,
    byte_len: usize,
    text_len: usize,
    hash_matches: bool,
) -> Result<RepoChunkPlan, MojoError> {
    let result = unsafe {
        prodex_runtime_repo_chunk_plan_v1(
            ABI_VERSION,
            i64::try_from(current_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(byte_len).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(text_len).map_err(|_| MojoError::InvalidInput)?,
            i64::from(hash_matches),
        )
    };
    match result {
        0 => Ok(RepoChunkPlan::Accept),
        1 => Ok(RepoChunkPlan::RejectCapacity),
        2 => Ok(RepoChunkPlan::RejectInvalid),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn repo_path_distance(start: usize, end: usize, line: usize) -> Result<usize, MojoError> {
    let result = unsafe {
        prodex_runtime_repo_path_distance_v1(
            ABI_VERSION,
            i64::try_from(start).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(end).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(line).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    if result < 0 {
        return Err(MojoError::InvalidInput);
    }
    usize::try_from(result).map_err(|_| MojoError::InvalidOutput)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoSymbolKind {
    Module,
    Symbol,
    Test,
}

pub fn repo_symbol_kind(label: Option<&str>, text: &str) -> Result<RepoSymbolKind, MojoError> {
    let label = label.unwrap_or_default();
    let result = unsafe {
        prodex_runtime_repo_symbol_kind_v1(
            ABI_VERSION,
            label.as_ptr() as usize as u64,
            input_len(label)?,
            text.as_ptr() as usize as u64,
            input_len(text)?,
        )
    };
    match result {
        1 => Ok(RepoSymbolKind::Module),
        2 => Ok(RepoSymbolKind::Symbol),
        3 => Ok(RepoSymbolKind::Test),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn repo_entry_should_replace(
    incoming_order: u64,
    current_order: u64,
    incoming_id: &str,
    current_id: &str,
) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_runtime_repo_entry_replace_v1(
            ABI_VERSION,
            incoming_order,
            current_order,
            incoming_id.as_ptr() as usize as u64,
            input_len(incoming_id)?,
            current_id.as_ptr() as usize as u64,
            input_len(current_id)?,
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn repo_module_from_path(path: &str) -> Result<Option<String>, MojoError> {
    let mut output = vec![0_u8; MAX_OUTPUT_BYTES];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_runtime_repo_module_path_v1(
            ABI_VERSION,
            path.as_ptr() as usize as u64,
            input_len(path)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    if status == 3 {
        return Ok(None);
    }
    if status != 0 {
        return Err(if status == 1 {
            MojoError::InvalidInput
        } else {
            MojoError::InvalidOutput
        });
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    if bytes.is_empty() {
        return Ok(None);
    }
    String::from_utf8(bytes.to_vec())
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_map_kernel_matches_module_helpers() {
        assert_eq!(
            repo_chunk_plan(0, 4, 10, 10, true).unwrap(),
            RepoChunkPlan::Accept
        );
        assert_eq!(
            repo_chunk_plan(4, 4, 10, 10, true).unwrap(),
            RepoChunkPlan::RejectCapacity
        );
        assert_eq!(
            repo_chunk_plan(0, 4, 10, 9, true).unwrap(),
            RepoChunkPlan::RejectInvalid
        );
        assert_eq!(repo_path_distance(10, 20, 15).unwrap(), 0);
        assert_eq!(repo_path_distance(10, 20, 25).unwrap(), 5);
        assert_eq!(
            repo_symbol_kind(Some("test_symbol"), "fn t() {}").unwrap(),
            RepoSymbolKind::Test
        );
        assert_eq!(
            repo_symbol_kind(Some("symbol"), "pub mod alpha {").unwrap(),
            RepoSymbolKind::Module
        );
        assert!(repo_entry_should_replace(2, 1, "b", "a").unwrap());
        assert!(repo_entry_should_replace(1, 1, "a", "b").unwrap());

        for (path, expected) in [
            ("src/foo/mod.rs", Some("foo".to_string())),
            ("src/lib.rs", Some("lib".to_string())),
            ("a/src/api/client.ts", Some("api::client".to_string())),
            ("b/app/index.js", Some("app".to_string())),
            ("./pkg/main.py", Some("pkg".to_string())),
            ("src\\api\\mod.rs", Some("api".to_string())),
            ("\"a/src/foo.rs\"", Some("a::src::foo".to_string())),
            ("dir.name/file", Some("dir".to_string())),
            (".", None),
            ("", None),
        ] {
            assert_eq!(repo_module_from_path(path).unwrap(), expected, "{path:?}");
        }
    }
}

#[cfg(test)]
mod compiled_artifact_order_tests {
    use super::*;

    #[test]
    fn compiled_artifact_order_prioritizes_latest_then_id_with_stable_duplicates() {
        let inputs = [
            RepoArtifactOrderInput {
                order: 1,
                id: "last",
            },
            RepoArtifactOrderInput {
                order: 10,
                id: "雪",
            },
            RepoArtifactOrderInput { order: 10, id: "a" },
            RepoArtifactOrderInput {
                order: 10, id: "é"
            },
            RepoArtifactOrderInput { order: 10, id: "a" },
            RepoArtifactOrderInput { order: 0, id: "" },
        ];
        assert_eq!(repo_artifact_order(&inputs), Ok(vec![2, 4, 3, 1, 0, 5]));
        assert_eq!(repo_artifact_order(&[]), Ok(vec![]));
        let lots = (0..1025)
            .map(|index| RepoArtifactOrderInput {
                order: (index % 10) as u64,
                id: if index % 2 == 0 { "a" } else { "b" },
            })
            .collect::<Vec<_>>();
        let selected = repo_artifact_order(&lots).unwrap();
        assert_eq!(selected.len(), lots.len());
        assert!(selected.windows(2).all(|window| {
            let left = lots[window[0]];
            let right = lots[window[1]];
            left.order > right.order
                || left.order == right.order
                    && (left.id < right.id || left.id == right.id && window[0] < window[1])
        }));
    }

    #[test]
    fn compiled_artifact_order_abi_rejects_malformed_late_rows_without_output() {
        let mut rows = [0_u64; 6];
        let mut output = [79_i64; 2];
        let source = rows.as_ptr() as usize as u64;
        let dest = output.as_mut_ptr() as usize as u64;
        for (abi, ptr, count, capacity, expected) in [
            (2, source, 2, 2, 4),
            (1, 0, 2, 2, 1),
            (1, source, -1, 2, 1),
            (1, source, 65_537, 2, 1),
            (1, source, 2, 1, 1),
        ] {
            assert_eq!(
                unsafe { prodex_runtime_repo_artifact_order_v1(abi, ptr, count, dest, capacity) },
                expected,
            );
            assert_eq!(output, [79; 2]);
        }
        rows[5] = 2;
        assert_eq!(
            unsafe {
                prodex_runtime_repo_artifact_order_v1(1, rows.as_ptr() as usize as u64, 2, dest, 2)
            },
            1,
        );
        assert_eq!(output, [79; 2]);
        assert_eq!(
            unsafe { prodex_runtime_repo_artifact_order_v1(1, 0, 0, dest, 2) },
            0,
        );
        assert_eq!(output, [79; 2]);
    }
}

#[cfg(test)]
mod compiled_artifact_eviction_tests {
    use super::*;

    fn item<'a>(order: u64, id: &'a str, byte_len: usize) -> RepoArtifactEvictionInput<'a> {
        RepoArtifactEvictionInput {
            order,
            id,
            byte_len,
        }
    }

    #[test]
    fn compiled_artifact_eviction_selects_oldest_then_id_until_both_limits_hold() {
        let inputs = [
            item(1, "z", 4),
            item(1, "a", 3),
            item(2, "newer", 10),
            item(3, "latest", 1),
        ];
        assert_eq!(
            repo_artifact_eviction_plan(&inputs, 18, 2, 12),
            Ok(vec![1, 0])
        );
        assert_eq!(
            repo_artifact_eviction_plan(&inputs, 18, 4, 11),
            Ok(vec![1, 0])
        );
        assert_eq!(
            repo_artifact_eviction_plan(&inputs, 18, 2, 18),
            Ok(vec![1, 0])
        );
        assert_eq!(repo_artifact_eviction_plan(&inputs, 18, 4, 18), Ok(vec![]));
        assert_eq!(
            repo_artifact_eviction_plan(&inputs, 18, 4, 0),
            Ok(vec![1, 0, 2, 3])
        );
        assert_eq!(repo_artifact_eviction_plan(&[], 0, 0, 0), Ok(vec![]));
        assert_eq!(
            repo_artifact_eviction_plan(&[item(0, "x", usize::MAX)], 2, 0, 0),
            Ok(vec![0]),
            "total_bytes saturates to zero after evicting an oversized artifact",
        );
    }

    #[test]
    fn compiled_artifact_eviction_handles_unicode_equal_order_and_large_batches() {
        let unicode = [
            item(4, "雪", 1),
            item(4, "é", 1),
            item(4, "a", 1),
            item(4, "a", 1),
        ];
        assert_eq!(
            repo_artifact_eviction_plan(&unicode, 4, 1, 4),
            Ok(vec![2, 3, 1])
        );
        let many = (0..1025)
            .map(|idx| item(idx as u64, "id", 1))
            .collect::<Vec<_>>();
        let chosen = repo_artifact_eviction_plan(&many, 1025, 1000, 1000).unwrap();
        assert_eq!(chosen, (0..25).collect::<Vec<_>>());
        assert_eq!(
            repo_artifact_eviction_plan(&many, 1025, 1025, 1025),
            Ok(vec![]),
        );
    }

    #[test]
    fn compiled_artifact_eviction_abi_rejects_late_bad_input_without_partial_output() {
        let mut rows = [0_u64; 8];
        let mut scratch = [71_i64; 2];
        let mut output = [73_i64; 2];
        let mut written = 91_i64;
        let source = rows.as_ptr() as usize as u64;
        let work = scratch.as_mut_ptr() as usize as u64;
        let dest = output.as_mut_ptr() as usize as u64;
        let result = (&mut written as *mut i64) as usize as u64;
        for (abi, address, count, max_count, cap, out, expected) in [
            (2, source, 2, 1, 2, result, 4),
            (1, 0, 2, 1, 2, result, 1),
            (1, source, -1, 1, 2, result, 1),
            (1, source, 1_048_577, 1, 2, result, 1),
            (1, source, 2, -1, 2, result, 1),
            (1, source, 2, 1, 1, result, 1),
            (1, source, 2, 1, 2, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_runtime_repo_artifact_eviction_v1(
                        abi, address, count, max_count, 2, 1, work, 2, dest, cap, out,
                    )
                },
                expected,
            );
            assert_eq!(scratch, [71; 2]);
            assert_eq!(output, [73; 2]);
            assert_eq!(written, 91);
        }
        rows[6] = 1;
        assert_eq!(
            unsafe {
                prodex_runtime_repo_artifact_eviction_v1(
                    1,
                    rows.as_ptr() as usize as u64,
                    2,
                    1,
                    2,
                    1,
                    work,
                    2,
                    dest,
                    2,
                    result,
                )
            },
            1,
        );
        assert_eq!(scratch, [71; 2]);
        assert_eq!(output, [73; 2]);
        assert_eq!(written, 91);
    }
}

#[cfg(test)]
mod compiled_nearest_path_tests {
    use super::*;

    fn range(start: usize, end: usize, has_path: bool) -> RepoNearestPathInput {
        RepoNearestPathInput {
            start,
            end,
            has_path,
        }
    }

    #[test]
    fn compiled_repo_nearest_path_uses_distance_then_start_then_first_index() {
        let cases = [
            range(1, 2, true),
            range(4, 5, true),
            range(2, 100, false),
            range(4, 5, true),
            range(1, 2, true),
        ];
        assert_eq!(repo_nearest_path_index(&cases, 3), Ok(Some(0)));
        assert_eq!(repo_nearest_path_index(&cases, 5), Ok(Some(1)));
        assert_eq!(repo_nearest_path_index(&cases, 99), Ok(Some(1)));
        assert_eq!(repo_nearest_path_index(&cases, 0), Ok(Some(0)));
        assert_eq!(repo_nearest_path_index(&cases, 4), Ok(Some(1)));
        assert_eq!(repo_nearest_path_index(&[], 4), Ok(None));
        assert_eq!(
            repo_nearest_path_index(&[range(1, 2, false), range(20, 30, false)], 9),
            Ok(None),
        );
        let same = [range(10, 20, true), range(10, 20, true)];
        assert_eq!(repo_nearest_path_index(&same, 12), Ok(Some(0)));
        let large = (0..1025_usize)
            .map(|start| range(start * 2, start * 2 + 1, true))
            .collect::<Vec<_>>();
        assert_eq!(repo_nearest_path_index(&large, 2048), Ok(Some(1024)));
    }

    #[test]
    fn compiled_repo_nearest_path_abi_rejects_invalid_late_row_without_writes() {
        let mut rows = [0_i64; 6];
        rows[2] = 1;
        let mut output = 93_i64;
        let source = rows.as_ptr() as usize as u64;
        let result = (&mut output as *mut i64) as usize as u64;
        for (abi, ptr, count, line, dest, expected) in [
            (2, source, 2, 3, result, 4),
            (1, 0, 2, 3, result, 1),
            (1, source, -1, 3, result, 1),
            (1, source, 1_048_577, 3, result, 1),
            (1, source, 2, -1, result, 1),
            (1, source, 2, 3, 0, 1),
        ] {
            assert_eq!(
                unsafe { prodex_runtime_repo_nearest_path_v1(abi, ptr, count, line, dest) },
                expected,
            );
            assert_eq!(output, 93);
        }
        rows[5] = 2;
        assert_eq!(
            unsafe {
                prodex_runtime_repo_nearest_path_v1(1, rows.as_ptr() as usize as u64, 2, 3, result)
            },
            1,
        );
        assert_eq!(output, 93);
        rows[5] = 1;
        rows[3] = 30;
        rows[4] = 20;
        assert_eq!(
            unsafe {
                prodex_runtime_repo_nearest_path_v1(1, rows.as_ptr() as usize as u64, 2, 3, result)
            },
            1,
        );
        assert_eq!(output, 93);
        assert_eq!(
            unsafe { prodex_runtime_repo_nearest_path_v1(1, 0, 0, 3, result) },
            0,
        );
        assert_eq!(output, -1);
    }
}

#[cfg(test)]
mod compiled_repo_map_path_set_tests {
    use super::*;

    #[test]
    fn compiled_repo_map_path_set_filters_rust_unicode_whitespace_and_deduplicates_originals() {
        let paths = [
            Some("src/雪.rs"),
            None,
            Some("\u{2003} \t"),
            Some("src/a.rs"),
            Some("src/雪.rs"),
            Some(""),
            Some(" src/a.rs "),
            Some("\u{3000}"),
            Some("src/é.rs"),
            Some("src/a.rs"),
            Some("src/🦀.rs"),
        ];
        let indices = repo_map_unique_path_indices(&paths).unwrap();
        assert_eq!(
            indices
                .iter()
                .map(|index| paths[*index].unwrap())
                .collect::<Vec<_>>(),
            [
                " src/a.rs ",
                "src/a.rs",
                "src/é.rs",
                "src/雪.rs",
                "src/🦀.rs",
            ]
        );
        assert_eq!(indices, [6, 3, 8, 0, 10]);
        assert_eq!(repo_map_unique_path_indices(&[]), Ok(Vec::new()));
        assert_eq!(
            repo_map_unique_path_indices(&[None, Some(" "), Some("")]),
            Ok(Vec::new())
        );
        assert_eq!(
            repo_map_unique_path_indices(&[Some("a"), Some("a")]),
            Ok(vec![0])
        );

        let strings = (0..1025)
            .rev()
            .map(|i| format!("src/{i:04}.rs"))
            .collect::<Vec<_>>();
        let borrowed = strings.iter().map(|s| Some(s.as_str())).collect::<Vec<_>>();
        let indices = repo_map_unique_path_indices(&borrowed).unwrap();
        assert_eq!(indices.len(), 1025);
        assert_eq!(indices[0], 1024);
        assert_eq!(indices[1024], 0);
    }

    #[test]
    fn compiled_repo_map_path_set_rejects_late_bad_input_without_writing() {
        let mut rows = [0_u64; 6];
        let mut output = [73_i64; 2];
        let mut selected = 91_i64;
        let input = rows.as_ptr() as usize as u64;
        let destination = output.as_mut_ptr() as usize as u64;
        let selected_ptr = (&mut selected as *mut i64) as usize as u64;
        for (abi, source, count, capacity, answer, expected) in [
            (2, input, 2, 2, selected_ptr, 4),
            (1, 0, 2, 2, selected_ptr, 1),
            (1, input, -1, 2, selected_ptr, 1),
            (1, input, 1_048_577, 2, selected_ptr, 1),
            (1, input, 2, 1, selected_ptr, 1),
            (1, input, 2, 2, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_runtime_repo_map_path_set_v1(
                        abi,
                        source,
                        count,
                        destination,
                        capacity,
                        answer,
                    )
                },
                expected,
            );
            assert_eq!(output, [73; 2]);
            assert_eq!(selected, 91);
        }
        rows[3] = 2;
        assert_eq!(
            unsafe {
                prodex_runtime_repo_map_path_set_v1(
                    1,
                    rows.as_ptr() as usize as u64,
                    2,
                    destination,
                    2,
                    selected_ptr,
                )
            },
            1,
        );
        assert_eq!(output, [73; 2]);
        assert_eq!(selected, 91);
        rows[3] = 1;
        rows[5] = 1;
        assert_eq!(
            unsafe {
                prodex_runtime_repo_map_path_set_v1(
                    1,
                    rows.as_ptr() as usize as u64,
                    2,
                    destination,
                    2,
                    selected_ptr,
                )
            },
            1,
            "present path with nonempty length and no address is malformed",
        );
        assert_eq!(output, [73; 2]);
        assert_eq!(selected, 91);
    }
}

#[cfg(test)]
mod compiled_duplicate_chunk_group_tests {
    use super::*;

    fn record<'a>(hash: &'a str, byte_len: usize) -> RepoDuplicateChunkInput<'a> {
        RepoDuplicateChunkInput { hash, byte_len }
    }

    #[test]
    fn compiled_duplicate_chunk_groups_sort_hash_and_size_with_source_order() {
        let inputs = [
            record("hashB", 3),
            record("hashA", 2),
            record("hashB", 3),
            record("hashA", 2),
            record("hashA", 5),
            record("hashA", 5),
            record("lonely", 2),
            record("hashB", 3),
        ];
        let plan = repo_duplicate_chunk_groups(&inputs, 4, 3).unwrap();
        assert_eq!(
            plan,
            RepoDuplicateChunkGroupsPlan {
                groups: vec![
                    RepoDuplicateChunkGroup {
                        first_source: 1,
                        occurrence_count: 2,
                        occurrence_indices: vec![1, 3],
                    },
                    RepoDuplicateChunkGroup {
                        first_source: 4,
                        occurrence_count: 2,
                        occurrence_indices: vec![4, 5],
                    },
                    RepoDuplicateChunkGroup {
                        first_source: 0,
                        occurrence_count: 3,
                        occurrence_indices: vec![0, 2, 7],
                    },
                ],
                complete: true,
            }
        );
        let limited = repo_duplicate_chunk_groups(&inputs, 2, 3).unwrap();
        assert_eq!(limited.groups.len(), 2);
        assert!(!limited.complete);
        let truncated = repo_duplicate_chunk_groups(&inputs, 4, 2).unwrap();
        assert_eq!(truncated.groups[2].occurrence_indices, vec![0, 2]);
        assert_eq!(truncated.groups[2].occurrence_count, 3);
        assert!(!truncated.complete);
        assert_eq!(
            repo_duplicate_chunk_groups(&[record("solo", 1)], 0, 0),
            Ok(RepoDuplicateChunkGroupsPlan {
                groups: vec![],
                complete: true,
            }),
        );
        assert_eq!(
            repo_duplicate_chunk_groups(&inputs, 0, 8).unwrap().complete,
            false,
        );
        assert_eq!(
            repo_duplicate_chunk_groups(&[], 64, 8),
            Ok(RepoDuplicateChunkGroupsPlan {
                groups: vec![],
                complete: true,
            }),
        );
    }

    #[test]
    fn compiled_duplicate_chunk_groups_enforce_64_group_and_8_occurrence_limits() {
        let names = (0..65)
            .map(|index| format!("{index:04}"))
            .collect::<Vec<_>>();
        let mut cases = Vec::new();
        for name in &names {
            cases.push(record(name, 1));
            cases.push(record(name, 1));
        }
        let limit = repo_duplicate_chunk_groups(&cases, 64, 8).unwrap();
        assert_eq!(limit.groups.len(), 64);
        assert!(!limit.complete);
        assert_eq!(limit.groups[0].occurrence_indices, vec![0, 1]);
        assert_eq!(limit.groups[63].occurrence_indices, vec![126, 127]);
        assert_eq!(
            repo_duplicate_chunk_groups(&cases, 65, 8)
                .unwrap()
                .groups
                .len(),
            65
        );
        let repeated = vec![record("repeat", 1); 9];
        let plan = repo_duplicate_chunk_groups(&repeated, 64, 8).unwrap();
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.groups[0].occurrence_count, 9);
        assert_eq!(
            plan.groups[0].occurrence_indices,
            (0..8).collect::<Vec<_>>()
        );
        assert!(!plan.complete);
    }

    #[test]
    fn compiled_duplicate_chunk_groups_abi_rejects_bad_late_rows_without_writes() {
        let mut rows = [0_u64; 6];
        let mut sorted = [73_i64; 2];
        let mut groups = [79_i64; 6];
        let mut result = [91_i64; 2];
        let source = rows.as_ptr() as usize as u64;
        let out = sorted.as_mut_ptr() as usize as u64;
        let group_ptr = groups.as_mut_ptr() as usize as u64;
        let result_ptr = result.as_mut_ptr() as usize as u64;
        for (
            version,
            input,
            count,
            max_group,
            max_occ,
            idx_cap,
            group_cap,
            result_addr,
            expected,
        ) in [
            (2, source, 2, 2, 3, 2, 6, result_ptr, 4),
            (1, 0, 2, 2, 3, 2, 6, result_ptr, 1),
            (1, source, -1, 2, 3, 2, 6, result_ptr, 1),
            (1, source, 65_537, 2, 3, 2, 6, result_ptr, 1),
            (1, source, 2, -1, 3, 2, 6, result_ptr, 1),
            (1, source, 2, 2, -1, 2, 6, result_ptr, 1),
            (1, source, 2, 2, 3, 1, 6, result_ptr, 1),
            (1, source, 2, 2, 3, 2, 5, result_ptr, 1),
            (1, source, 2, 2, 3, 2, 6, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_runtime_repo_duplicate_groups_v1(
                        version,
                        input,
                        count,
                        max_group,
                        max_occ,
                        out,
                        idx_cap,
                        group_ptr,
                        group_cap,
                        result_addr,
                    )
                },
                expected,
            );
            assert_eq!(sorted, [73; 2]);
            assert_eq!(groups, [79; 6]);
            assert_eq!(result, [91; 2]);
        }
        rows[4] = 1;
        assert_eq!(
            unsafe {
                prodex_runtime_repo_duplicate_groups_v1(
                    1,
                    rows.as_ptr() as usize as u64,
                    2,
                    2,
                    3,
                    out,
                    2,
                    group_ptr,
                    6,
                    result_ptr,
                )
            },
            1,
        );
        assert_eq!(sorted, [73; 2]);
        assert_eq!(groups, [79; 6]);
        assert_eq!(result, [91; 2]);
    }
}
