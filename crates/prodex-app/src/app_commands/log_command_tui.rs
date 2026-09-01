pub(super) use self::render::log_snapshot_items;
#[cfg(test)]
pub(super) use self::render::log_stream_tui_text;
use super::{
    FollowedLog, FollowedLogPaths, LOG_SNAPSHOT_TAIL_BYTES, LiveRuntimeLogSource, LogLoadAggregate,
    LogStreamItem, TranscriptEvent, collect_live_log_items, collect_new_runtime_log_stream_items,
    collect_new_runtime_log_stream_items_for_tui_with_throughput, collect_new_transcript_events,
    is_routine_load_event, latest_transcript_event, local_token_usage_event, print_log_stream_item,
    print_token_usage_event, print_transcript_event, print_upstream_payload_event,
    recent_session_log_paths, retain_followed_logs,
};
use crate::app_commands::collect_recent_runtime_log_paths;
use crate::app_commands::log_tui::{
    LogTuiHeaderDetail, LogTuiInput, LogTuiState, LogTuiTerminal, OutputThroughput,
    log_tui_header_detail, log_tui_header_next_refresh_at,
};
use crate::app_commands::log_upstream::{
    latest_upstream_payload_event, stream_upstream_payload_events,
};
use crate::app_commands::log_upstream_payload::UpstreamPayloadEvent;
use crate::app_commands::log_upstream_payload::upstream_payload_event_from_runtime_line;
use crate::reports::{InfoTokenUsageEvent, info_token_usage_event_from_line};
use crate::{LogArgs, LogMode, prodex_runtime_log_paths_in_dir, runtime_proxy_log_dir};
use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyEventKind};
use prodex_runtime_doctor::read_runtime_log_tail;
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

#[path = "log_tui_render.rs"]
mod render;

const LOG_STREAM_POLL_INTERVAL: Duration = Duration::from_millis(250);
const SESSION_PATH_RECONCILE_INTERVAL: Duration = Duration::from_secs(10);
const LOG_TUI_EVENT_LIMIT: usize = 200;
const LOG_LOAD_COALESCE_WINDOW: Duration = Duration::from_secs(5);

pub(crate) fn handle_log(args: LogArgs) -> Result<()> {
    match args.mode {
        LogMode::Last => {
            let transcript = latest_transcript_event()?;
            let upstream_payload = latest_upstream_payload_event();
            let token_usage = latest_token_usage_event();
            if args.json {
                for item in log_snapshot_items(
                    transcript.as_ref(),
                    upstream_payload.as_ref(),
                    token_usage.as_ref(),
                ) {
                    print_log_stream_item(&item, true)?;
                }
                return Ok(());
            }
            print_log_snapshot(
                transcript.as_ref(),
                upstream_payload.as_ref(),
                token_usage.as_ref(),
            )
        }
        LogMode::Stream => stream_token_usage_events(args.json),
        LogMode::Upstream => stream_upstream_payload_events(args.json),
    }
}

fn latest_token_usage_event() -> Option<InfoTokenUsageEvent> {
    let mut latest = None;
    for path in collect_recent_runtime_log_paths(32) {
        let tail = match read_runtime_log_tail(&path, LOG_SNAPSHOT_TAIL_BYTES) {
            Ok(tail) => tail,
            Err(_) => continue,
        };
        for line in String::from_utf8_lossy(&tail).lines() {
            let Some(event) = info_token_usage_event_from_line(line).map(local_token_usage_event)
            else {
                continue;
            };
            if latest
                .as_ref()
                .is_none_or(|current: &InfoTokenUsageEvent| event.timestamp >= current.timestamp)
            {
                latest = Some(event);
            }
        }
    }
    let mut live_source = LiveRuntimeLogSource::new();
    for (_, line) in live_source
        .as_mut()
        .map(LiveRuntimeLogSource::poll)
        .into_iter()
        .flatten()
    {
        let Some(event) = info_token_usage_event_from_line(&line).map(local_token_usage_event)
        else {
            continue;
        };
        if latest
            .as_ref()
            .is_none_or(|current: &InfoTokenUsageEvent| event.timestamp >= current.timestamp)
        {
            latest = Some(event);
        }
    }
    latest
}

fn stream_token_usage_events(json: bool) -> Result<()> {
    if !json
        && !super::no_color_requested()
        && io::stdout().is_terminal()
        && io::stdin().is_terminal()
    {
        return stream_token_usage_events_tui();
    }

    let mut live_source = LiveRuntimeLogSource::new();
    print_initial_token_usage_events(json, &mut live_source)?;
    let mut runtime_paths =
        FollowedLogPaths::new(prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()));
    let mut session_paths = FollowedLogPaths::with_refresh_interval(
        stream_session_log_paths(),
        SESSION_PATH_RECONCILE_INTERVAL,
    );
    let mut followed_runtime_logs = followed_logs(
        runtime_paths.refresh(|| prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir())),
    );
    let mut followed_session_logs = followed_logs(session_paths.refresh(stream_session_log_paths));
    follow_token_usage_events(
        json,
        &mut followed_runtime_logs,
        &mut followed_session_logs,
        &mut runtime_paths,
        &mut session_paths,
        &mut live_source,
    )
}

fn stream_token_usage_events_tui() -> Result<()> {
    let mut tui = LogTuiTerminal::stdout("log stream TUI")?;
    let mut view = LogTuiState::default();
    let mut live_source = LiveRuntimeLogSource::new();
    let mut throughput = OutputThroughput::default();
    crate::app_commands::log_tui::seed_output_throughput_from_history(&mut throughput);
    let mut items = initial_log_stream_items_with_live(&mut live_source, Some(&mut throughput))?;
    let mut header_profile = latest_log_stream_profile(&items).map(str::to_string);
    let mut header_detail = log_tui_header_detail(header_profile.as_deref());
    let mut header_refresh_at =
        log_tui_header_next_refresh_at(header_detail.as_ref(), Instant::now());

    let mut runtime_paths =
        FollowedLogPaths::new(prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()));
    let mut session_paths = FollowedLogPaths::with_refresh_interval(
        stream_session_log_paths(),
        SESSION_PATH_RECONCILE_INTERVAL,
    );
    let mut followed_runtime_logs = followed_logs(
        runtime_paths.refresh(|| prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir())),
    );
    let mut followed_session_logs = followed_logs(session_paths.refresh(stream_session_log_paths));

    loop {
        collect_log_stream_items_with_live(
            &mut items,
            &mut followed_runtime_logs,
            &mut followed_session_logs,
            &mut runtime_paths,
            &mut session_paths,
            &mut throughput,
            &mut live_source,
        )?;
        update_log_stream_header(
            &items,
            &mut header_profile,
            &mut header_detail,
            &mut header_refresh_at,
        );

        tui.terminal
            .draw(|frame| {
                render_log_stream_tui(
                    frame,
                    &items,
                    &view,
                    header_detail.as_ref(),
                    throughput.display_rate_for_profile(Instant::now(), header_profile.as_deref()),
                )
            })
            .context("failed to draw log stream TUI")?;

        if log_stream_tui_should_quit(&mut view)? {
            return Ok(());
        }
    }
}

fn print_initial_token_usage_events(
    json: bool,
    live_source: &mut Option<LiveRuntimeLogSource>,
) -> Result<()> {
    let items = initial_log_stream_items_with_live(live_source, None)?;
    if items.is_empty() {
        eprintln!("Waiting for transcript, upstream payload, or token usage events...");
        return Ok(());
    }
    for item in items {
        print_log_stream_item(&item, json)?;
    }
    Ok(())
}

fn followed_logs(paths: &[PathBuf]) -> BTreeMap<PathBuf, FollowedLog> {
    paths
        .iter()
        .map(|path| (path.clone(), FollowedLog::at_end(path)))
        .collect()
}

fn stream_session_log_paths() -> Vec<PathBuf> {
    recent_session_log_paths().unwrap_or_default()
}

fn follow_token_usage_events(
    json: bool,
    followed_runtime_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    followed_session_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    runtime_paths: &mut FollowedLogPaths,
    session_paths: &mut FollowedLogPaths,
    live_source: &mut Option<LiveRuntimeLogSource>,
) -> Result<()> {
    loop {
        read_token_usage_events_tick_with_live(
            json,
            followed_runtime_logs,
            followed_session_logs,
            runtime_paths,
            session_paths,
            live_source,
        )?;
        thread::sleep(LOG_STREAM_POLL_INTERVAL);
    }
}

#[cfg(test)]
fn read_token_usage_events_tick(
    json: bool,
    followed_runtime_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    followed_session_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    runtime_paths: &mut FollowedLogPaths,
    session_paths: &mut FollowedLogPaths,
) -> Result<()> {
    read_token_usage_events_tick_with_live(
        json,
        followed_runtime_logs,
        followed_session_logs,
        runtime_paths,
        session_paths,
        &mut None,
    )
}

fn read_token_usage_events_tick_with_live(
    json: bool,
    followed_runtime_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    followed_session_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    runtime_paths: &mut FollowedLogPaths,
    session_paths: &mut FollowedLogPaths,
    live_source: &mut Option<LiveRuntimeLogSource>,
) -> Result<()> {
    for event in collect_live_log_items(live_source, true, None)? {
        print_log_stream_item(&event, json)?;
    }
    let current_runtime_paths =
        runtime_paths.refresh(|| prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()));
    retain_followed_logs(followed_runtime_logs, current_runtime_paths);
    for path in current_runtime_paths {
        let state = followed_runtime_logs
            .entry(path.clone())
            .or_insert_with(|| FollowedLog::at_end(path));
        for event in collect_new_runtime_log_stream_items(path, state, true)? {
            print_log_stream_item(&event, json)?;
        }
    }
    let current_session_paths = session_paths.refresh(stream_session_log_paths);
    retain_followed_logs(followed_session_logs, current_session_paths);
    for path in current_session_paths {
        let state = followed_session_logs
            .entry(path.clone())
            .or_insert_with(|| FollowedLog::at_end(path));
        for event in collect_new_transcript_events(path, state)? {
            if json {
                print_log_stream_item(&LogStreamItem::Transcript(event), true)?;
            } else {
                print_transcript_event(&event)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
fn initial_log_stream_items() -> Result<VecDeque<LogStreamItem>> {
    initial_log_stream_items_with_live(&mut None, None)
}

fn initial_log_stream_items_with_live(
    live_source: &mut Option<LiveRuntimeLogSource>,
    throughput: Option<&mut OutputThroughput>,
) -> Result<VecDeque<LogStreamItem>> {
    let mut items = VecDeque::new();
    if let Ok(Some(event)) = latest_transcript_event() {
        push_log_stream_item(&mut items, LogStreamItem::Transcript(event));
    }
    let (stream, upstream, token_usage) = latest_runtime_snapshot_events();
    if let Some(event) = stream {
        push_log_stream_item(&mut items, LogStreamItem::Transcript(event));
    }
    if let Some(event) = upstream {
        push_log_stream_item(&mut items, LogStreamItem::UpstreamPayload(event));
    }
    if let Some(event) = token_usage {
        push_log_stream_item(&mut items, LogStreamItem::TokenUsage(event));
    }
    for event in collect_live_log_items(live_source, true, throughput)? {
        push_log_stream_item(&mut items, event);
    }
    Ok(items)
}

fn latest_runtime_snapshot_events() -> (
    Option<TranscriptEvent>,
    Option<UpstreamPayloadEvent>,
    Option<InfoTokenUsageEvent>,
) {
    let mut latest_stream = None;
    let mut latest_upstream = None;
    let mut latest_token_usage = None;
    for path in collect_recent_runtime_log_paths(32) {
        let Ok(tail) = read_runtime_log_tail(&path, LOG_SNAPSHOT_TAIL_BYTES) else {
            continue;
        };
        for line in String::from_utf8_lossy(&tail).lines() {
            if let Some(event) = super::log_stream::stream_payload_event_from_runtime_line(line)
                && latest_stream
                    .as_ref()
                    .is_none_or(|current: &TranscriptEvent| event.timestamp >= current.timestamp)
            {
                latest_stream = Some(event);
            }
            if let Some(event) = upstream_payload_event_from_runtime_line(line)
                && latest_upstream
                    .as_ref()
                    .is_none_or(|current: &UpstreamPayloadEvent| {
                        event.timestamp >= current.timestamp
                    })
            {
                latest_upstream = Some(event);
            }
            if let Some(event) = info_token_usage_event_from_line(line).map(local_token_usage_event)
                && latest_token_usage
                    .as_ref()
                    .is_none_or(|current: &InfoTokenUsageEvent| {
                        event.timestamp >= current.timestamp
                    })
            {
                latest_token_usage = Some(event);
            }
        }
    }
    (latest_stream, latest_upstream, latest_token_usage)
}

fn collect_log_stream_items_with_live(
    items: &mut VecDeque<LogStreamItem>,
    followed_runtime_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    followed_session_logs: &mut BTreeMap<PathBuf, FollowedLog>,
    runtime_paths: &mut FollowedLogPaths,
    session_paths: &mut FollowedLogPaths,
    throughput: &mut OutputThroughput,
    live_source: &mut Option<LiveRuntimeLogSource>,
) -> Result<()> {
    for event in collect_live_log_items(live_source, true, Some(throughput))? {
        push_log_stream_item(items, event);
    }
    let current_runtime_paths =
        runtime_paths.refresh(|| prodex_runtime_log_paths_in_dir(&runtime_proxy_log_dir()));
    retain_followed_logs(followed_runtime_logs, current_runtime_paths);
    for path in current_runtime_paths {
        let state = followed_runtime_logs
            .entry(path.clone())
            .or_insert_with(|| FollowedLog::at_end(path));
        for event in collect_new_runtime_log_stream_items_for_tui_with_throughput(
            path,
            state,
            true,
            Some(throughput),
        )? {
            push_log_stream_item(items, event);
        }
    }
    let current_session_paths = session_paths.refresh(stream_session_log_paths);
    retain_followed_logs(followed_session_logs, current_session_paths);
    for path in current_session_paths {
        let state = followed_session_logs
            .entry(path.clone())
            .or_insert_with(|| FollowedLog::at_end(path));
        for event in collect_new_transcript_events(path, state)? {
            push_log_stream_item(items, LogStreamItem::Transcript(event));
        }
    }
    Ok(())
}

fn update_log_stream_header(
    items: &VecDeque<LogStreamItem>,
    header_profile: &mut Option<String>,
    header_detail: &mut Option<LogTuiHeaderDetail>,
    header_refresh_at: &mut Instant,
) {
    let latest_profile = latest_log_stream_profile(items).map(str::to_string);
    let now = Instant::now();
    if latest_profile != *header_profile || now >= *header_refresh_at {
        *header_profile = latest_profile;
        *header_detail = log_tui_header_detail(header_profile.as_deref());
        *header_refresh_at = log_tui_header_next_refresh_at(header_detail.as_ref(), now);
    }
}

fn log_stream_tui_should_quit(view: &mut LogTuiState) -> Result<bool> {
    if !event::poll(LOG_STREAM_POLL_INTERVAL).context("failed to poll log stream TUI input")? {
        return Ok(false);
    }
    let Event::Key(key) = event::read().context("failed to read log stream TUI input")? else {
        return Ok(false);
    };
    Ok(key.kind == KeyEventKind::Press && view.apply_key(key) == LogTuiInput::Quit)
}

fn print_log_snapshot(
    transcript: Option<&TranscriptEvent>,
    upstream_payload: Option<&UpstreamPayloadEvent>,
    token_usage: Option<&InfoTokenUsageEvent>,
) -> Result<()> {
    if !super::no_color_requested()
        && io::stdout().is_terminal()
        && let Some(mut terminal) = crate::try_inline_stdout_terminal(
            render::log_snapshot_tui_height(transcript, upstream_payload, token_usage),
        )
    {
        let items = log_snapshot_items(transcript, upstream_payload, token_usage);
        terminal
            .draw(|frame| render::render_log_snapshot_tui(frame, &items))
            .context("failed to draw log snapshot TUI")?;
        let _ = terminal.show_cursor();
        return Ok(());
    }

    if transcript.is_none() && upstream_payload.is_none() && token_usage.is_none() {
        println!("No transcript, upstream payload, or token usage events found.");
        return Ok(());
    }
    if let Some(event) = transcript {
        print_transcript_event(event)?;
    }
    if let Some(event) = upstream_payload {
        print_upstream_payload_event(event)?;
    }
    if let Some(event) = token_usage {
        print_token_usage_event(event, false)?;
    }
    Ok(())
}

fn push_log_stream_item(items: &mut VecDeque<LogStreamItem>, item: LogStreamItem) {
    push_log_stream_item_at(items, item, Instant::now());
}

fn push_log_stream_item_at(items: &mut VecDeque<LogStreamItem>, item: LogStreamItem, now: Instant) {
    if matches!(&item, LogStreamItem::LoadObservation(observation) if is_routine_load_event(&observation.event_name))
    {
        return;
    }
    if let LogStreamItem::LoadObservation(observation) = item {
        let key = load_observation_key(&observation);
        let event = observation.event;
        let run_id = observation.run_id;
        // ponytail: a bounded 5s projection episode with at most 256 run ids; replace the
        // vector with a compact counter if higher-cardinality diagnostics become necessary.
        if let Some(LogStreamItem::LoadAggregate(aggregate)) = items.back_mut()
            && aggregate.key == key
            && now.saturating_duration_since(aggregate.last_seen) <= LOG_LOAD_COALESCE_WINDOW
        {
            aggregate.observe(event, run_id, now);
            return;
        }
        items.push_back(LogStreamItem::LoadAggregate(LogLoadAggregate::new(
            event, key, run_id, now,
        )));
    } else {
        items.push_back(item);
    }
    while items.len() > LOG_TUI_EVENT_LIMIT {
        items.pop_front();
    }
}

fn load_observation_key(observation: &super::LogLoadObservation) -> String {
    let field = |name: &str| {
        observation
            .fields
            .get(name)
            .map(String::as_str)
            .unwrap_or("-")
    };
    [
        observation.event_name.as_str(),
        field("profile"),
        field("route"),
        field("lane"),
        field("transport"),
        field("context"),
        field("path"),
        field("provider"),
        field("model"),
        field("active"),
        observation
            .fields
            .get("limit")
            .or_else(|| observation.fields.get("hard_limit"))
            .map(String::as_str)
            .unwrap_or("-"),
        field("reason"),
    ]
    .join("\u{1f}")
}

fn latest_log_stream_profile(items: &VecDeque<LogStreamItem>) -> Option<&str> {
    items.iter().rev().find_map(|item| match item {
        LogStreamItem::TokenUsage(event) => Some(event.profile.as_str()),
        // Upstream payload metadata is a route observation, not the header's canonical
        // profile identity.  The shared header falls back to AppState when no token event
        // supplies a profile, so a payload field cannot replace quota/profile state.
        LogStreamItem::UpstreamPayload(_) => None,
        LogStreamItem::LoadObservation(_) => None,
        LogStreamItem::LoadAggregate(_) => None,
        LogStreamItem::Transcript(_) => None,
    })
}

fn render_log_stream_tui(
    frame: &mut ratatui::Frame<'_>,
    items: &VecDeque<LogStreamItem>,
    state: &LogTuiState,
    header_detail: Option<&LogTuiHeaderDetail>,
    throughput_rate: Option<f64>,
) {
    render::render_log_stream_tui(frame, items, state, header_detail, throughput_rate);
}

#[cfg(test)]
mod tests {
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
}
