use crate::MojoError;

const ABI_VERSION: i64 = 1;
const OWNED_ROOT_TEMP: i64 = 0;
const ROOT_TEMP_PID: i64 = 1;
const STALE_ROOT_TEMP: i64 = 2;
const RUNTIME_LOG_NAME: i64 = 3;
const LOGIN_TEMP_NAME: i64 = 4;
const BROKER_ARTIFACT_KEY: i64 = 5;
const BROKER_LEASE_PID: i64 = 6;

unsafe extern "C" {
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
