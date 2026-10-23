//! Borrowed ranges for request Connection values; token grammar stays in Mojo.
use crate::{MojoError, json::StringView};

unsafe extern "C" {
    fn prodex_runtime_request_connection_tokens_v1(
        abi_version: i64,
        values_address: u64,
        value_count: i64,
        measuring: i64,
        output_address: u64,
        capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Preserve accepted token order, casing and duplicates across Connection fields.
pub fn request_connection_tokens(values: &[&str]) -> Result<Vec<String>, MojoError> {
    let views = values
        .iter()
        .map(|value| StringView::from(*value))
        .collect::<Vec<_>>();
    let value_count = i64::try_from(views.len()).map_err(|_| MojoError::InvalidInput)?;
    let invoke = |phase, output, capacity, written: &mut i64| unsafe {
        prodex_runtime_request_connection_tokens_v1(
            1,
            views.as_ptr() as u64,
            value_count,
            phase,
            output,
            capacity,
            (written as *mut i64) as u64,
        )
    };
    let mut required = -1_i64;
    crate::launch::status(invoke(1, 0, 0, &mut required))?;
    let count = usize::try_from(required).map_err(|_| MojoError::InvalidOutput)?;
    let maximum = values
        .iter()
        .try_fold(0usize, |total, value| total.checked_add(value.len()))
        .ok_or(MojoError::InvalidInput)?;
    if count > maximum {
        return Err(MojoError::InvalidOutput);
    }
    let mut rows = Vec::<[i64; 3]>::new();
    rows.try_reserve_exact(count)
        .map_err(|_| MojoError::Capacity)?;
    rows.resize(count, [-1; 3]);
    let mut written = -1_i64;
    crate::launch::status(invoke(0, rows.as_mut_ptr() as u64, required, &mut written))?;
    if written != required {
        return Err(MojoError::InvalidOutput);
    }
    rows.into_iter()
        .map(|[source, start, end]| {
            let source = usize::try_from(source).map_err(|_| MojoError::InvalidOutput)?;
            let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
            let value = values.get(source).ok_or(MojoError::InvalidOutput)?;
            value
                .get(start..end)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or(MojoError::InvalidOutput)
        })
        .collect()
}

#[cfg(test)]
mod tests;
