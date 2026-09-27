use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 512;

unsafe extern "C" {
    fn prodex_runtime_repo_module_like_v1(abi_version: i64, address: u64, length: i64) -> i64;
    fn prodex_runtime_repo_module_path_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn input_len(value: &str) -> Result<i64, MojoError> {
    if value.len() > MAX_INPUT_BYTES {
        return Err(MojoError::InvalidInput);
    }
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

pub fn repo_module_like(text: &str) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_runtime_repo_module_like_v1(
            ABI_VERSION,
            text.as_ptr() as usize as u64,
            input_len(text)?,
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn repo_module_from_path(path: &str) -> Result<Option<String>, MojoError> {
    let mut output = vec![0_u8; MAX_OUTPUT_BYTES];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_runtime_repo_module_path_v1(
            ABI_VERSION,
            path.as_ptr() as usize as u64,
            input_len(path)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    if status == 3 {
        return Ok(None);
    }
    if status != 0 {
        return Err(if status == 1 {
            MojoError::InvalidInput
        } else {
            MojoError::InvalidOutput
        });
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    if bytes.is_empty() {
        return Ok(None);
    }
    String::from_utf8(bytes.to_vec())
        .map(Some)
        .map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_map_kernel_matches_module_helpers() {
        for (text, expected) in [
            ("#[cfg(test)]\npub mod alpha {", true),
            ("// comment\n export default class Widget {", true),
            ("\u{3000}# comment\n\u{2003}pub(super) mod beta;", true),
            ("@decorator\nasync fn run() {}", false),
            ("pub async fn run() {}", false),
            ("pub fn run() {}", false),
            ("\n\r\nclass Widget:", true),
        ] {
            assert_eq!(repo_module_like(text).unwrap(), expected, "{text:?}");
        }

        for (path, expected) in [
            ("src/foo/mod.rs", Some("foo".to_string())),
            ("src/lib.rs", Some("lib".to_string())),
            ("a/src/api/client.ts", Some("api::client".to_string())),
            ("b/app/index.js", Some("app".to_string())),
            ("./pkg/main.py", Some("pkg".to_string())),
            ("src\\api\\mod.rs", Some("api".to_string())),
            ("\"a/src/foo.rs\"", Some("a::src::foo".to_string())),
            ("dir.name/file", Some("dir".to_string())),
            (".", None),
            ("", None),
        ] {
            assert_eq!(repo_module_from_path(path).unwrap(), expected, "{path:?}");
        }
    }
}
