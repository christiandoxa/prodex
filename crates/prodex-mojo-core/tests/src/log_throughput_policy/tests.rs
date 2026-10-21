use super::*;

#[test]
fn compiled_log_retention_scanner_classifies_case_sensitive_names() {
    use LogRetentionFileKind::{Ignore, Lock, Log};
    for (name, prefix, expected) in [
        ("prodex-runtime-1.log", "prodex-runtime", Log),
        ("prodex-runtime-1.lock", "prodex-runtime", Lock),
        ("prodex-runtime-1.log.lock", "prodex-runtime", Lock),
        ("prodex-runtime-1.lock.log", "prodex-runtime", Log),
        ("prodex-runtime-1.LOG", "prodex-runtime", Ignore),
        ("other-prodex-runtime-1.log", "prodex-runtime", Ignore),
        ("prodex-runtime", "prodex-runtime", Ignore),
        ("prodex-runtime-雪.log", "prodex-runtime", Log),
        ("prodex-runtime-雪.lock", "prodex-runtime", Lock),
        ("x.log", "", Log),
        (".lock", "", Lock),
        (".log", "a", Ignore),
        ("", "", Ignore),
    ] {
        assert_eq!(
            log_scanned_basename_kind(name, prefix),
            Ok(expected),
            "{name}"
        );
    }
}

#[test]
fn compiled_log_retention_scanner_rejects_invalid_abi_without_writes() {
    let mut result = [53_u64; 4];
    let target = result.as_mut_ptr() as usize as u64;
    let name = b"app.log";
    let prefix = b"app";
    for (version, name_length, prefix_length, expected) in
        [(2, 7, 3, 4), (1, u64::MAX, 3, 1), (1, 7, u64::MAX, 1)]
    {
        let status_code = unsafe {
            prodex_log_retention_policy_v1(
                version,
                RETENTION_SCAN_BASENAME,
                name.as_ptr() as usize as u64,
                name_length,
                prefix.as_ptr() as usize as u64,
                prefix_length,
                0,
                0,
                target,
            )
        };
        assert_eq!(status_code, expected);
        if expected == 4 {
            assert_eq!(result, [53; 4]);
        }
        result = [53; 4];
    }
}

#[test]
fn throughput_policy_kernel_preserves_counter_and_rate_contract() {
    assert_eq!(
        sample_plan(Some((100, 1000)), 99, 1100).unwrap(),
        ThroughputSamplePlan {
            counter_reset: true,
            append_sample: true,
        }
    );
    assert_eq!(
        sample_plan(Some((100, 1000)), 100, 1100).unwrap(),
        ThroughputSamplePlan {
            counter_reset: false,
            append_sample: false,
        }
    );
    assert_eq!(
        sample_plan(Some((100, 1000)), 101, 900).unwrap(),
        ThroughputSamplePlan {
            counter_reset: true,
            append_sample: true,
        }
    );
    assert_eq!(completed_rate(0, 1000).unwrap(), None);
    assert_eq!(completed_rate(100, 0).unwrap(), None);
    assert_eq!(completed_rate(100, 2000).unwrap(), Some(50.0));
    assert_eq!(stream_rate(100, 1000, 200, 3000).unwrap(), Some(50.0));
    assert_eq!(stream_rate(100, 1000, 110, 1200).unwrap(), None);
    assert_eq!(stream_rate(200, 1000, 100, 3000).unwrap(), None);
    assert_eq!(stream_rate(100, 3000, 200, 1000).unwrap(), None);
    assert_eq!(bounded_policy_value(Some(9), 5, 1, 8).unwrap(), 5);
    assert_eq!(bounded_policy_value(Some(7), 5, 1, 8).unwrap(), 7);
    assert_eq!(bounded_text_policy_value(None, 5, 1, 8).unwrap(), 5);
    assert_eq!(bounded_text_policy_value(Some("7"), 5, 1, 8).unwrap(), 7);
    assert_eq!(bounded_text_policy_value(Some("+7"), 5, 1, 8).unwrap(), 7);
    assert_eq!(bounded_text_policy_value(Some(" 7"), 5, 1, 8).unwrap(), 5);
    assert_eq!(bounded_text_policy_value(Some("7 "), 5, 1, 8).unwrap(), 5);
    assert_eq!(bounded_text_policy_value(Some("-1"), 5, 1, 8).unwrap(), 5);
    assert_eq!(bounded_text_policy_value(Some(""), 5, 1, 8).unwrap(), 5);
    assert_eq!(bounded_text_policy_value(Some("09"), 5, 1, 10).unwrap(), 9);
    assert_eq!(
        bounded_text_policy_value(Some("18446744073709551615"), 5, 1, u64::MAX).unwrap(),
        u64::MAX
    );
    assert_eq!(
        bounded_text_policy_value(Some("18446744073709551616"), 5, 1, u64::MAX).unwrap(),
        5
    );
    assert_eq!(
        log_rotation_plan(10, 5, 12).unwrap(),
        LogRotationPlan {
            rotate_before_write: true,
            rotate_after_oversized_line: false,
        }
    );
}

#[test]
fn runtime_log_retention_abi_selects_expired_and_oldest_budget_candidates() {
    let candidates = [
        LogRetentionCandidate {
            file_name: b"prodex-runtime-z-old.log",
            size: 6,
            modified_epoch_seconds: 99,
            removable: true,
            unavailable: false,
        },
        LogRetentionCandidate {
            file_name: b"prodex-runtime-b-cutoff.log",
            size: 5,
            modified_epoch_seconds: 100,
            removable: true,
            unavailable: false,
        },
        LogRetentionCandidate {
            file_name: b"prodex-runtime-c-new.log",
            size: 4,
            modified_epoch_seconds: 101,
            removable: true,
            unavailable: false,
        },
        LogRetentionCandidate {
            file_name: b"prodex-runtime-a-protected.log",
            size: 3,
            modified_epoch_seconds: 0,
            removable: false,
            unavailable: false,
        },
    ];
    assert_eq!(
        log_expired_candidate_plan(&candidates, 100).unwrap(),
        [true, false, false, false]
    );
    assert_eq!(log_expired_candidate_plan(&[], 100).unwrap(), []);

    let unordered = [
        LogRetentionCandidate {
            file_name: b"prodex-runtime-z-new.log",
            size: 6,
            modified_epoch_seconds: 5,
            removable: false,
            unavailable: false,
        },
        LogRetentionCandidate {
            file_name: b"prodex-runtime-b-old.log",
            size: 5,
            modified_epoch_seconds: 2,
            removable: true,
            unavailable: false,
        },
        LogRetentionCandidate {
            file_name: b"prodex-runtime-a-removed.log",
            size: 4,
            modified_epoch_seconds: 3,
            removable: true,
            unavailable: true,
        },
        LogRetentionCandidate {
            file_name: b"prodex-runtime-c-protected.log",
            size: 3,
            modified_epoch_seconds: 4,
            removable: false,
            unavailable: false,
        },
        LogRetentionCandidate {
            file_name: b"prodex-runtime-a-oldest.log",
            size: 2,
            modified_epoch_seconds: 1,
            removable: true,
            unavailable: false,
        },
    ];
    assert_eq!(
        log_over_budget_candidate_plan(&unordered, 4, 3, 16, 10).unwrap(),
        [4, 1]
    );

    let tied = [
        LogRetentionCandidate {
            file_name: b"prodex-runtime-b.log",
            size: 6,
            modified_epoch_seconds: 1,
            removable: true,
            unavailable: false,
        },
        LogRetentionCandidate {
            file_name: b"prodex-runtime-a.log",
            size: 5,
            modified_epoch_seconds: 1,
            removable: true,
            unavailable: false,
        },
    ];
    assert_eq!(
        log_over_budget_candidate_plan(&tied, 2, 1, 11, 6).unwrap(),
        [1]
    );

    let unicode_tie = [
        LogRetentionCandidate {
            file_name: "prodex-runtime-Ω.log".as_bytes(),
            size: 1,
            modified_epoch_seconds: 7,
            removable: true,
            unavailable: false,
        },
        LogRetentionCandidate {
            file_name: "prodex-runtime-é.log".as_bytes(),
            size: 1,
            modified_epoch_seconds: 7,
            removable: true,
            unavailable: false,
        },
    ];
    assert_eq!(
        log_over_budget_candidate_plan(&unicode_tie, 2, 1, 2, 1).unwrap(),
        [1]
    );
}

#[test]
fn runtime_log_retention_abi_rejects_invalid_candidate_flags() {
    assert_eq!(
        retention_candidates_abi_call(
            RETENTION_OVER_BUDGET_CANDIDATES,
            &[1, 0, 4, 0, 1],
            b"a",
            [0, 1, 1, 1, 1],
        ),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        log_over_budget_candidate_plan(
            &[LogRetentionCandidate {
                file_name: b"",
                size: 1,
                modified_epoch_seconds: 0,
                removable: true,
                unavailable: false,
            }],
            1,
            1,
            1,
            1,
        ),
        Err(MojoError::InvalidInput)
    );
}
