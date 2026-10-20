use super::*;

fn sample<'a>(
    active: bool,
    rank: i64,
    epoch: i64,
    profile: &'a str,
    auth: &'a str,
    account: &'a str,
    plan: &'a str,
) -> QuotaReportSortBatchInput<'a> {
    QuotaReportSortBatchInput {
        active,
        status_rank: rank,
        reset_epoch: epoch,
        profile,
        auth,
        account,
        plan,
    }
}

fn pairwise_reference(sort: i64, rows: &[QuotaReportSortBatchInput<'_>]) -> Vec<usize> {
    use std::cmp::Ordering;
    let mut indices = (0..rows.len()).collect::<Vec<_>>();
    // Test-only legacy order oracle: the compiled pairwise comparison is
    // checked against the compiled batch for every sort mode.
    indices.sort_by(|&left, &right| {
        let lhs = &rows[left];
        let rhs = &rows[right];
        let code = quota_report_compare(
            sort,
            lhs.active,
            rhs.active,
            lhs.status_rank,
            rhs.status_rank,
            lhs.reset_epoch,
            rhs.reset_epoch,
            lhs.profile,
            rhs.profile,
            lhs.auth,
            rhs.auth,
            lhs.account,
            rhs.account,
            lhs.plan,
            rhs.plan,
        )
        .unwrap();
        let result = match code {
            -1 => Ordering::Less,
            0 => Ordering::Equal,
            1 => Ordering::Greater,
            _ => panic!("compiled comparator returned an impossible result"),
        };
        result.then_with(|| lhs.profile.cmp(rhs.profile))
    });
    indices
}

#[test]
fn compiled_quota_report_sort_batch_matches_pairwise_mojo_for_all_six_modes() {
    let rows = [
        sample(false, 2, 20, "zeta", "Api", "x@example.com", "Pro"),
        sample(true, 0, i64::MAX, "alpha", "OAuth", "z@example.com", "Free"),
        sample(false, 1, 10, "東京", "api", "a@example.com", "pro"),
        sample(false, 1, 10, "beta", "Oauth", "x@example.com", "plus"),
        sample(false, 1, 10, "Beta", "oauth", "X@example.com", "PLUS"),
        sample(true, 1, 9, "雪", "other", "", ""),
        sample(false, 0, i64::MIN, "", "", "", ""),
        sample(false, 1, 10, "beta", "Oauth", "x@example.com", "plus"),
    ];
    for mode in 0..=5 {
        let expected = pairwise_reference(mode, &rows);
        let actual = quota_report_sort_batch(mode, &rows).unwrap();
        assert_eq!(
            actual, expected,
            "quota mode {mode} changed pairwise precedence"
        );
        let mut sorted = actual.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..rows.len()).collect::<Vec<_>>());
        // Identical full rows must preserve their original relative position.
        let before = actual.iter().position(|index| *index == 3).unwrap();
        let after = actual.iter().position(|index| *index == 7).unwrap();
        assert!(
            before < after,
            "stable original order changed in mode {mode}"
        );
    }
    assert_eq!(quota_report_sort_batch(0, &[]), Ok(vec![]));
    assert_eq!(
        quota_report_sort_batch(6, &rows),
        Err(crate::MojoError::InvalidInput)
    );
    assert_eq!(
        quota_report_sort_batch(-1, &rows),
        Err(crate::MojoError::InvalidInput)
    );
    assert_eq!(
        quota_report_sort_batch(1, &[sample(false, -1, 0, "", "", "", "")]),
        Err(crate::MojoError::InvalidInput),
    );
}

#[test]
fn compiled_quota_report_sort_batch_retains_total_order_for_thousands_of_rows() {
    let names = (0..2_049)
        .map(|index| format!("profile-{index:04}"))
        .collect::<Vec<_>>();
    let rows = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            sample(
                index % 13 == 0,
                (index % 5) as i64,
                (index % 7) as i64 - 3,
                name,
                if index % 2 == 0 { "ZOOM" } else { "zoom" },
                if index % 3 == 0 { "delta" } else { "echo" },
                if index % 4 == 0 { "Pro" } else { "free" },
            )
        })
        .collect::<Vec<_>>();
    for mode in 0..6 {
        let actual = quota_report_sort_batch(mode, &rows).unwrap();
        assert_eq!(actual.len(), names.len(), "mode {mode}");
        let mut seen = vec![false; actual.len()];
        for index in actual.iter().copied() {
            assert!(!std::mem::replace(&mut seen[index], true));
        }
        // Check every adjacent pair against the original compiled pairwise
        // comparator; the production caller no longer sorts in Rust.
        for pair in actual.windows(2) {
            let lhs = rows[pair[0]];
            let rhs = rows[pair[1]];
            let code = quota_report_compare(
                mode,
                lhs.active,
                rhs.active,
                lhs.status_rank,
                rhs.status_rank,
                lhs.reset_epoch,
                rhs.reset_epoch,
                lhs.profile,
                rhs.profile,
                lhs.auth,
                rhs.auth,
                lhs.account,
                rhs.account,
                lhs.plan,
                rhs.plan,
            )
            .unwrap();
            assert!(
                code < 0 || (code == 0 && lhs.profile <= rhs.profile),
                "unexpected order for mode {mode}: pair {pair:?}, code {code}",
            );
        }
    }
}

#[test]
fn compiled_quota_report_sort_batch_abi_rejects_late_bad_row_before_output_writes() {
    let names = ["one", "two"];
    let mut rows = [0_i64; 22];
    for (index, name) in names.iter().enumerate() {
        let start = index * 11;
        rows[start] = 1;
        rows[start + 1] = 0;
        rows[start + 3] = name.as_ptr() as usize as i64;
        rows[start + 4] = name.len() as i64;
    }
    let mut result = [73_i64; 2];
    let address = rows.as_ptr() as usize as u64;
    let dest = result.as_mut_ptr() as usize as u64;
    for (version, mode, source, count, destination, capacity, expected) in [
        (2, 1, address, 2, dest, 2, 4),
        (1, 6, address, 2, dest, 2, 1),
        (1, 0, 0, 2, dest, 2, 1),
        (1, 0, address, -1, dest, 2, 1),
        (1, 0, address, 1_000_001, dest, 2, 1),
        (1, 0, address, 2, dest, 1, 1),
        (1, 0, address, 2, 0, 2, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_quota_report_sort_batch_v1(
                    version,
                    mode,
                    source,
                    count,
                    destination,
                    capacity,
                )
            },
            expected,
        );
        assert_eq!(result, [73; 2]);
    }
    rows[11 + 1] = -1;
    assert_eq!(
        unsafe {
            prodex_quota_report_sort_batch_v1(1, 1, rows.as_ptr() as usize as u64, 2, dest, 2)
        },
        1,
    );
    assert_eq!(result, [73; 2]);
    rows[11 + 1] = 0;
    rows[11 + 4] = -1;
    assert_eq!(
        unsafe {
            prodex_quota_report_sort_batch_v1(1, 1, rows.as_ptr() as usize as u64, 2, dest, 2)
        },
        1,
    );
    assert_eq!(result, [73; 2]);
}
