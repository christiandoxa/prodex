//! Numeric binding-retention plans. Identifiers and map mutation stay with the host.
use crate::MojoError;

const NEED_RANKS: i64 = 5;

unsafe extern "C" {
    fn prodex_state_binding_prune_v1(
        version: i64,
        ranks_address: u64,
        count: i64,
        width: i64,
        max_entries: u64,
        output_address: u64,
        output_words: i64,
    ) -> i64;
}

/// Selects the coldest binding indices from numeric retention keys.
/// Widths 2, 9 and 10 represent profile, response and status retention keys.
/// Equal keys retain their original input order. Facts are acquired lazily:
/// the iterator is not evaluated when the compiled policy keeps every entry.
pub fn binding_prune_indices<const WIDTH: usize>(
    ranks: impl ExactSizeIterator<Item = [i64; WIDTH]>,
    max_entries: usize,
) -> Result<Vec<usize>, MojoError> {
    let count = ranks.len();
    let count_abi = i64::try_from(count).map_err(|_| MojoError::InvalidInput)?;
    let width = i64::try_from(WIDTH).map_err(|_| MojoError::InvalidInput)?;
    let max_abi = u64::try_from(max_entries).map_err(|_| MojoError::InvalidInput)?;
    let mut probe = [-1_i64];
    let status = unsafe {
        prodex_state_binding_prune_v1(
            1,
            0,
            count_abi,
            width,
            max_abi,
            probe.as_mut_ptr() as u64,
            1,
        )
    };
    match status {
        0 if probe == [0] && count <= max_entries => return Ok(Vec::new()),
        NEED_RANKS => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let ranks = ranks.collect::<Vec<_>>();
    if ranks.len() != count {
        return Err(MojoError::InvalidInput);
    }
    let capacity = count.checked_add(1).ok_or(MojoError::Capacity)?;
    let mut output = vec![-1_i64; capacity];
    let status = unsafe {
        prodex_state_binding_prune_v1(
            1,
            ranks.as_ptr() as u64,
            count_abi,
            width,
            max_abi,
            output.as_mut_ptr() as u64,
            i64::try_from(capacity).map_err(|_| MojoError::Capacity)?,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let removed = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if count.checked_sub(max_entries) != Some(removed) || removed > count {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; count];
    output[1..1 + removed]
        .iter()
        .map(|value| {
            let index = usize::try_from(*value).map_err(|_| MojoError::InvalidOutput)?;
            if index >= count || std::mem::replace(&mut seen[index], true) {
                return Err(MojoError::InvalidOutput);
            }
            Ok(index)
        })
        .collect()
}

#[cfg(test)]
#[path = "binding_prune_tests.rs"]
mod tests;
