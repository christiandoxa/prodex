use super::*;

#[test]
fn refresh_lease_acquires_owner_and_creates_hashed_lock() {
    let root = temp_dir("refresh-lease-owner");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let sensitive_key = "refresh-token-secret";
    let paths = coordinator.paths_for_key(sensitive_key);

    let decision = coordinator.acquire(sensitive_key).unwrap();
    assert_eq!(decision.role(), RefreshLeaseRole::Owner);

    match decision {
        RefreshLeaseDecision::Owner(owner) => {
            assert_eq!(owner.lock_path(), paths.lock_path());
            assert!(owner.lock_path().exists());
            assert!(
                !owner
                    .lock_path()
                    .display()
                    .to_string()
                    .contains(sensitive_key)
            );
            let lock_record = fs::read_to_string(owner.lock_path()).unwrap();
            assert!(lock_record.starts_with("prodex-refresh-lease\nversion=1\n"));
            let fence_token = lock_record
                .lines()
                .find_map(|line| line.strip_prefix("fence_token="))
                .expect("versioned lock should contain fence token");
            assert_eq!(fence_token.len(), 64);
            assert!(fence_token.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert!(
                !owner
                    .result_path()
                    .display()
                    .to_string()
                    .contains(sensitive_key)
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
                let metadata = fs::metadata(owner.lock_path()).unwrap();
                assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
                assert_eq!(metadata.uid(), fs::metadata(&root).unwrap().uid());
            }
        }
        other => panic!("expected owner, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_stale_owner_cannot_commit_or_remove_a_replacement_lock() {
    let root = temp_dir("refresh-lease-identity");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let sensitive_key = "identity-bound-refresh-token";
    let original = match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner,
        other => panic!("expected owner, got {other:?}"),
    };
    fs::remove_file(original.lock_path()).unwrap();
    let replacement = match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner,
        other => panic!("expected replacement owner, got {other:?}"),
    };

    let error = original
        .commit_result("{\"access_token\":\"stale-result\"}")
        .expect_err("stale owner must be fenced");
    assert!(error.is_ownership_lost());
    assert!(
        !coordinator
            .paths_for_key(sensitive_key)
            .result_path()
            .exists()
    );
    assert!(replacement.lock_path().exists());

    replacement
        .commit_result("{\"access_token\":\"replacement-result\"}")
        .unwrap();
    match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Follower { result_json } => assert_eq!(
            result_json.as_str(),
            "{\"access_token\":\"replacement-result\"}"
        ),
        other => panic!("expected replacement result, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_follower_reads_committed_result() {
    let root = temp_dir("refresh-lease-follower");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let sensitive_key = "access-token-secret";

    match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner
            .commit_result("{\"access_token\":\"redacted-result\"}")
            .unwrap(),
        other => panic!("expected owner, got {other:?}"),
    }

    match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Follower { result_json } => {
            assert_eq!(
                result_json.as_str(),
                "{\"access_token\":\"redacted-result\"}"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                assert_eq!(
                    fs::metadata(coordinator.paths_for_key(sensitive_key).result_path())
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    0o600
                );
            }
        }
        other => panic!("expected follower, got {other:?}"),
    }

    let result_record =
        fs::read_to_string(coordinator.paths_for_key(sensitive_key).result_path()).unwrap();
    assert!(result_record.starts_with("prodex-refresh-result\nversion=1\nfence_token="));
    assert!(result_record.ends_with("{\"access_token\":\"redacted-result\"}"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_reads_legacy_unversioned_result() {
    let root = temp_dir("refresh-lease-legacy-result");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let sensitive_key = "legacy-result-refresh-token";
    let result_path = coordinator
        .paths_for_key(sensitive_key)
        .result_path()
        .to_path_buf();
    fs::write(&result_path, "{\"access_token\":\"legacy-result\"}").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&result_path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Follower { result_json } => {
            assert_eq!(result_json.as_str(), "{\"access_token\":\"legacy-result\"}")
        }
        other => panic!("expected legacy follower result, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_ignores_oversized_result_file() {
    let root = temp_dir("refresh-lease-oversized-result");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let sensitive_key = "shared-refresh-token-secret";
    let paths = coordinator.paths_for_key(sensitive_key);
    let file = fs::File::create(paths.result_path()).unwrap();
    file.set_len(1024 * 1024 + 1).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(paths.result_path(), fs::Permissions::from_mode(0o600)).unwrap();
    }

    match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => {
            assert_eq!(owner.lock_path(), paths.lock_path());
            assert!(!paths.result_path().exists());
        }
        other => panic!("expected owner after oversized result cleanup, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_rejects_oversized_committed_result() {
    let root = temp_dir("refresh-lease-oversized-commit");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let sensitive_key = "shared-refresh-token-secret";
    let paths = coordinator.paths_for_key(sensitive_key);
    let owner = match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner,
        other => panic!("expected owner, got {other:?}"),
    };

    let err = owner
        .commit_result("x".repeat(1024 * 1024 + 1))
        .unwrap_err();
    assert!(matches!(
        &err,
        RefreshLeaseError::Io {
            kind: crate::RefreshLeaseIoKind::ResultTooLarge,
            ..
        }
    ));
    assert!(err.to_string().contains("exceeds safe size limit"));
    assert!(!paths.result_path().exists());

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn refresh_lease_ignores_symlinked_result() {
    let root = temp_dir("refresh-lease-symlink-result");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let paths = coordinator.paths_for_key("shared-refresh-token-secret");
    let outside = root.join("outside-result.json");
    fs::write(&outside, "{\"access_token\":\"attacker\"}").unwrap();
    std::os::unix::fs::symlink(&outside, paths.result_path()).unwrap();

    match coordinator.acquire("shared-refresh-token-secret").unwrap() {
        RefreshLeaseDecision::Owner(owner) => {
            assert_eq!(owner.lock_path(), paths.lock_path());
            assert!(!paths.result_path().is_symlink());
            assert_eq!(
                fs::read_to_string(&outside).unwrap(),
                "{\"access_token\":\"attacker\"}"
            );
        }
        other => panic!("expected owner after unsafe result cleanup, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn refresh_lease_removes_symlinked_lock_before_acquiring() {
    let root = temp_dir("refresh-lease-symlink-lock");
    let coordinator = RefreshLeaseCoordinator::new(&root);
    let paths = coordinator.paths_for_key("shared-refresh-token-secret");
    let outside = root.join("outside-lock");
    fs::write(&outside, "pid=attacker\n").unwrap();
    std::os::unix::fs::symlink(&outside, paths.lock_path()).unwrap();

    match coordinator.acquire("shared-refresh-token-secret").unwrap() {
        RefreshLeaseDecision::Owner(owner) => {
            assert_eq!(owner.lock_path(), paths.lock_path());
            assert!(!owner.lock_path().is_symlink());
            assert_eq!(fs::read_to_string(&outside).unwrap(), "pid=attacker\n");
        }
        other => panic!("expected owner after unsafe lock cleanup, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_waiting_follower_reads_committed_result() {
    let root = temp_dir("refresh-lease-waiting-follower");
    let owner_coordinator = RefreshLeaseCoordinator::new(&root);
    let follower_coordinator = RefreshLeaseCoordinator::new(&root)
        .with_wait_timeout(Duration::from_secs(2))
        .with_poll_interval(Duration::from_millis(1));
    let sensitive_key = "shared-refresh-token-secret";
    let owner = match owner_coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner,
        other => panic!("expected owner, got {other:?}"),
    };

    let follower = std::thread::spawn(move || follower_coordinator.acquire(sensitive_key).unwrap());
    std::thread::sleep(Duration::from_millis(20));
    owner
        .commit_result("{\"access_token\":\"shared-result\"}")
        .unwrap();

    match follower.join().unwrap() {
        RefreshLeaseDecision::Follower { result_json } => {
            assert_eq!(result_json.as_str(), "{\"access_token\":\"shared-result\"}");
        }
        other => panic!("expected follower, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_heartbeat_keeps_long_running_owner_live() {
    let root = temp_dir("refresh-lease-heartbeat");
    let coordinator = RefreshLeaseCoordinator::new(&root)
        .with_lease_ttl(Duration::from_millis(30))
        .with_wait_timeout(Duration::ZERO)
        .with_poll_interval(Duration::from_millis(1));
    let sensitive_key = "heartbeat-refresh-token";
    let owner = match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner,
        other => panic!("expected owner, got {other:?}"),
    };
    let initial_modified = fs::metadata(owner.lock_path()).unwrap().modified().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while fs::metadata(owner.lock_path()).unwrap().modified().unwrap() <= initial_modified {
        assert!(
            std::time::Instant::now() < deadline,
            "heartbeat should advance lock mtime"
        );
        std::thread::sleep(Duration::from_millis(2));
    }

    match coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Bypass { reason } => {
            assert_eq!(reason, RefreshLeaseBypassReason::WaitTimeout);
        }
        other => panic!("live owner must not be taken over, got {other:?}"),
    }

    drop(owner);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn concurrent_refresh_leases_keep_single_owner_past_ttl() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};

    const WORKERS: usize = 8;
    let root = temp_dir("refresh-lease-concurrency");
    let coordinator = Arc::new(
        RefreshLeaseCoordinator::new(&root)
            .with_lease_ttl(Duration::from_millis(15))
            .with_wait_timeout(Duration::from_secs(2))
            .with_poll_interval(Duration::from_millis(1)),
    );
    let barrier = Arc::new(Barrier::new(WORKERS));
    let owners = Arc::new(AtomicUsize::new(0));
    let sensitive_key = "concurrent-heartbeat-refresh-token";

    let roles = std::thread::scope(|scope| {
        let handles = (0..WORKERS)
            .map(|_| {
                let coordinator = Arc::clone(&coordinator);
                let barrier = Arc::clone(&barrier);
                let owners = Arc::clone(&owners);
                scope.spawn(move || {
                    barrier.wait();
                    match coordinator.acquire(sensitive_key).unwrap() {
                        RefreshLeaseDecision::Owner(owner) => {
                            owners.fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(Duration::from_millis(75));
                            owner
                                .commit_result("{\"access_token\":\"shared-result\"}")
                                .unwrap();
                            RefreshLeaseRole::Owner
                        }
                        RefreshLeaseDecision::Follower { result_json } => {
                            assert_eq!(
                                result_json.as_str(),
                                "{\"access_token\":\"shared-result\"}"
                            );
                            RefreshLeaseRole::Follower
                        }
                        other => panic!("concurrent lease should not bypass: {other:?}"),
                    }
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("lease worker should finish"))
            .collect::<Vec<_>>()
    });

    assert_eq!(owners.load(Ordering::SeqCst), 1);
    assert_eq!(
        roles
            .iter()
            .filter(|role| **role == RefreshLeaseRole::Follower)
            .count(),
        WORKERS - 1
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_recovers_stale_lock() {
    let root = temp_dir("refresh-lease-stale");
    let coordinator = RefreshLeaseCoordinator::new(&root)
        .with_lease_ttl(Duration::ZERO)
        .with_wait_timeout(Duration::from_millis(20))
        .with_poll_interval(Duration::from_millis(1));
    let paths = coordinator.paths_for_key("stale-token-secret");
    fs::write(paths.lock_path(), "pid=old\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(paths.lock_path(), fs::Permissions::from_mode(0o600)).unwrap();
    }
    std::thread::sleep(Duration::from_millis(2));

    match coordinator.acquire("stale-token-secret").unwrap() {
        RefreshLeaseDecision::Owner(owner) => {
            assert_eq!(owner.lock_path(), paths.lock_path());
            assert!(owner.lock_path().exists());
        }
        other => panic!("expected owner after stale cleanup, got {other:?}"),
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_times_out_to_bypass_when_lock_is_held() {
    let root = temp_dir("refresh-lease-bypass");
    let owner_coordinator = RefreshLeaseCoordinator::new(&root);
    let follower_coordinator = RefreshLeaseCoordinator::new(&root)
        .with_wait_timeout(Duration::ZERO)
        .with_poll_interval(Duration::from_millis(1));
    let sensitive_key = "held-token-secret";
    let owner = match owner_coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Owner(owner) => owner,
        other => panic!("expected owner, got {other:?}"),
    };

    match follower_coordinator.acquire(sensitive_key).unwrap() {
        RefreshLeaseDecision::Bypass { reason } => {
            assert_eq!(reason, RefreshLeaseBypassReason::WaitTimeout);
        }
        other => panic!("expected bypass, got {other:?}"),
    }

    drop(owner);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn refresh_lease_file_names_use_hash_not_sensitive_material() {
    let root = temp_dir("refresh-lease-hash");
    let coordinator = RefreshLeaseCoordinator::new(&root).with_namespace("quota-refresh");
    let sensitive_key = "sk-prodex-super-secret-token";
    let paths = coordinator.paths_for_key(sensitive_key);
    let lock_name = paths.lock_path().file_name().unwrap().to_string_lossy();
    let result_name = paths.result_path().file_name().unwrap().to_string_lossy();

    assert_eq!(paths.digest().len(), 64);
    assert!(paths.digest().chars().all(|ch| ch.is_ascii_hexdigit()));
    assert_eq!(lock_name, format!("{}.lock", paths.digest()));
    assert_eq!(result_name, format!("{}.result.json", paths.digest()));
    assert!(!lock_name.contains(sensitive_key));
    assert!(!result_name.contains(sensitive_key));

    let _ = fs::remove_dir_all(root);
}
