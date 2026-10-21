use crate::MojoError;

const ABI_VERSION: i64 = 1;
const OWNED_ROOT_TEMP: i64 = 0;
const ROOT_TEMP_PID: i64 = 1;
const STALE_ROOT_TEMP: i64 = 2;
const RUNTIME_LOG_NAME: i64 = 3;
const LOGIN_TEMP_NAME: i64 = 4;
const BROKER_ARTIFACT_KEY: i64 = 5;
const BROKER_LEASE_PID: i64 = 6;
const STALE_LOGIN_TEMP: i64 = 7;

unsafe extern "C" {
    fn prodex_core_file_lexical_order_v1(
        abi_version: i64,
        views_address: u64,
        count: i64,
        unique: i64,
        indices_address: u64,
        indices_capacity: i64,
        selected_count_address: u64,
    ) -> i64;
    fn prodex_core_file_policy_v1(
        abi_version: i64,
        operation: i64,
        name_address: u64,
        name_length: i64,
        prefix_address: u64,
        prefix_length: i64,
        flag: i64,
        signed0: i64,
        signed1: i64,
        output_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Stable original source indices for raw lexical filename/key order.
/// The compiled file-policy owner optionally collapses identical views.
/// The host performs no comparison or duplicate-suppression fallback.
pub fn lexical_file_key_order(keys: &[&[u8]], unique: bool) -> Result<Vec<usize>, MojoError> {
    if keys.len() > 1_048_576 {
        return Err(MojoError::InvalidInput);
    }
    let mut views = Vec::<u64>::new();
    views
        .try_reserve_exact(keys.len().checked_mul(2).ok_or(MojoError::Capacity)?)
        .map_err(|_| MojoError::Capacity)?;
    for key in keys {
        views.extend([
            key.as_ptr() as usize as u64,
            u64::try_from(key.len()).map_err(|_| MojoError::InvalidInput)?,
        ]);
    }
    let mut indices = Vec::<i64>::new();
    indices
        .try_reserve_exact(keys.len())
        .map_err(|_| MojoError::Capacity)?;
    indices.resize(keys.len(), -1);
    let mut written = -1_i64;
    status(unsafe {
        prodex_core_file_lexical_order_v1(
            ABI_VERSION,
            views.as_ptr() as usize as u64,
            i64::try_from(keys.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(unique),
            indices.as_mut_ptr() as usize as u64,
            i64::try_from(indices.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if count > keys.len() || (!unique && count != keys.len()) {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; keys.len()];
    let mut selected = Vec::with_capacity(count);
    for raw in indices.into_iter().take(count) {
        let index = usize::try_from(raw).map_err(|_| MojoError::InvalidOutput)?;
        let used = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
        if *used {
            return Err(MojoError::InvalidOutput);
        }
        *used = true;
        selected.push(index);
    }
    Ok(selected)
}

fn call(
    operation: i64,
    name: &str,
    prefix: &str,
    flag: bool,
    signed0: i64,
    signed1: i64,
) -> Result<[u64; 4], MojoError> {
    let mut output = [0_u64; 4];
    status(unsafe {
        prodex_core_file_policy_v1(
            ABI_VERSION,
            operation,
            name.as_ptr() as usize as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
            prefix.as_ptr() as usize as u64,
            i64::try_from(prefix.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(flag),
            signed0,
            signed1,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(output)
}

fn bool_output(value: u64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn optional_u32(output: [u64; 4]) -> Result<Option<u32>, MojoError> {
    match output[0] {
        0 => Ok(None),
        1 => Ok(Some(
            u32::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        )),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn owned_root_temp_file_name(name: &str) -> Result<bool, MojoError> {
    bool_output(call(OWNED_ROOT_TEMP, name, "", false, 0, 0)?[0])
}

pub fn root_temp_file_pid(name: &str) -> Result<Option<u32>, MojoError> {
    optional_u32(call(ROOT_TEMP_PID, name, "", false, 0, 0)?)
}

pub fn stale_root_temp_file_should_remove(
    name: &str,
    modified_epoch_seconds: i64,
    oldest_allowed_epoch_seconds: i64,
    pid_alive: bool,
) -> Result<bool, MojoError> {
    bool_output(
        call(
            STALE_ROOT_TEMP,
            name,
            "",
            pid_alive,
            modified_epoch_seconds,
            oldest_allowed_epoch_seconds,
        )?[0],
    )
}

pub fn runtime_proxy_log_file_name_is_owned(name: &str, prefix: &str) -> Result<bool, MojoError> {
    bool_output(call(RUNTIME_LOG_NAME, name, prefix, false, 0, 0)?[0])
}

pub fn login_temp_dir_name_is_owned(name: &str) -> Result<bool, MojoError> {
    bool_output(call(LOGIN_TEMP_NAME, name, "", false, 0, 0)?[0])
}

pub fn stale_login_dir_should_remove(
    modified_epoch_seconds: i64,
    oldest_allowed_epoch_seconds: i64,
) -> Result<bool, MojoError> {
    bool_output(
        call(
            STALE_LOGIN_TEMP,
            "",
            "",
            false,
            modified_epoch_seconds,
            oldest_allowed_epoch_seconds,
        )?[0],
    )
}

pub fn runtime_broker_artifact_key_range(
    name: &str,
    is_dir: bool,
) -> Result<Option<(usize, usize)>, MojoError> {
    let output = call(BROKER_ARTIFACT_KEY, name, "", is_dir, 0, 0)?;
    match output[0] {
        0 => Ok(None),
        1 => {
            let start = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
            let length = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
            let end = start.checked_add(length).ok_or(MojoError::InvalidOutput)?;
            if end > name.len() || !name.is_char_boundary(start) || !name.is_char_boundary(end) {
                return Err(MojoError::InvalidOutput);
            }
            Ok(Some((start, end)))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn runtime_broker_lease_pid(file_name: &str) -> Result<Option<u32>, MojoError> {
    optional_u32(call(BROKER_LEASE_PID, file_name, "", false, 0, 0)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_file_policy_preserves_name_and_pid_contracts() {
        assert!(owned_root_temp_file_name("state.json.10.20.1.tmp").unwrap());
        assert!(!owned_root_temp_file_name("other.json.10.20.1.tmp").unwrap());
        assert_eq!(
            root_temp_file_pid("runtime-backoffs.json.999999999.1.0.tmp").unwrap(),
            Some(999_999_999)
        );
        assert_eq!(
            root_temp_file_pid("runtime-backoffs.json.tmp").unwrap(),
            None
        );
        assert!(
            stale_root_temp_file_should_remove(
                "runtime-backoffs.json.999999999.1.0.tmp",
                100,
                50,
                false
            )
            .unwrap()
        );
        assert!(
            !stale_root_temp_file_should_remove(
                "runtime-backoffs.json.999999999.1.0.tmp",
                100,
                50,
                true
            )
            .unwrap()
        );
        assert!(
            runtime_proxy_log_file_name_is_owned("runtime-proxy-1.log", "runtime-proxy-").unwrap()
        );
        assert!(login_temp_dir_name_is_owned(".login-main").unwrap());
        assert!(stale_login_dir_should_remove(99, 100).unwrap());
        assert!(!stale_login_dir_should_remove(100, 100).unwrap());
        assert!(!stale_login_dir_should_remove(101, 100).unwrap());
        assert_eq!(
            runtime_broker_artifact_key_range("runtime-broker-main.json.last-good", false).unwrap(),
            Some((15, 19))
        );
        assert_eq!(
            runtime_broker_lease_pid("1234-main.json").unwrap(),
            Some(1234)
        );
        assert_eq!(runtime_broker_lease_pid("x-main.json").unwrap(), None);
    }
}

#[cfg(test)]
mod lexical_file_key_order_tests {
    use super::*;

    #[test]
    fn compiled_lexical_file_key_order_sorts_raw_names_and_stable_duplicates() {
        let keys = [
            "雪".as_bytes(),
            "z".as_bytes(),
            "".as_bytes(),
            "é".as_bytes(),
            "a".as_bytes(),
            "a".as_bytes(),
        ];
        assert_eq!(
            lexical_file_key_order(&keys, false),
            Ok(vec![2, 4, 5, 1, 3, 0])
        );
        assert_eq!(lexical_file_key_order(&keys, true), Ok(vec![2, 4, 1, 3, 0]));
        let non_utf8 = [255_u8, 254];
        let raw = [non_utf8.as_slice(), b"z".as_slice(), b"a".as_slice()];
        assert_eq!(lexical_file_key_order(&raw, true), Ok(vec![2, 1, 0]));
        assert_eq!(lexical_file_key_order(&[], false), Ok(vec![]));
        assert_eq!(lexical_file_key_order(&[], true), Ok(vec![]));
        let keys = (0..1025_u64)
            .rev()
            .map(|x| format!("key-{x:04}"))
            .collect::<Vec<_>>();
        let borrowed = keys.iter().map(String::as_bytes).collect::<Vec<_>>();
        let indices = lexical_file_key_order(&borrowed, true).unwrap();
        assert_eq!(indices.len(), 1025);
        assert_eq!(indices[0], 1024);
        assert_eq!(indices[1024], 0);
        assert!(
            indices
                .windows(2)
                .all(|two| borrowed[two[0]] < borrowed[two[1]])
        );
    }

    #[test]
    fn compiled_lexical_file_key_order_abi_rejects_bad_late_view_without_output_writes() {
        let mut views = [0_u64; 4];
        let mut selected = [73_i64; 2];
        let mut count = 91_i64;
        let source = views.as_ptr() as usize as u64;
        let output = selected.as_mut_ptr() as usize as u64;
        let result = (&mut count as *mut i64) as usize as u64;
        for (version, address, size, mode, capacity, selected_addr, count_addr, expected) in [
            (2, source, 2, 1, 2, output, result, 4),
            (1, 0, 2, 1, 2, output, result, 1),
            (1, source, -1, 1, 2, output, result, 1),
            (1, source, 1_048_577, 1, 2, output, result, 1),
            (1, source, 2, 2, 2, output, result, 1),
            (1, source, 2, 1, 1, output, result, 1),
            (1, source, 2, 1, 2, 0, result, 1),
            (1, source, 2, 1, 2, output, 0, 1),
        ] {
            assert_eq!(
                unsafe {
                    prodex_core_file_lexical_order_v1(
                        version,
                        address,
                        size,
                        mode,
                        selected_addr,
                        capacity,
                        count_addr,
                    )
                },
                expected,
            );
            assert_eq!(selected, [73; 2]);
            assert_eq!(count, 91);
        }
        views[3] = u64::MAX;
        assert_eq!(
            unsafe {
                prodex_core_file_lexical_order_v1(
                    1,
                    views.as_ptr() as usize as u64,
                    2,
                    1,
                    output,
                    2,
                    result,
                )
            },
            1,
        );
        assert_eq!(selected, [73; 2]);
        assert_eq!(count, 91);
    }
}
