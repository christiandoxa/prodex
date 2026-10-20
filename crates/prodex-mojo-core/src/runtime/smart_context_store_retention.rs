//! Compiled retention selection for persisted Smart Context scope stores.
//! Host owns filesystem metadata and eventual removal; Mojo owns ordering,
//! strict expiry and the cumulative disk byte limit.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_STORES: usize = 65_536;

unsafe extern "C" {
    fn prodex_smart_context_store_retention_v1(
        abi_version: i64,
        rows_address: u64,
        count: i64,
        retention_secs: u64,
        retention_nanos: u64,
        disk_cap: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// Borrowed raw filename/path bytes plus system-clock and elapsed facts.
#[derive(Debug, Clone, Copy)]
pub struct SmartContextStoreRetentionInput<'a> {
    pub path: &'a [u8],
    pub modified_before_unix_epoch: bool,
    pub modified_abs_seconds: u64,
    pub modified_abs_nanos: u32,
    pub elapsed_seconds: u64,
    pub elapsed_nanos: u32,
    pub size: u64,
}

/// Sorted original indices for stores whose expiration or running disk
/// budget requires removal. The caller retains all original PathBufs.
pub fn smart_context_store_retention_plan(
    stores: &[SmartContextStoreRetentionInput<'_>],
    retention: std::time::Duration,
    disk_cap: u64,
) -> Result<Vec<usize>, MojoError> {
    if stores.len() > MAX_STORES {
        return Err(MojoError::InvalidInput);
    }
    let mut rows = Vec::<u64>::new();
    rows.try_reserve_exact(stores.len().checked_mul(9).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for store in stores {
        if store.modified_abs_nanos >= 1_000_000_000 || store.elapsed_nanos >= 1_000_000_000 {
            return Err(MojoError::InvalidInput);
        }
        rows.extend([
            store.path.as_ptr() as usize as u64,
            u64::try_from(store.path.len()).map_err(|_| MojoError::InvalidInput)?,
            u64::from(store.modified_before_unix_epoch),
            store.modified_abs_seconds,
            u64::from(store.modified_abs_nanos),
            store.elapsed_seconds,
            u64::from(store.elapsed_nanos),
            store.size,
            0,
        ]);
    }
    let mut output = Vec::<i64>::new();
    output
        .try_reserve_exact(stores.len())
        .map_err(|_| MojoError::Capacity)?;
    output.resize(stores.len(), -1);
    let mut written = -1_i64;
    match unsafe {
        prodex_smart_context_store_retention_v1(
            ABI_VERSION,
            rows.as_ptr() as usize as u64,
            i64::try_from(stores.len()).map_err(|_| MojoError::InvalidInput)?,
            retention.as_secs(),
            u64::from(retention.subsec_nanos()),
            disk_cap,
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
    if count > stores.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; stores.len()];
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry<'a>(
        path: &'a str,
        timestamp: u64,
        age_secs: u64,
        bytes: u64,
    ) -> SmartContextStoreRetentionInput<'a> {
        SmartContextStoreRetentionInput {
            path: path.as_bytes(),
            modified_before_unix_epoch: false,
            modified_abs_seconds: timestamp,
            modified_abs_nanos: 0,
            elapsed_seconds: age_secs,
            elapsed_nanos: 0,
            size: bytes,
        }
    }

    #[test]
    fn compiled_store_retention_sorts_by_timestamp_then_path_and_enforces_budget() {
        let stores = [
            entry("z", 100, 10, 4),
            entry("b", 10, 100, 8),
            entry("a", 10, 100, 3),
            entry("y", 150, 0, 3),
        ];
        assert_eq!(
            smart_context_store_retention_plan(&stores, std::time::Duration::from_secs(50), 12),
            Ok(vec![2, 1]),
            "expired oldest stores remove first and free enough disk space",
        );
        assert_eq!(
            smart_context_store_retention_plan(&stores, std::time::Duration::from_secs(200), 12),
            Ok(vec![2, 1]),
            "over budget removes earliest timestamps, breaking ties by path",
        );
        assert_eq!(
            smart_context_store_retention_plan(&stores, std::time::Duration::from_secs(200), 18),
            Ok(vec![]),
        );
        assert_eq!(
            smart_context_store_retention_plan(&[], std::time::Duration::ZERO, 0),
            Ok(vec![]),
        );
    }

    #[test]
    fn compiled_store_retention_handles_strict_nano_expiry_pre_epoch_and_unicode_paths() {
        let records = [
            SmartContextStoreRetentionInput {
                path: "雪".as_bytes(),
                modified_before_unix_epoch: true,
                modified_abs_seconds: 3,
                modified_abs_nanos: 1,
                elapsed_seconds: 10,
                elapsed_nanos: 1,
                size: 1,
            },
            SmartContextStoreRetentionInput {
                path: "é".as_bytes(),
                modified_before_unix_epoch: true,
                modified_abs_seconds: 3,
                modified_abs_nanos: 2,
                elapsed_seconds: 10,
                elapsed_nanos: 0,
                size: 1,
            },
            SmartContextStoreRetentionInput {
                path: "a".as_bytes(),
                modified_before_unix_epoch: false,
                modified_abs_seconds: 0,
                modified_abs_nanos: 0,
                elapsed_seconds: 10,
                elapsed_nanos: 0,
                size: 1,
            },
        ];
        assert_eq!(
            smart_context_store_retention_plan(&records, std::time::Duration::from_secs(10), 3,),
            Ok(vec![0]),
            "only a duration strictly greater than threshold is expired",
        );
        assert_eq!(
            smart_context_store_retention_plan(&records, std::time::Duration::from_secs(20), 1,),
            Ok(vec![1, 0]),
            "pre-epoch timestamps are sorted in reverse absolute time",
        );
        let old = [
            SmartContextStoreRetentionInput {
                path: "é".as_bytes(),
                ..records[2]
            },
            SmartContextStoreRetentionInput {
                path: "雪".as_bytes(),
                ..records[2]
            },
            SmartContextStoreRetentionInput {
                path: "a".as_bytes(),
                ..records[2]
            },
        ];
        assert_eq!(
            smart_context_store_retention_plan(&old, std::time::Duration::from_secs(20), 1),
            Ok(vec![2, 0]),
        );
    }

    #[test]
    fn compiled_store_retention_abi_rejects_late_invalid_row_before_writes() {
        let mut rows = [0_u64; 18];
        let mut output = [71_i64; 2];
        let mut written = 91_i64;
        let input_ptr = rows.as_ptr() as usize as u64;
        let output_ptr = output.as_mut_ptr() as usize as u64;
        let count_ptr = (&mut written as *mut i64) as usize as u64;
        for (abi, ptr, count, nanos, capacity, wrote, expected) in [
            (2, input_ptr, 2, 0, 2, count_ptr, 4),
            (1, 0, 2, 0, 2, count_ptr, 1),
            (1, input_ptr, -1, 0, 2, count_ptr, 1),
            (1, input_ptr, 65_537, 0, 2, count_ptr, 1),
            (1, input_ptr, 2, 1_000_000_000, 2, count_ptr, 1),
            (1, input_ptr, 2, 0, 1, count_ptr, 1),
            (1, input_ptr, 2, 0, 2, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_smart_context_store_retention_v1(
                        abi, ptr, count, 10, nanos, 2, output_ptr, capacity, wrote,
                    )
                },
                expected,
            );
            assert_eq!(output, [71; 2]);
            assert_eq!(written, 91);
        }
        rows[13] = 1_000_000_000; // elapsed subsecond for final row
        assert_eq!(
            unsafe {
                prodex_smart_context_store_retention_v1(
                    1,
                    rows.as_ptr() as usize as u64,
                    2,
                    10,
                    0,
                    2,
                    output_ptr,
                    2,
                    count_ptr,
                )
            },
            1,
        );
        assert_eq!(output, [71; 2]);
        assert_eq!(written, 91);
    }
}
