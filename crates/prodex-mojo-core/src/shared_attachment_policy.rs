use crate::MojoError;

const ABI_VERSION: i64 = 1;
const IMAGE_TAG_RANGE: i64 = 0;
const CLIPBOARD_RANGE: i64 = 1;
const ATTACHMENT_RANGE: i64 = 2;
const CLIPBOARD_NAME: i64 = 3;
const PERSISTABLE_NAME: i64 = 4;
const ROLLOUT_NAME: i64 = 5;

unsafe extern "C" {
    fn prodex_shared_attachment_policy_v1(
        abi_version: i64,
        operation: i64,
        input_address: u64,
        input_length: i64,
        cursor: i64,
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

fn call(operation: i64, input: &str, cursor: usize) -> Result<[i64; 3], MojoError> {
    let mut output = [0_i64; 3];
    status(unsafe {
        prodex_shared_attachment_policy_v1(
            ABI_VERSION,
            operation,
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(cursor).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(output)
}

fn optional_range(
    output: [i64; 3],
    input: &str,
    minimum_start: usize,
) -> Result<Option<(usize, usize)>, MojoError> {
    match output[0] {
        0 => Ok(None),
        1 => {
            let start = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
            if start < minimum_start
                || start > end
                || end > input.len()
                || !input.is_char_boundary(start)
                || !input.is_char_boundary(end)
            {
                return Err(MojoError::InvalidOutput);
            }
            Ok(Some((start, end)))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

fn bool_output(output: [i64; 3]) -> Result<bool, MojoError> {
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn image_tag_path_range(tag: &str) -> Result<Option<(usize, usize)>, MojoError> {
    optional_range(call(IMAGE_TAG_RANGE, tag, 0)?, tag, 0)
}

pub fn next_clipboard_path(
    contents: &str,
    cursor: usize,
) -> Result<Option<(usize, usize)>, MojoError> {
    optional_range(call(CLIPBOARD_RANGE, contents, cursor)?, contents, cursor)
}

pub fn next_attachment_path(
    contents: &str,
    cursor: usize,
) -> Result<Option<(usize, usize)>, MojoError> {
    optional_range(call(ATTACHMENT_RANGE, contents, cursor)?, contents, cursor)
}

pub fn clipboard_file_name(file_name: &str) -> Result<bool, MojoError> {
    bool_output(call(CLIPBOARD_NAME, file_name, 0)?)
}

pub fn persistable_attachment_file_name(file_name: &str) -> Result<bool, MojoError> {
    bool_output(call(PERSISTABLE_NAME, file_name, 0)?)
}

pub fn rollout_file_name(file_name: &str) -> Result<bool, MojoError> {
    bool_output(call(ROLLOUT_NAME, file_name, 0)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_attachment_policy_preserves_scanner_contracts() {
        assert_eq!(
            image_tag_path_range("<image path=\\\"/tmp/codex-clipboard-a.png\\\"").unwrap(),
            Some((14, 40))
        );
        assert_eq!(
            image_tag_path_range(r#"<image path="/tmp/codex-clipboard-a.png""#).unwrap(),
            Some((13, 39))
        );

        let clipboard = "see /tmp/codex-clipboard-a.png.";
        let range = next_clipboard_path(clipboard, 0).unwrap().unwrap();
        assert_eq!(&clipboard[range.0..range.1], "/tmp/codex-clipboard-a.png");

        let attachment = r#"see /tmp/attachments/thread/image-a.png, next"#;
        let range = next_attachment_path(attachment, 0).unwrap().unwrap();
        assert_eq!(
            &attachment[range.0..range.1],
            "/tmp/attachments/thread/image-a.png"
        );

        let adjacent_escaped =
            r#"/tmp/attachments/first/image-1.png\n/tmp/attachments/second/image-2.png"#;
        let first = next_attachment_path(adjacent_escaped, 0).unwrap().unwrap();
        assert_eq!(
            &adjacent_escaped[first.0..first.1],
            "/tmp/attachments/first/image-1.png"
        );
        let second = next_attachment_path(adjacent_escaped, first.1)
            .unwrap()
            .unwrap();
        assert!(second.0 >= first.1);
        assert_eq!(
            &adjacent_escaped[second.0..second.1],
            "/tmp/attachments/second/image-2.png"
        );

        assert!(clipboard_file_name("codex-clipboard-a.png").unwrap());
        assert!(persistable_attachment_file_name("pasted-text-1.txt").unwrap());
        assert!(persistable_attachment_file_name("image-1.png").unwrap());
        assert!(persistable_attachment_file_name("goal-objective.md").unwrap());
        assert!(!persistable_attachment_file_name("other.txt").unwrap());
        assert!(rollout_file_name("rollout.jsonl").unwrap());
        assert!(rollout_file_name("rollout.jsonl.zst").unwrap());
        assert!(!rollout_file_name("rollout.json").unwrap());
    }
}
