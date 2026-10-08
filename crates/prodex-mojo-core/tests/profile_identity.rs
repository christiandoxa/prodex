#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::profile_identity::{
    ProfileManagementRowStatus, ProfileManagementScreenStatus, ProfileManagementStatusInput,
    profile_management_status,
};

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
        .map(
            |input| match (input.active, input.managed, input.identity_present) {
                (true, true, true) => ProfileManagementRowStatus::ActiveManagedWithIdentity,
                (true, true, false) => ProfileManagementRowStatus::ActiveManagedWithoutIdentity,
                (false, true, true) => ProfileManagementRowStatus::InactiveManagedWithIdentity,
                (false, true, false) => ProfileManagementRowStatus::InactiveManagedWithoutIdentity,
                (true, false, true) => ProfileManagementRowStatus::ActiveExternalWithIdentity,
                (true, false, false) => ProfileManagementRowStatus::ActiveExternalWithoutIdentity,
                (false, false, true) => ProfileManagementRowStatus::InactiveExternalWithIdentity,
                (false, false, false) => {
                    ProfileManagementRowStatus::InactiveExternalWithoutIdentity
                }
            },
        )
        .collect();
    (screen, rows)
}

#[test]
fn profile_management_status_has_rust_parity_across_the_policy_matrix() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) }

    for active_profile_present in [false, true] {
        for count in 0..=3 {
            let inputs = (0..count)
                .map(|index| ProfileManagementStatusInput {
                    active: index % 2 == 0,
                    managed: index % 3 != 0,
                    identity_present: index % 2 != 0,
                })
                .collect::<Vec<_>>();
            let expected = rust_status_oracle(active_profile_present, &inputs);
            let actual = profile_management_status(active_profile_present, &inputs).unwrap();
            assert_eq!((actual.screen, actual.rows), expected);
        }
    }
}

#[test]
fn profile_management_status_mutation_proof_keeps_each_status_input_live() {
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
    assert_ne!(
        profile_management_status(false, &active).unwrap().rows,
        base_plan.rows
    );
    assert_ne!(
        profile_management_status(false, &external).unwrap().rows,
        base_plan.rows
    );
    assert_ne!(
        profile_management_status(false, &missing_identity)
            .unwrap()
            .rows,
        base_plan.rows
    );
}
