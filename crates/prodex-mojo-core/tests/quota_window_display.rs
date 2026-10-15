#![cfg(feature = "mojo-quota")]

unsafe extern "C" {
    fn prodex_quota_window_text_v1(
        abi_version: i64,
        mode: i64,
        first_present: i64,
        first_label_address: u64,
        first_label_length: i64,
        first_used_present: i64,
        first_used: i64,
        first_reset_present: i64,
        first_reset_address: u64,
        first_reset_length: i64,
        second_present: i64,
        second_label_address: u64,
        second_label_length: i64,
        second_used_present: i64,
        second_used: i64,
        second_reset_present: i64,
        second_reset_address: u64,
        second_reset_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

#[test]
fn required_mojo_window_text_abi_rejects_bad_version_and_capacity() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) };
    let mut written = -1_i64;
    let invalid_version = unsafe {
        prodex_quota_window_text_v1(
            99,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    assert_eq!(invalid_version, 4);

    let label = b"5h";
    let reset = b"-";
    let mut output = [0_u8; 1];
    let capacity = unsafe {
        prodex_quota_window_text_v1(
            1,
            1,
            1,
            label.as_ptr() as usize as u64,
            label.len() as i64,
            1,
            100,
            1,
            reset.as_ptr() as usize as u64,
            reset.len() as i64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            output.as_mut_ptr() as usize as u64,
            output.len() as i64,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    assert_eq!(capacity, 2);
}
