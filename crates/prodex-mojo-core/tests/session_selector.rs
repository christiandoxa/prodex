#![cfg(any(feature = "mojo-rich", feature = "mojo-runtime"))]

#[repr(C)]
#[derive(Clone, Copy)]
struct View {
    address: u64,
    length: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RecentWorkspaceCandidate {
    modified_epoch: i64,
    absolute: i64,
    workspace: View,
}

unsafe extern "C" {
    fn prodex_session_report_query_matches_v1(
        abi_version: i64,
        query_address: u64,
        query_length: i64,
        fields_address: u64,
        fields_count: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_recent_workspace_plan_v1(
        abi_version: i64,
        candidates_address: u64,
        candidates_count: i64,
        max_workspaces: i64,
        output_address: u64,
        output_capacity: i64,
        scratch_address: u64,
        scratch_capacity: i64,
    ) -> i64;
}

fn view(value: &str) -> View {
    View {
        address: value.as_ptr() as u64,
        length: value.len() as u64,
    }
}

#[test]
fn direct_query_abi_matches_normalized_unicode_and_rejects_wrong_field_count() {
    let query = "i\u{307}stanbul";
    let values = [
        "01900000-0000-7000-8000-000000000301",
        "straße i\u{307}stanbul",
        "/home/test-user/workspace",
        "main",
        "openai",
        "/home/test-user/.codex/sessions/unicode.jsonl",
    ];
    let fields = values.map(view);
    let mut output = [-1_i64; 1];
    let status = unsafe {
        prodex_session_report_query_matches_v1(
            1,
            query.as_ptr() as u64,
            query.len() as i64,
            fields.as_ptr() as u64,
            fields.len() as i64,
            output.as_mut_ptr() as u64,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(output, [1]);

    let mut invalid_output = [-1_i64; 1];
    let invalid_status = unsafe {
        prodex_session_report_query_matches_v1(
            1,
            query.as_ptr() as u64,
            query.len() as i64,
            fields.as_ptr() as u64,
            (fields.len() - 1) as i64,
            invalid_output.as_mut_ptr() as u64,
        )
    };
    assert_eq!(invalid_status, 1);
    assert_eq!(invalid_output, [-1]);

    let borrowed = values.map(Some);
    assert!(prodex_mojo_core::json::session_report_query_matches(query, &borrowed).unwrap());
}

#[test]
fn recent_workspace_abi_orders_deduplicates_and_limits_candidates() {
    let workspace_a = "/home/test-user/workspace-a";
    let workspace_b = "/home/test-user/workspace-b";
    let relative = "relative/workspace";
    let workspace_c = "/home/test-user/workspace-c";
    let candidates = [
        RecentWorkspaceCandidate {
            modified_epoch: 10,
            absolute: 1,
            workspace: view(workspace_a),
        },
        RecentWorkspaceCandidate {
            modified_epoch: 30,
            absolute: 1,
            workspace: view(workspace_b),
        },
        RecentWorkspaceCandidate {
            modified_epoch: 20,
            absolute: 1,
            workspace: view(workspace_a),
        },
        RecentWorkspaceCandidate {
            modified_epoch: 40,
            absolute: 0,
            workspace: view(relative),
        },
        RecentWorkspaceCandidate {
            modified_epoch: 20,
            absolute: 1,
            workspace: view(workspace_c),
        },
    ];
    let mut output = [-1_i64; 4];
    let mut scratch = [-1_i64; 5];
    let status = unsafe {
        prodex_session_recent_workspace_plan_v1(
            1,
            candidates.as_ptr() as u64,
            candidates.len() as i64,
            3,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            scratch.as_mut_ptr() as u64,
            scratch.len() as i64,
        )
    };
    assert_eq!(status, 0);
    assert_eq!(output, [3, 1, 2, 4]);

    let plan = prodex_mojo_core::json::session_recent_workspace_plan(
        &[
            prodex_mojo_core::json::SessionRecentWorkspaceCandidate {
                modified_epoch: 10,
                absolute: true,
                workspace: workspace_a,
            },
            prodex_mojo_core::json::SessionRecentWorkspaceCandidate {
                modified_epoch: 30,
                absolute: true,
                workspace: workspace_b,
            },
            prodex_mojo_core::json::SessionRecentWorkspaceCandidate {
                modified_epoch: 20,
                absolute: true,
                workspace: workspace_a,
            },
            prodex_mojo_core::json::SessionRecentWorkspaceCandidate {
                modified_epoch: 40,
                absolute: false,
                workspace: relative,
            },
            prodex_mojo_core::json::SessionRecentWorkspaceCandidate {
                modified_epoch: 20,
                absolute: true,
                workspace: workspace_c,
            },
        ],
        2,
    )
    .unwrap();
    assert_eq!(plan, [1, 2]);

    let mut short_output = [-1_i64; 1];
    assert_eq!(
        unsafe {
            prodex_session_recent_workspace_plan_v1(
                1,
                candidates.as_ptr() as u64,
                candidates.len() as i64,
                3,
                short_output.as_mut_ptr() as u64,
                short_output.len() as i64,
                scratch.as_mut_ptr() as u64,
                scratch.len() as i64,
            )
        },
        1
    );
    assert_eq!(short_output, [-1]);
}
