//! Checked first-occurrence project-trust ordering from compiled Mojo.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_ENTRIES: usize = 65_536;
const MAX_PATH_BYTES: usize = 1_048_576;

#[repr(C)]
#[derive(Clone, Copy)]
struct TrustEntryView {
    path: u64,
    path_bytes: u64,
    encoded_bytes: u64,
}

const _: () = {
    assert!(std::mem::size_of::<TrustEntryView>() == 24);
    assert!(std::mem::align_of::<TrustEntryView>() == 8);
};

unsafe extern "C" {
    fn prodex_runtime_overlay_trust_entry_plan_v1(
        abi_version: i64,
        records_address: u64,
        count: i64,
        max_encoded_bytes: u64,
        output_address: u64,
        output_capacity: i64,
        scratch_address: u64,
        scratch_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Select input-order project entries after exact path deduplication and
/// bounded serialized output planning. Rust never repeats Mojo's choices.
pub fn workspace_trust_entry_indices(
    entries: &[(&str, usize)],
    max_encoded_bytes: usize,
) -> Result<Vec<usize>, MojoError> {
    if entries.len() > MAX_ENTRIES || entries.iter().any(|(path, _)| path.len() > MAX_PATH_BYTES) {
        return Err(MojoError::InvalidInput);
    }
    let rows = entries
        .iter()
        .map(|(path, entry_len)| {
            Ok(TrustEntryView {
                path: path.as_ptr() as usize as u64,
                path_bytes: u64::try_from(path.len()).map_err(|_| MojoError::InvalidInput)?,
                encoded_bytes: u64::try_from(*entry_len).map_err(|_| MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let mut selected = vec![-1_i64; rows.len()];
    let mut scratch = vec![-1_i64; rows.len().checked_mul(2).ok_or(MojoError::Capacity)?];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_runtime_overlay_trust_entry_plan_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(rows.len()).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(max_encoded_bytes).map_err(|_| MojoError::InvalidInput)?,
            selected.as_mut_ptr() as usize as u64,
            i64::try_from(selected.len()).map_err(|_| MojoError::InvalidInput)?,
            scratch.as_mut_ptr() as usize as u64,
            i64::try_from(scratch.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > rows.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut previous = None;
    selected
        .into_iter()
        .take(count)
        .map(|index| {
            let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
            if index >= rows.len() || previous.is_some_and(|previous| previous >= index) {
                return Err(MojoError::InvalidOutput);
            }
            previous = Some(index);
            Ok(index)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_trust_selection_is_first_occurrence_and_input_order() {
        assert_eq!(
            workspace_trust_entry_indices(
                &[("/a", 10), ("/b", 20), ("/a", 10), ("/c", 30), ("/b", 20)],
                42,
            ),
            Ok(vec![0, 1])
        );
        assert_eq!(workspace_trust_entry_indices(&[], 1), Ok(vec![]));
        assert_eq!(
            workspace_trust_entry_indices(&[("雪", 200)], 10),
            Ok(vec![0])
        );
        assert_eq!(
            workspace_trust_entry_indices(&[("Case", 1), ("case", 1)], 40),
            Ok(vec![0, 1])
        );
        assert_eq!(
            workspace_trust_entry_indices(&[("α", 1), ("α", 1)], 40),
            Ok(vec![0])
        );
    }

    #[test]
    fn compiled_trust_selection_rejects_oversized_records() {
        let long = "x".repeat(MAX_PATH_BYTES + 1);
        assert_eq!(
            workspace_trust_entry_indices(&[(&long, 10)], 100),
            Err(MojoError::InvalidInput)
        );
    }
}
