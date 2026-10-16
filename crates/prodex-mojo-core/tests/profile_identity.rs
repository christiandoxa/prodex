#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::profile_identity::{
    ProfileManagementRowStatus, ProfileManagementScreenStatus, ProfileManagementStatusInput,
    profile_management_status,
};

unsafe extern "C" {
    fn prodex_mojo_profile_identity_v2(
        abi_version: i64,
        operation: i64,
        primary_address: u64,
        primary_length: i64,
        secondary_address: u64,
        secondary_length: i64,
        flags: i64,
        records_address: u64,
        record_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        result_address: u64,
    ) -> i64;
    fn prodex_profile_management_status_v1(
        abi_version: i64,
        active_profile_present: i64,
        profile_count: i64,
        flags_address: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn raw_profile_identity_call(
    operation: i64,
    primary: &[u8],
    secondary: &[u8],
    flags: i64,
    output: &mut [u8],
    written: &mut i64,
    result: &mut i64,
) -> i64 {
    raw_profile_identity_call_with_abi(
        2, operation, primary, secondary, flags, output, written, result,
    )
}

fn raw_profile_identity_call_with_abi(
    abi_version: i64,
    operation: i64,
    primary: &[u8],
    secondary: &[u8],
    flags: i64,
    output: &mut [u8],
    written: &mut i64,
    result: &mut i64,
) -> i64 {
    // SAFETY: test slices stay alive for the call and the declared lengths match their storage.
    unsafe {
        prodex_mojo_profile_identity_v2(
            abi_version,
            operation,
            primary.as_ptr() as usize as u64,
            primary.len() as i64,
            secondary.as_ptr() as usize as u64,
            secondary.len() as i64,
            flags,
            0,
            0,
            output.as_mut_ptr() as usize as u64,
            output.len() as i64,
            written as *mut i64 as usize as u64,
            result as *mut i64 as usize as u64,
        )
    }
}

#[test]
fn default_api_key_profile_name_abi_owns_prefix_fallback_and_validation() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) }

    let host = b"API.OPENAI.COM";
    let mut output = [0_u8; 64];
    let mut written = -1_i64;
    let mut result = -1_i64;
    assert_eq!(
        raw_profile_identity_call(22, host, &[], 1, &mut output, &mut written, &mut result,),
        0
    );
    assert_eq!(&output[..written as usize], b"api_key_api.openai.com");
    assert_eq!(result, 0);

    let mut output = [0_u8; 16];
    written = -1;
    result = -1;
    assert_eq!(
        raw_profile_identity_call(22, &[], &[], 0, &mut output, &mut written, &mut result,),
        0
    );
    assert_eq!(&output[..written as usize], b"api_key");

    let mut output = [91_u8; 3];
    written = 92;
    result = 93;
    assert_eq!(
        raw_profile_identity_call(22, host, &[], 1, &mut output, &mut written, &mut result,),
        2
    );
    assert_eq!(output, [91; 3]);
    assert_eq!(written, 0);
    assert_eq!(result, 0);

    let mut output = [91_u8; 64];
    written = 92;
    result = 93;
    assert_eq!(
        raw_profile_identity_call(22, host, &[], 2, &mut output, &mut written, &mut result,),
        1
    );
    assert_eq!(output, [91; 64]);
    assert_eq!(written, 0);
    assert_eq!(result, 0);

    written = 92;
    result = 93;
    assert_eq!(
        raw_profile_identity_call_with_abi(
            1,
            22,
            host,
            &[],
            1,
            &mut output,
            &mut written,
            &mut result,
        ),
        4
    );
    assert_eq!(written, 92);
    assert_eq!(result, 93);
}

#[test]
fn profile_import_email_abi_preserves_auth_and_rejects_invalid_flags() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) }

    let primary = b"auth@example.com";
    let fallback = "\u{2003} fallback@example.com \u{3000}".as_bytes();
    let mut output = [0_u8; 64];
    let mut written = -1_i64;
    let mut result = -1_i64;
    assert_eq!(
        raw_profile_identity_call(
            21,
            primary,
            fallback,
            1,
            &mut output,
            &mut written,
            &mut result,
        ),
        0
    );
    assert_eq!(&output[..written as usize], primary);
    assert_eq!(result, 1);

    let mut output = [0_u8; 64];
    written = -1;
    result = -1;
    assert_eq!(
        raw_profile_identity_call(21, &[], fallback, 2, &mut output, &mut written, &mut result,),
        0
    );
    assert_eq!(&output[..written as usize], b"fallback@example.com");
    assert_eq!(result, 1);

    let mut output = [91_u8; 8];
    written = 92;
    result = 93;
    assert_eq!(
        raw_profile_identity_call(21, &[], fallback, 4, &mut output, &mut written, &mut result,),
        1
    );
    assert_eq!(output, [91; 8]);
    assert_eq!(written, 0);
    assert_eq!(result, 0);
}

fn rust_status_oracle(
    active_profile_present: bool,
    inputs: &[ProfileManagementStatusInput],
) -> (
    ProfileManagementScreenStatus,
    Vec<ProfileManagementRowStatus>,
) {
    let screen = if active_profile_present {
        ProfileManagementScreenStatus::Active
    } else if inputs.len() == 1 {
        ProfileManagementScreenStatus::OnlyProfile
    } else {
        ProfileManagementScreenStatus::NoActive
    };
    let rows = inputs
        .iter()
        .map(|input| ProfileManagementRowStatus {
            active: input.active,
            managed: input.managed,
            identity_present: input.identity_present,
        })
        .collect();
    (screen, rows)
}

fn raw_status_call(
    abi_version: i64,
    active_profile_present: i64,
    flags: &[i64],
    profile_count: i64,
    output: &mut [i64],
    output_capacity: i64,
    written: &mut i64,
) -> i64 {
    // SAFETY: test slices stay alive for the call and the declared lengths match their storage.
    unsafe {
        prodex_profile_management_status_v1(
            abi_version,
            active_profile_present,
            profile_count,
            flags.as_ptr() as usize as u64,
            output.as_mut_ptr() as usize as u64,
            output_capacity,
            written as *mut i64 as usize as u64,
        )
    }
}

#[test]
fn profile_management_status_matches_every_screen_and_row_case() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    for active_profile_present in [false, true] {
        for count in 0..=2 {
            let inputs = vec![
                ProfileManagementStatusInput {
                    active: true,
                    managed: true,
                    identity_present: true,
                };
                count
            ];
            let expected = rust_status_oracle(active_profile_present, &inputs);
            let actual = profile_management_status(active_profile_present, &inputs).unwrap();
            assert_eq!((actual.screen, actual.rows), expected);
        }
    }

    let inputs = (0..8)
        .map(|flags| ProfileManagementStatusInput {
            active: flags & 1 != 0,
            managed: flags & 2 != 0,
            identity_present: flags & 4 != 0,
        })
        .collect::<Vec<_>>();
    let actual = profile_management_status(false, &inputs).unwrap();
    assert_eq!(
        (actual.screen, actual.rows),
        rust_status_oracle(false, &inputs)
    );
}

#[test]
fn profile_management_status_mutations_change_the_mojo_result() {
    let base = [ProfileManagementStatusInput {
        active: false,
        managed: true,
        identity_present: true,
    }];
    let active = [ProfileManagementStatusInput {
        active: true,
        ..base[0]
    }];
    let external = [ProfileManagementStatusInput {
        managed: false,
        ..base[0]
    }];
    let missing_identity = [ProfileManagementStatusInput {
        identity_present: false,
        ..base[0]
    }];

    let base_plan = profile_management_status(false, &base).unwrap();
    assert_ne!(
        profile_management_status(true, &base).unwrap().screen,
        base_plan.screen
    );
    for changed in [active, external, missing_identity] {
        assert_ne!(
            profile_management_status(false, &changed).unwrap().rows,
            base_plan.rows
        );
    }
}

#[test]
fn profile_management_status_rejects_more_than_256_profiles() {
    let inputs = vec![
        ProfileManagementStatusInput {
            active: false,
            managed: true,
            identity_present: false,
        };
        257
    ];
    assert_eq!(
        profile_management_status(false, &inputs),
        Err(prodex_mojo_core::MojoError::InvalidInput)
    );
}

#[test]
fn profile_management_status_abi_rejects_bad_version_capacity_and_flags_without_writes() {
    const ABI_VERSION: i64 = 1;

    let flags = [1_i64, 1, 1];
    let mut output = [91_i64; 2];
    let mut written = 92_i64;
    let output_capacity = output.len() as i64;
    assert_eq!(
        raw_status_call(
            ABI_VERSION + 1,
            0,
            &flags,
            1,
            &mut output,
            output_capacity,
            &mut written,
        ),
        4
    );
    assert_eq!(output, [91, 91]);
    assert_eq!(written, 92);

    assert_eq!(
        raw_status_call(ABI_VERSION, 0, &flags, 1, &mut output, 1, &mut written,),
        2
    );
    assert_eq!(output, [91, 91]);
    assert_eq!(written, 92);

    let invalid_flags = [1_i64, 1, 1, 0, 1, 2];
    let mut output = [91_i64; 3];
    let output_capacity = output.len() as i64;
    assert_eq!(
        raw_status_call(
            ABI_VERSION,
            0,
            &invalid_flags,
            2,
            &mut output,
            output_capacity,
            &mut written,
        ),
        1
    );
    assert_eq!(output, [91, 91, 91]);
    assert_eq!(written, 92);
}
