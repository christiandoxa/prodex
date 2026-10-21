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
    for raw in groups.as_chunks::<3>().0.iter().take(group_count) {
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
#[path = "../tests/src/runtime_repo_map/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/src/runtime_repo_map/compiled_artifact_order_tests.rs"]
mod compiled_artifact_order_tests;

#[cfg(test)]
#[path = "../tests/src/runtime_repo_map/compiled_artifact_eviction_tests.rs"]
mod compiled_artifact_eviction_tests;

#[cfg(test)]
#[path = "../tests/src/runtime_repo_map/compiled_nearest_path_tests.rs"]
mod compiled_nearest_path_tests;

#[cfg(test)]
#[path = "../tests/src/runtime_repo_map/compiled_repo_map_path_set_tests.rs"]
mod compiled_repo_map_path_set_tests;

#[cfg(test)]
#[path = "../tests/src/runtime_repo_map/compiled_duplicate_chunk_group_tests.rs"]
mod compiled_duplicate_chunk_group_tests;
