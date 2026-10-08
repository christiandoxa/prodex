//! Mojo-owned dynamic provider catalog policy ABI adapter.

use super::*;

/// Chooses the dynamic model-catalog status from observed entries and load state.
pub fn catalog_status(has_models: bool, degraded: bool) -> Result<CatalogStatusPlan, MojoError> {
    let scalar = i64::from(has_models) | (i64::from(degraded) << 1);
    let result = call(Operation::CatalogStatus, "", scalar)?;
    match result[0] {
        0 => Ok(CatalogStatusPlan::NoDynamicCatalog),
        1 => Ok(CatalogStatusPlan::Available),
        2 => Ok(CatalogStatusPlan::Degraded),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Selects and normalizes one provider catalog entry using its JSON field values.
pub fn catalog_entry_plan(
    id_fields: [Option<&str>; 5],
    supported_in_api_false: bool,
    hidden_true: bool,
    visibility: Option<&str>,
) -> Result<CatalogEntryPlan, MojoError> {
    let values = id_fields
        .into_iter()
        .chain([visibility])
        .map(|value| -> Result<_, MojoError> {
            let value = value.unwrap_or_default();
            Ok(CatalogStringView {
                ptr: value.as_ptr() as usize as u64,
                len: u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let flags = i64::from(supported_in_api_false)
        | (i64::from(hidden_true) << 1)
        | (i64::from(visibility.is_some()) << 2);
    let mut result = [-1_i64; 5];
    let status = unsafe {
        prodex_sub_agent_catalog_entry_v1(
            ABI_VERSION,
            values.as_ptr() as usize as u64,
            i64::try_from(values.len()).map_err(|_| MojoError::InvalidInput)?,
            flags,
            result.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let model_id = match result[0] {
        -1 if result[1] == 0 && result[2] == 0 => None,
        field if (0..5).contains(&field) => {
            let field = usize::try_from(field).map_err(|_| MojoError::InvalidOutput)?;
            let start = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(result[2]).map_err(|_| MojoError::InvalidOutput)?;
            let Some(value) = id_fields[field] else {
                return Err(MojoError::InvalidOutput);
            };
            if start >= end || value.get(start..end).is_none() {
                return Err(MojoError::InvalidOutput);
            }
            Some((field, start, end))
        }
        _ => return Err(MojoError::InvalidOutput),
    };
    let set_id = match result[3] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let selectable = match result[4] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(CatalogEntryPlan {
        model_id,
        set_id,
        selectable,
    })
}

/// Uses the full effort list when a model is absent from the provider catalog.
pub fn use_all_effort_suggestions(model_catalogued: bool) -> Result<bool, MojoError> {
    let scalar = i64::from(model_catalogued);
    let result = call(Operation::EffortSuggestionMask, "", scalar)?;
    match result[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}
