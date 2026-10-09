use super::*;
use crate::app_commands::LogLoadObservation;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn upstream_payload_metadata_cannot_replace_stream_header_profile() {
    let items = VecDeque::from([LogStreamItem::UpstreamPayload(UpstreamPayloadEvent {
        timestamp: "2026-08-28 10:00:00".to_string(),
        request: Some(1),
        transport: "http".to_string(),
        route: "responses".to_string(),
        profile: "second".to_string(),
        bytes: 1,
        logged_bytes: 1,
        truncated: false,
        payload: "{}".to_string(),
    })]);

    assert_eq!(latest_log_stream_profile(&items), None);
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn maps_scroll_and_search_keys() {
    let mut state = LogTuiState::default();

    assert_eq!(state.apply_key(key(KeyCode::Up)), LogTuiInput::Continue);
    assert_eq!(state.scroll_from_bottom(), 1);
    assert_eq!(state.apply_key(key(KeyCode::Down)), LogTuiInput::Continue);
    assert_eq!(state.scroll_from_bottom(), 0);

    state.apply_key(key(KeyCode::Char('/')));
    state.apply_key(key(KeyCode::Char('h')));
    state.apply_key(key(KeyCode::Char('i')));
    state.apply_key(key(KeyCode::Enter));

    assert_eq!(state.query(), Some("hi"));
    assert!(state.footer_text("q quit").contains("search: /hi"));
}

fn load_event(run_id: usize, profile: &str, limit: usize) -> LogStreamItem {
    let event = TranscriptEvent {
        timestamp: "2026-08-31 10:00:00.000 +07:00".to_string(),
        source: "load".to_string(),
        text: format!("r{run_id:04x}  profile busy  profile={profile} · route=responses"),
    };
    LogStreamItem::LoadObservation(LogLoadObservation {
        event,
        event_name: "profile_inflight_saturated".to_string(),
        fields: BTreeMap::from([
            ("profile".to_string(), profile.to_string()),
            ("route".to_string(), "responses".to_string()),
            ("transport".to_string(), "http".to_string()),
            ("active".to_string(), limit.to_string()),
            ("hard_limit".to_string(), limit.to_string()),
        ]),
        run_id: Some(format!("r{run_id:04x}")),
    })
}

#[test]
fn hides_repeated_profile_busy_observations_from_default_timeline() {
    let mut items = VecDeque::new();
    let now = Instant::now();
    for run_id in 0..100 {
        push_log_stream_item_at(&mut items, load_event(run_id, "main", 8), now);
    }

    assert!(items.is_empty(), "routine profile-busy telemetry is hidden");
}

#[test]
fn routine_load_telemetry_cannot_evict_a_real_error() {
    let mut items = VecDeque::new();
    let now = Instant::now();
    for run_id in 0..100_000 {
        push_log_stream_item_at(&mut items, load_event(run_id, "main", 8), now);
    }
    items.push_back(LogStreamItem::Transcript(TranscriptEvent {
        timestamp: "2026-08-31 10:00:01.000 +07:00".to_string(),
        source: "error".to_string(),
        text: "provider auth failed".to_string(),
    }));

    assert_eq!(items.len(), 1);
    assert!(matches!(
        &items[0],
        LogStreamItem::Transcript(event) if event.source == "error"
    ));
}

#[test]
fn load_aggregation_keeps_profiles_limits_and_separate_episodes_distinct() {
    let mut items = VecDeque::new();
    let now = Instant::now();
    push_log_stream_item_at(&mut items, load_event(1, "main", 8), now);
    push_log_stream_item_at(&mut items, load_event(2, "backup", 8), now);
    push_log_stream_item_at(&mut items, load_event(3, "main", 16), now);
    push_log_stream_item_at(
        &mut items,
        load_event(4, "main", 8),
        now + Duration::from_secs(6),
    );

    assert!(items.is_empty(), "routine load telemetry is hidden");
}

#[test]
fn load_aggregation_does_not_evict_a_meaningful_event() {
    let mut items = VecDeque::new();
    let now = Instant::now();
    for run_id in 0..1000 {
        push_log_stream_item_at(&mut items, load_event(run_id, "main", 8), now);
    }
    items.push_back(LogStreamItem::Transcript(TranscriptEvent {
        timestamp: "2026-08-31 10:00:01.000 +07:00".to_string(),
        source: "error".to_string(),
        text: "provider auth failed".to_string(),
    }));
    for run_id in 1000..2000 {
        push_log_stream_item_at(&mut items, load_event(run_id, "main", 8), now);
    }

    assert_eq!(items.len(), 1);
    assert!(items.iter().any(|item| {
        matches!(item, LogStreamItem::Transcript(event) if event.source == "error")
    }));
    assert!(items.iter().all(|item| !matches!(
        item,
        LogStreamItem::LoadObservation(_) | LogStreamItem::LoadAggregate(_)
    )));
}

#[test]
fn load_recovery_starts_a_new_busy_episode() {
    let mut items = VecDeque::new();
    let now = Instant::now();
    push_log_stream_item_at(&mut items, load_event(1, "main", 8), now);
    let mut recovery = load_event(2, "main", 8);
    if let LogStreamItem::LoadObservation(observation) = &mut recovery {
        observation.event_name = "profile_inflight".to_string();
        observation.event.text = "r0002 profile available profile=main active=7".to_string();
        observation
            .fields
            .insert("active".to_string(), "7".to_string());
    }
    push_log_stream_item_at(&mut items, recovery, now + Duration::from_millis(10));
    push_log_stream_item_at(
        &mut items,
        load_event(3, "main", 8),
        now + Duration::from_millis(20),
    );

    assert_eq!(items.len(), 1);
    assert!(matches!(
        &items[0],
        LogStreamItem::LoadAggregate(aggregate)
            if aggregate.key.starts_with("profile_inflight\u{1f}")
    ));
}

#[test]
fn mojo_load_plan_coalesces_at_boundary_and_restarts_after_freshness_window() {
    let mut items = VecDeque::new();
    let now = Instant::now();
    let visible_event = |run_id| {
        let mut item = load_event(run_id, "main", 8);
        if let LogStreamItem::LoadObservation(observation) = &mut item {
            observation.event_name = "profile_inflight".to_string();
        }
        item
    };

    push_log_stream_item_at(&mut items, visible_event(1), now);
    push_log_stream_item_at(&mut items, visible_event(2), now + Duration::from_secs(5));
    push_log_stream_item_at(
        &mut items,
        visible_event(3),
        now + Duration::from_secs(10) + Duration::from_nanos(1),
    );

    assert_eq!(items.len(), 2);
    let LogStreamItem::LoadAggregate(first) = &items[0] else {
        panic!("expected first load aggregate episode");
    };
    let LogStreamItem::LoadAggregate(second) = &items[1] else {
        panic!("expected second load aggregate episode");
    };
    assert_eq!(first.occurrences, 2);
    assert_eq!(
        first.unique_runs,
        vec!["r0001".to_string(), "r0002".to_string()]
    );
    assert_eq!(
        first.as_transcript().text,
        "r0002  profile busy  profile=main · route=responses · ×2 · 2 runs"
    );
    assert_eq!(second.occurrences, 1);
}

#[test]
fn stream_tick_survives_temporarily_unreadable_session_root() {
    let _runtime_lock = crate::acquire_test_runtime_lock();
    let root = std::env::temp_dir().join(format!(
        "prodex-log-stream-transient-root-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let shared_file = root.join("shared-file");
    std::fs::write(&shared_file, "temporarily unavailable").unwrap();
    let _home = crate::TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
    let _shared =
        crate::TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", shared_file.to_str().unwrap());
    let _logs = crate::TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        root.join("logs").to_str().unwrap(),
    );

    assert!(initial_log_stream_items().is_ok());
    let mut runtime_paths = FollowedLogPaths::default();
    let mut session_paths = FollowedLogPaths::default();
    assert!(
        read_token_usage_events_tick(
            true,
            &mut BTreeMap::new(),
            &mut BTreeMap::new(),
            &mut runtime_paths,
            &mut session_paths,
        )
        .is_ok()
    );
    let _ = std::fs::remove_dir_all(root);
}
