use super::render_native_json;
use crate::MojoError;

#[test]
fn native_json_metadata_preserves_payload_and_rejects_bad_presence_or_lengths() {
    let text = render_native_json(|phase, output, capacity, metadata| {
        // SAFETY: the runner supplies two metadata words and the measured output.
        unsafe {
            *(metadata as *mut [i64; 2]) = [1, 2];
            if phase == 0 {
                assert_eq!(capacity, 2);
                std::ptr::copy_nonoverlapping(b"[]".as_ptr(), output as *mut u8, 2);
            }
        }
        0
    })
    .unwrap();
    assert_eq!(text, "[]");
    for (present, length) in [(0, 0), (2, 0), (1, -1)] {
        assert_eq!(
            render_native_json(|_, _, _, metadata| {
                // SAFETY: the runner owns the two synchronous metadata words.
                unsafe {
                    *(metadata as *mut [i64; 2]) = [present, length];
                }
                0
            }),
            Err(MojoError::InvalidOutput)
        );
    }
    assert_eq!(
        render_native_json(|_, _, _, _| 4),
        Err(MojoError::AbiMismatch)
    );
}

#[test]
fn native_json_metadata_rejects_changed_write_length_and_invalid_utf8() {
    assert_eq!(
        render_native_json(|phase, _, _, metadata| {
            // SAFETY: the runner owns the two synchronous metadata words.
            unsafe {
                *(metadata as *mut [i64; 2]) = [1, if phase == 1 { 2 } else { 1 }];
            }
            0
        }),
        Err(MojoError::InvalidOutput)
    );
    assert_eq!(
        render_native_json(|phase, output, _, metadata| {
            // SAFETY: the runner supplies two metadata words and one output byte.
            unsafe {
                *(metadata as *mut [i64; 2]) = [1, 1];
                if phase == 0 {
                    *(output as *mut u8) = 255;
                }
            }
            0
        }),
        Err(MojoError::InvalidOutput)
    );
}
