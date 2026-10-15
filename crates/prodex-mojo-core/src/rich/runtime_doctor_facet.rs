//! Runtime-doctor facet winner selection ABI.
//!
//! Rust owns the ordered map and materializes the selected value. Mojo owns
//! the bounded count and lexical tie-break reducer.

use super::{ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view};
use crate::MojoError;

/// Version of the runtime-doctor facet selection ABI.
pub const RUNTIME_DOCTOR_TOP_FACET_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_TOP_FACET_MAX_COUNT: usize = 65_536;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_top_facet_selection_v1(
        abi_version: i64,
        values_address: u64,
        counts_address: u64,
        count: i64,
        selected_index_address: u64,
        selected_count_address: u64,
    ) -> i64;
}

/// The map entry selected by the Mojo facet reducer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorTopFacetSelection {
    pub index: usize,
    pub count: usize,
}

/// Select the highest-count facet entry, breaking equal counts by the lexical
/// maximum of the already ordered UTF-8 keys.
pub fn runtime_doctor_top_facet_selection<'a>(
    entries: impl IntoIterator<Item = (&'a str, usize)>,
) -> Result<Option<RuntimeDoctorTopFacetSelection>, MojoError> {
    ensure_rich_abi()?;
    let entries = entries.into_iter().collect::<Vec<_>>();
    if entries.len() > RUNTIME_DOCTOR_TOP_FACET_MAX_COUNT {
        return Err(MojoError::Capacity);
    }
    if entries.is_empty() {
        return Ok(None);
    }
    let values = entries
        .iter()
        .map(|(value, _)| view(value))
        .collect::<Vec<_>>();
    let counts = entries
        .iter()
        .map(|(_, count)| u64::try_from(*count).map_err(|_| MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let mut selected_index = -1_i64;
    let mut selected_count = 0_u64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_top_facet_selection_v1(
            RUNTIME_DOCTOR_TOP_FACET_ABI_VERSION,
            mojo_pointer_address(values.as_ptr()),
            mojo_pointer_address(counts.as_ptr()),
            i64::try_from(entries.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut selected_index),
            mojo_mut_pointer_address(&mut selected_count),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::InvalidOutput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let index = usize::try_from(selected_index).map_err(|_| MojoError::InvalidOutput)?;
    let count = usize::try_from(selected_count).map_err(|_| MojoError::InvalidOutput)?;
    let expected_count = entries
        .get(index)
        .map(|(_, value)| *value)
        .ok_or(MojoError::InvalidOutput)?;
    if count != expected_count {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(RuntimeDoctorTopFacetSelection { index, count }))
}

#[cfg(test)]
mod tests {
    use super::runtime_doctor_top_facet_selection;

    #[test]
    fn facet_selection_preserves_count_precedence_and_lexical_ties() {
        assert_eq!(runtime_doctor_top_facet_selection([]).unwrap(), None);
        assert_eq!(
            runtime_doctor_top_facet_selection([("alpha", 4), ("zeta", 4), ("highest", 5),])
                .unwrap(),
            Some(super::RuntimeDoctorTopFacetSelection { index: 2, count: 5 })
        );
        assert_eq!(
            runtime_doctor_top_facet_selection([("alpha", 2), ("é", 2), ("東京", 2)]).unwrap(),
            Some(super::RuntimeDoctorTopFacetSelection { index: 2, count: 2 })
        );
    }
}
