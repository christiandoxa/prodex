#[test]
fn all_quota_watch_refresh_keeps_single_background_refresh_in_flight() {
    let mut refresh = AllQuotaWatchRefresh::new();
    let (release_sender, release_receiver) = mpsc::channel();

    assert!(refresh.try_start(move || {
        release_receiver
            .recv()
            .expect("test refresh should be released");
        AllQuotaWatchSnapshot::Empty {
            updated: "done".to_string(),
        }
    }));
    assert!(!refresh.try_start(|| AllQuotaWatchSnapshot::Empty {
        updated: "second".to_string(),
    }));
    assert!(refresh.take_latest().is_none());

    release_sender
        .send(())
        .expect("test refresh release should send");
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut completed = None;
    while Instant::now() < deadline {
        if let Some(snapshot) = refresh.take_latest() {
            completed = Some(snapshot);
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(matches!(
        completed,
        Some(AllQuotaWatchSnapshot::Empty { .. })
    ));
    assert!(refresh.try_start(|| AllQuotaWatchSnapshot::Empty {
        updated: "third".to_string(),
    }));
}

#[test]
fn quota_watch_refresh_recovers_from_loader_panic() {
    let mut refresh = AllQuotaWatchRefresh::new();
    assert!(refresh.try_start_catching_panic(
        || -> AllQuotaWatchSnapshot { panic!("test loader panic") },
        AllQuotaWatchSnapshot::Error {
            updated: "fallback".to_string(),
            message: "quota refresh failed unexpectedly".to_string(),
        },
    ));

    let deadline = Instant::now() + Duration::from_secs(2);
    let mut completed = None;
    while Instant::now() < deadline {
        if let Some(snapshot) = refresh.take_latest() {
            completed = Some(snapshot);
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(matches!(
        completed,
        Some(AllQuotaWatchSnapshot::Error { message, .. })
            if message == "quota refresh failed unexpectedly"
    ));
    assert!(refresh.try_start(|| AllQuotaWatchSnapshot::Empty {
        updated: "after-panic".to_string(),
    }));
}

#[test]
fn profile_quota_watch_refresh_keeps_single_background_refresh_in_flight() {
    let mut refresh = ProfileQuotaWatchRefresh::new();
    let (release_sender, release_receiver) = mpsc::channel();

    assert!(refresh.try_start(move || {
        release_receiver
            .recv()
            .expect("test refresh should be released");
        ProfileQuotaWatchSnapshot {
            updated: "done".to_string(),
            quota: Err("refresh failed".to_string()),
        }
    }));
    assert!(!refresh.try_start(|| ProfileQuotaWatchSnapshot {
        updated: "second".to_string(),
        quota: Err("second refresh".to_string()),
    }));
    assert!(refresh.take_latest().is_none());

    release_sender
        .send(())
        .expect("test refresh release should send");
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut completed = None;
    while Instant::now() < deadline {
        if let Some(snapshot) = refresh.take_latest() {
            completed = Some(snapshot);
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }

    assert!(matches!(
        completed,
        Some(ProfileQuotaWatchSnapshot { updated, .. })
            if updated == "done"
    ));
    assert!(refresh.try_start(|| ProfileQuotaWatchSnapshot {
        updated: "third".to_string(),
        quota: Err("third refresh".to_string()),
    }));
}
