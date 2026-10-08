use std::cmp::Ordering;

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const CODEX_VERSION_FORMAT_ABI_VERSION: i64 = 1;
const INSTALL_CHANNEL: i64 = 0;
const EMIT_NOTICE: i64 = 1;
const CACHE_FRESH: i64 = 2;
const RELEASE_VERSION_VALID: i64 = 3;
const RELEASE_VERSION_COMPARE: i64 = 4;
const UPDATE_DECISION: i64 = 5;

const RELEASE_VERSION_TOTAL_ORDER: i64 = 0;
const RELEASE_VERSION_PRECEDENCE: i64 = 1;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseVersionOrder {
    Total,
    Precedence,
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

    fn prodex_update_notice_format_codex_version_v1(
        abi_version: i64,
        current_address: u64,
        current_length: i64,
        current_present: i64,
        latest_address: u64,
        latest_length: i64,
        latest_present: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
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
    call_bytes(operation, text0.as_bytes(), text1.as_bytes(), input)
}

fn call_bytes(
    operation: i64,
    text0: &[u8],
    text1: &[u8],
    input: PolicyInput,
) -> Result<i64, MojoError> {
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

pub fn release_version_is_valid(version: &str) -> Result<bool, MojoError> {
    match call(RELEASE_VERSION_VALID, version, "", PolicyInput::default())? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn compare_release_versions(
    candidate: &str,
    current: &str,
    order: ReleaseVersionOrder,
) -> Result<Option<Ordering>, MojoError> {
    let result = call(
        RELEASE_VERSION_COMPARE,
        candidate,
        current,
        PolicyInput {
            tag0: match order {
                ReleaseVersionOrder::Total => RELEASE_VERSION_TOTAL_ORDER,
                ReleaseVersionOrder::Precedence => RELEASE_VERSION_PRECEDENCE,
            },
            ..PolicyInput::default()
        },
    )?;
    match result {
        -2 => Ok(None),
        -1 => Ok(Some(Ordering::Less)),
        0 => Ok(Some(Ordering::Equal)),
        1 => Ok(Some(Ordering::Greater)),
        _ => Err(MojoError::InvalidOutput),
    }
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

pub fn update_decision(current: &str, target: &str) -> Result<i64, MojoError> {
    call(UPDATE_DECISION, current, target, PolicyInput::default())
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

pub fn format_codex_version(
    current: Option<&str>,
    latest: Option<&str>,
) -> Result<String, MojoError> {
    let current_text = current.unwrap_or_default();
    let latest_text = latest.unwrap_or_default();
    let capacity = current_text
        .len()
        .checked_add(latest_text.len())
        .and_then(|length| length.checked_add(64))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_update_notice_format_codex_version_v1(
            CODEX_VERSION_FORMAT_ABI_VERSION,
            current_text.as_ptr() as usize as u64,
            i64::try_from(current_text.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(current.is_some()),
            latest_text.as_ptr() as usize as u64,
            i64::try_from(latest_text.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(latest.is_some()),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
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

    #[test]
    fn codex_version_formatter_preserves_statuses_and_total_order() {
        assert_eq!(
            format_codex_version(Some("1.0.0+build.2"), Some("1.0.0+build.10")).unwrap(),
            "1.0.0+build.2 (update available: 1.0.0+build.10)"
        );
        assert_eq!(
            format_codex_version(Some("1.0.0"), Some("1.0.0-rc.1")).unwrap(),
            "1.0.0 (up to date)"
        );
        assert_eq!(
            format_codex_version(Some("unknown"), Some("1.0.0")).unwrap(),
            "unknown (up to date)"
        );
        assert_eq!(
            format_codex_version(Some("1.0.0"), None).unwrap(),
            "1.0.0 (update check unavailable)"
        );
        assert_eq!(
            format_codex_version(None, Some("1.0.0")).unwrap(),
            "not detected (latest release: 1.0.0)"
        );
        assert_eq!(
            format_codex_version(None, None).unwrap(),
            "not detected (update check unavailable)"
        );
    }

    #[test]
    fn release_version_abi_matches_strict_semver_validation_and_ordering() {
        let valid_cases = [
            ("1.2.3", true),
            ("v1.2.3", true),
            ("\u{2003}v1.2.3\u{3000}", true),
            ("1.2.3-alpha.9", true),
            ("1.2.3+build.01", true),
            ("18446744073709551615.0.0", true),
            ("01.2.3", false),
            ("1.02.3", false),
            ("1.2.03", false),
            ("1.2.3-01", false),
            ("18446744073709551616.0.0", false),
            ("1.2", false),
            ("1.2.3.4", false),
            ("vv1.2.3", false),
            ("V1.2.3", false),
            ("1.2.3+", false),
            ("1.2.3-α", false),
        ];
        for (version, expected) in valid_cases {
            assert_eq!(
                release_version_is_valid(version).unwrap(),
                expected,
                "{version}"
            );
        }

        let compare = |candidate, current, order| {
            compare_release_versions(candidate, current, order).unwrap()
        };
        assert_eq!(
            compare("1.2.3", "1.2.3", ReleaseVersionOrder::Total),
            Some(Ordering::Equal)
        );
        assert_eq!(
            compare("1.10.0", "1.9.0", ReleaseVersionOrder::Precedence),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare("1.0.0-rc.10", "1.0.0-rc.2", ReleaseVersionOrder::Precedence),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare("1.0.0-rc.1", "1.0.0", ReleaseVersionOrder::Precedence),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare(
                "1.0.0+build.10",
                "1.0.0+build.2",
                ReleaseVersionOrder::Total
            ),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare("1.0.0+001", "1.0.0+1", ReleaseVersionOrder::Total),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare(
                "1.0.0+build-b",
                "1.0.0+build-a",
                ReleaseVersionOrder::Precedence
            ),
            Some(Ordering::Equal)
        );
        assert_eq!(
            compare("vv1.0.0", "1.0.0", ReleaseVersionOrder::Total),
            None
        );
    }

    #[test]
    fn release_version_abi_rejects_invalid_utf8_views() {
        let invalid_utf8 = [b'1', b'.', b'2', b'.', b'3', 0xff];
        assert_eq!(
            call_bytes(
                RELEASE_VERSION_VALID,
                &invalid_utf8,
                b"",
                PolicyInput::default(),
            ),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn release_version_abi_enforces_the_existing_text_bound() {
        const MAX_TEXT_BYTES: usize = 1_048_576;
        let version = format!("1.2.3+{}", "a".repeat(MAX_TEXT_BYTES - 6));
        assert!(release_version_is_valid(&version).unwrap());
        assert_eq!(
            call_bytes(
                RELEASE_VERSION_VALID,
                format!("{version}a").as_bytes(),
                b"",
                PolicyInput::default(),
            ),
            Err(MojoError::InvalidInput)
        );
    }
}
