#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::profile_identity::{
    ProfileManagementRowStatus, ProfileManagementScreenStatus, ProfileManagementStatusInput,
    profile_management_status,
};

unsafe extern "C" {
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
fn profile_management_status_accepts_more_than_256_profiles() {
    let inputs = vec![
        ProfileManagementStatusInput {
            active: false,
            managed: true,
            identity_present: false,
        };
        257
    ];
    let plan = profile_management_status(false, &inputs).unwrap();
    assert_eq!(plan.screen, ProfileManagementScreenStatus::NoActive);
    assert_eq!(plan.rows.len(), inputs.len());
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
