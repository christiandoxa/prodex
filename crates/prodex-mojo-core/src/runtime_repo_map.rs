use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 512;

unsafe extern "C" {
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
    fn prodex_runtime_repo_duplicate_plan_v1(
        abi_version: i64,
        occurrence_count: i64,
        duplicate_count: i64,
        max_duplicate_count: i64,
        max_occurrences: i64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoDuplicatePlan {
    Skip,
    Append { occurrences_complete: bool },
    StopCapacity,
}

pub fn repo_duplicate_plan(
    occurrence_count: usize,
    duplicate_count: usize,
    max_duplicate_count: usize,
    max_occurrences: usize,
) -> Result<RepoDuplicatePlan, MojoError> {
    let result = unsafe {
        prodex_runtime_repo_duplicate_plan_v1(
            ABI_VERSION,
            i64::try_from(occurrence_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(duplicate_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_duplicate_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_occurrences).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    if result == -1 {
        return Err(MojoError::InvalidInput);
    }
    match result & 0xff {
        0 => Ok(RepoDuplicatePlan::Skip),
        1 => Ok(RepoDuplicatePlan::Append {
            occurrences_complete: ((result >> 8) & 0xff) == 1,
        }),
        2 => Ok(RepoDuplicatePlan::StopCapacity),
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
        assert_eq!(
            repo_duplicate_plan(3, 0, 8, 4).unwrap(),
            RepoDuplicatePlan::Append {
                occurrences_complete: true
            }
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
