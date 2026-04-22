fn wait_for_runtime_continuations<F>(paths: &AppPaths, predicate: F) -> RuntimeContinuationStore
where
    F: Fn(&RuntimeContinuationStore) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_continuations = None;
    loop {
        if let Ok(continuations) = load_runtime_continuations_with_recovery(
            paths,
            &AppState::load(paths).unwrap_or_default().profiles,
        )
        .map(|loaded| loaded.value)
        {
            if predicate(&continuations) {
                return continuations;
            }
            last_continuations = Some(continuations);
        }
        if Instant::now() >= deadline {
            let continuations = load_runtime_continuations_with_recovery(
                paths,
                &AppState::load(paths).unwrap_or_default().profiles,
            )
            .map(|loaded| loaded.value)
            .expect("runtime continuations should reload");
            panic!(
                "timed out waiting for runtime continuations predicate; last_continuations={:?} final_continuations={:?}",
                last_continuations, continuations
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn dead_continuation_status(now: i64) -> RuntimeContinuationBindingStatus {
    RuntimeContinuationBindingStatus {
        state: RuntimeContinuationBindingLifecycle::Dead,
        confidence: 0,
        last_touched_at: Some(now),
        last_verified_at: Some(now.saturating_sub(5)),
        last_verified_route: Some("responses".to_string()),
        last_not_found_at: Some(now),
        not_found_streak: RUNTIME_CONTINUATION_SUSPECT_NOT_FOUND_STREAK_LIMIT,
        success_count: 1,
        failure_count: 1,
    }
}
