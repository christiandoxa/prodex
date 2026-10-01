use crate::MojoError;

const ABI_VERSION: i64 = 1;
const INSTALL_CHANNEL: i64 = 0;
const EMIT_NOTICE: i64 = 1;
const CACHE_FRESH: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateInstallChannelClass {
    Standalone,
    Npm,
    Cargo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateNoticeCommandClass {
    Other,
    Doctor,
    Update,
    Quota,
}

unsafe extern "C" {
    fn prodex_update_notice_policy_v1(
        abi_version: i64,
        operation: i64,
        text0_address: u64,
        text0_length: i64,
        text1_address: u64,
        text1_length: i64,
        tag0: i64,
        flag0: i64,
        flag1: i64,
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

#[derive(Debug, Clone, Copy, Default)]
struct PolicyInput {
    tag0: i64,
    flag0: bool,
    flag1: bool,
    signed0: i64,
    signed1: i64,
}

fn call(operation: i64, text0: &str, text1: &str, input: PolicyInput) -> Result<i64, MojoError> {
    let mut output = -1_i64;
    status(unsafe {
        prodex_update_notice_policy_v1(
            ABI_VERSION,
            operation,
            text0.as_ptr() as usize as u64,
            i64::try_from(text0.len()).map_err(|_| MojoError::InvalidInput)?,
            text1.as_ptr() as usize as u64,
            i64::try_from(text1.len()).map_err(|_| MojoError::InvalidInput)?,
            input.tag0,
            i64::from(input.flag0),
            i64::from(input.flag1),
            input.signed0,
            input.signed1,
            (&mut output as *mut i64) as usize as u64,
        )
    })?;
    Ok(output)
}

pub fn install_channel(
    npm_package_name: Option<&str>,
    executable_path: Option<&str>,
) -> Result<UpdateInstallChannelClass, MojoError> {
    let package = npm_package_name.unwrap_or_default();
    let path = executable_path.unwrap_or_default();
    match call(
        INSTALL_CHANNEL,
        package,
        path,
        PolicyInput {
            tag0: i64::from(npm_package_name.is_some()),
            ..PolicyInput::default()
        },
    )? {
        0 => Ok(UpdateInstallChannelClass::Standalone),
        1 => Ok(UpdateInstallChannelClass::Npm),
        2 => Ok(UpdateInstallChannelClass::Cargo),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn should_emit_notice(
    command: UpdateNoticeCommandClass,
    doctor_json: bool,
    doctor_bundle_present: bool,
    quota_raw: bool,
) -> Result<bool, MojoError> {
    let (tag, flag0, flag1) = match command {
        UpdateNoticeCommandClass::Other => (0, false, false),
        UpdateNoticeCommandClass::Doctor => (1, doctor_json, doctor_bundle_present),
        UpdateNoticeCommandClass::Update => (2, false, false),
        UpdateNoticeCommandClass::Quota => (3, quota_raw, false),
    };
    match call(
        EMIT_NOTICE,
        "",
        "",
        PolicyInput {
            tag0: tag,
            flag0,
            flag1,
            ..PolicyInput::default()
        },
    )? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn cache_is_fresh(
    source_equal: bool,
    now: i64,
    checked_at: i64,
    ttl_seconds: i64,
) -> Result<bool, MojoError> {
    match call(
        CACHE_FRESH,
        "",
        "",
        PolicyInput {
            tag0: ttl_seconds,
            flag0: source_equal,
            signed0: now,
            signed1: checked_at,
            ..PolicyInput::default()
        },
    )? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_notice_policy_preserves_channel_notice_and_cache_contracts() {
        assert_eq!(
            install_channel(Some("@christiandoxa/prodex"), None).unwrap(),
            UpdateInstallChannelClass::Npm
        );
        assert_eq!(
            install_channel(
                None,
                Some("C:/Users/test-user/node_modules/@christiandoxa/prodex-win32-x64/vendor/prodex.exe")
            )
            .unwrap(),
            UpdateInstallChannelClass::Npm
        );
        assert_eq!(
            install_channel(None, Some("/home/x/.cargo/bin/prodex")).unwrap(),
            UpdateInstallChannelClass::Cargo
        );
        assert_eq!(
            install_channel(None, Some("/home/x/.local/bin/prodex")).unwrap(),
            UpdateInstallChannelClass::Standalone
        );

        assert!(should_emit_notice(UpdateNoticeCommandClass::Doctor, false, false, false).unwrap());
        assert!(!should_emit_notice(UpdateNoticeCommandClass::Doctor, true, false, false).unwrap());
        assert!(
            !should_emit_notice(UpdateNoticeCommandClass::Update, false, false, false).unwrap()
        );
        assert!(!should_emit_notice(UpdateNoticeCommandClass::Quota, false, false, true).unwrap());
        assert!(should_emit_notice(UpdateNoticeCommandClass::Other, false, false, false).unwrap());

        assert!(cache_is_fresh(true, 100, 0, 300).unwrap());
        assert!(!cache_is_fresh(false, 100, 0, 300).unwrap());
        assert!(!cache_is_fresh(true, 300, 0, 300).unwrap());
        assert!(cache_is_fresh(true, i64::MIN, i64::MAX, 300).unwrap());
    }
}
