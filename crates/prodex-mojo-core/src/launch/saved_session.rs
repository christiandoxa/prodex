//! Mojo-owned, append-only saved-session exit guidance.
use crate::MojoError;

unsafe extern "C" {
    fn prodex_launch_saved_session_guidance_v1(
        abi: i64,
        session_address: u64,
        session_length: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

/// The host supplies a canonical UUID or no identity; Mojo renders every line.
/// There is no Rust formatter or fallback recomputation.
pub fn saved_session_guidance(session_id: Option<&str>) -> Result<String, MojoError> {
    let session_id = session_id.unwrap_or("");
    let mut output = [0_u8; 512];
    let written = unsafe {
        prodex_launch_saved_session_guidance_v1(
            1,
            session_id.as_ptr() as usize as u64,
            i64::try_from(session_id.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            output.len() as i64,
        )
    };
    match written {
        -1 => return Err(MojoError::InvalidInput),
        -3 => return Err(MojoError::Capacity),
        -4 => return Err(MojoError::AbiMismatch),
        _ => {}
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "01900000-0000-7000-8000-000000000712";

    #[test]
    fn saved_session_guidance_keeps_copyable_uuid_and_command_without_cursor_motion() {
        let text = saved_session_guidance(Some(ID)).unwrap();
        assert!(text.starts_with("\r\nProdex saved session\r\n"));
        assert!(text.contains(&format!("\r\nSession ID: {ID}\r\n")));
        assert!(text.contains(&format!("\r\nResume: prodex s {ID}\r\n")));
        assert!(!text.contains('\x1b'));
        assert!(!text.contains("unix://"));
        assert!(text.ends_with("Paused goals stay paused until /goal resume.\r\n"));
        for (i, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                assert_eq!(text.as_bytes()[i - 1], b'\r');
            }
        }
    }

    #[test]
    fn saved_session_guidance_unknown_id_uses_picker_without_guessing() {
        let text = saved_session_guidance(None).unwrap();
        assert!(text.contains("Session ID was not reported by this launch."));
        assert!(text.contains("\r\nResume: prodex s resume\r\n"));
        assert!(!text.contains("Session ID:"));
        assert!(!text.contains("--last"));
    }

    #[test]
    fn saved_session_guidance_rejects_invalid_uuid_abi_and_buffer() {
        assert_eq!(
            saved_session_guidance(Some("not-a-session")),
            Err(MojoError::InvalidInput)
        );
        let mut invalid = ID.to_string();
        invalid.replace_range(0..1, "\x1b");
        assert_eq!(
            saved_session_guidance(Some(&invalid)),
            Err(MojoError::InvalidInput)
        );
        let mut output = [99_u8; 512];
        unsafe {
            assert_eq!(
                prodex_launch_saved_session_guidance_v1(
                    2,
                    0,
                    0,
                    output.as_mut_ptr() as usize as u64,
                    512
                ),
                -4
            );
            assert_eq!(
                prodex_launch_saved_session_guidance_v1(
                    1,
                    0,
                    0,
                    output.as_mut_ptr() as usize as u64,
                    511
                ),
                -3
            );
            assert_eq!(
                prodex_launch_saved_session_guidance_v1(
                    1,
                    0,
                    36,
                    output.as_mut_ptr() as usize as u64,
                    512
                ),
                -1
            );
        }
        assert_eq!(output, [99; 512]);
    }
}
