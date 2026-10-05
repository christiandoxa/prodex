use super::ProfileImportStringView;
use crate::{MojoError, rich::ensure_rich_abi};

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_profile_export_copilot_strip_json_line_comments_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_profile_export_copilot_metadata_v1(
        abi_version: i64,
        operation: i64,
        primary_address: u64,
        secondary_address: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_profile_export_copilot_url_v1(
        abi_version: i64,
        operation: i64,
        host_address: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

pub fn strip_copilot_json_line_comments(raw: &str) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let input_length = i64::try_from(raw.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = vec![0_u8; raw.len().max(1)];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_profile_export_copilot_strip_json_line_comments_v1(
            ABI_VERSION,
            raw.as_ptr() as usize as u64,
            input_length,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {
            let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
            if written > raw.len() {
                return Err(MojoError::InvalidOutput);
            }
            String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
        }
        2 => Err(MojoError::Capacity),
        99 => Err(MojoError::InvalidInput),
        100 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn copilot_metadata(operation: i64, primary: &str, secondary: &str) -> Result<[u64; 3], MojoError> {
    ensure_rich_abi()?;
    let primary = ProfileImportStringView::from((!primary.is_empty()).then_some(primary))?;
    let secondary = ProfileImportStringView::from((!secondary.is_empty()).then_some(secondary))?;
    let mut output = [0_u64; 3];
    let status = unsafe {
        prodex_profile_export_copilot_metadata_v1(
            ABI_VERSION,
            operation,
            (&primary as *const ProfileImportStringView) as usize as u64,
            (&secondary as *const ProfileImportStringView) as usize as u64,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        99 => Err(MojoError::InvalidInput),
        100 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn copilot_version_triplet(raw: &str) -> Result<(u64, u64, u64), MojoError> {
    let output = copilot_metadata(1, raw, "")?;
    Ok((output[0], output[1], output[2]))
}

pub fn copilot_platform_label(os: &str, arch: &str) -> Result<&'static str, MojoError> {
    match copilot_metadata(2, os, arch)?[0] {
        0 => Ok("linux-x64"),
        1 => Ok("linux-arm64"),
        2 => Ok("darwin-x64"),
        3 => Ok("darwin-arm64"),
        4 => Ok("win32-x64"),
        5 => Ok("win32-arm64"),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn copilot_url(operation: i64, host: &str) -> Result<Option<String>, MojoError> {
    ensure_rich_abi()?;
    let host_view = ProfileImportStringView::from((!host.is_empty()).then_some(host))?;
    let capacity = host
        .len()
        .checked_add(64)
        .ok_or(MojoError::InvalidInput)?
        .max(64);
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_profile_export_copilot_url_v1(
            ABI_VERSION,
            operation,
            (&host_view as *const ProfileImportStringView) as usize as u64,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {
            let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
            if written > output.len() {
                return Err(MojoError::InvalidOutput);
            }
            let value = String::from_utf8(output[..written].to_vec())
                .map_err(|_| MojoError::InvalidOutput)?;
            Ok(Some(value))
        }
        2 => Err(MojoError::Capacity),
        3 => Ok(None),
        99 => Err(MojoError::InvalidInput),
        100 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn copilot_user_api_origin(host: &str) -> Result<Option<String>, MojoError> {
    copilot_url(1, host)
}

pub fn copilot_models_api_url(host: &str) -> Result<String, MojoError> {
    copilot_url(2, host)?.ok_or(MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copilot_metadata_and_url_policy_are_mojo_owned() {
        assert_eq!(copilot_version_triplet("1.2.3-beta").unwrap(), (1, 2, 3));
        assert_eq!(copilot_version_triplet("9").unwrap(), (9, 0, 0));
        assert_eq!(
            copilot_version_triplet("18446744073709551616.2.3").unwrap(),
            (0, 2, 3)
        );
        assert_eq!(
            copilot_platform_label("windows", "aarch64").unwrap(),
            "win32-arm64"
        );
        assert_eq!(
            copilot_platform_label("unknown", "unknown").unwrap(),
            "linux-x64"
        );
        assert_eq!(
            copilot_user_api_origin("github.com").unwrap().as_deref(),
            Some("https://api.github.com")
        );
        assert_eq!(
            copilot_user_api_origin("http://127.0.0.1:1234/path")
                .unwrap()
                .as_deref(),
            Some("http://127.0.0.1:1234")
        );
        assert_eq!(copilot_user_api_origin("  ").unwrap(), None);
        assert_eq!(
            copilot_models_api_url("HTTPS://GITHUB.COM/").unwrap(),
            "https://api.githubcopilot.com"
        );
        assert_eq!(
            copilot_models_api_url("https://enterprise.ghe.com").unwrap(),
            "https://copilot-api.enterprise.ghe.com"
        );
    }

    #[test]
    fn copilot_jsonc_line_comment_stripping_is_mojo_owned() {
        let input = "// lead\n{\"url\":\"https://example.test//inside\", // tail\n\"value\":\"slash//inside\"}\n";
        let expected =
            "\n{\"url\":\"https://example.test//inside\", \n\"value\":\"slash//inside\"}\n";
        assert_eq!(strip_copilot_json_line_comments(input).unwrap(), expected);
    }
}
