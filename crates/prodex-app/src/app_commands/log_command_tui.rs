pub(super) fn log_snapshot_items(
    transcript: Option<&TranscriptEvent>,
    upstream_payload: Option<&UpstreamPayloadEvent>,
    token_usage: Option<&InfoTokenUsageEvent>,
) -> VecDeque<LogStreamItem> {
    let order = prodex_mojo_core::log::snapshot_item_order(
        transcript.is_some(),
        upstream_payload.is_some(),
        token_usage.is_some(),
    )
    .expect("Mojo log snapshot order returned invalid output");
    order
        .into_iter()
        .map(|kind| match kind {
            prodex_mojo_core::log::LogSnapshotItemKind::Transcript => LogStreamItem::Transcript(
                transcript
                    .expect("Mojo snapshot order selected absent transcript")
                    .clone(),
            ),
            prodex_mojo_core::log::LogSnapshotItemKind::UpstreamPayload => {
                LogStreamItem::UpstreamPayload(
                    upstream_payload
                        .expect("Mojo snapshot order selected absent upstream payload")
                        .clone(),
                )
            }
            prodex_mojo_core::log::LogSnapshotItemKind::TokenUsage => LogStreamItem::TokenUsage(
                token_usage
                    .expect("Mojo snapshot order selected absent token usage")
                    .clone(),
            ),
        })
        .collect()
}

#[cfg(test)]
pub(super) use self::render::log_stream_tui_text;
use super::{
    FollowedLog, FollowedLogPaths, LOG_SNAPSHOT_TAIL_BYTES, LiveRuntimeLogSource, LogLoadAggregate,
    LogStreamItem, TranscriptEvent, bounded_followed_log_paths, collect_live_log_items,
    collect_new_runtime_log_stream_items,
    collect_new_runtime_log_stream_items_for_tui_with_throughput, collect_new_transcript_events,
    followed_log_map, latest_transcript_event, local_token_usage_event, print_log_stream_item,
    print_token_usage_event, print_transcript_event, print_upstream_payload_event,
    recent_session_log_paths, retain_followed_logs, runtime_log_paths_for_follow,
};
use crate::app_commands::collect_recent_runtime_log_paths;
use crate::app_commands::log_tui::{
    LogTuiHeaderDetail, LogTuiInput, LogTuiState, LogTuiTerminal, OutputThroughput,
    OutputThroughputDisplay, log_tui_header_detail, log_tui_header_next_refresh_at,
};
use crate::app_commands::log_upstream::{
    latest_upstream_payload_event, stream_upstream_payload_events,
};
use crate::app_commands::log_upstream_payload::UpstreamPayloadEvent;
use crate::app_commands::log_upstream_payload::upstream_payload_event_from_runtime_line;
use crate::reports::{InfoTokenUsageEvent, info_token_usage_event_from_line};
use crate::{LogArgs, LogMode};
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

const LOG_STREAM_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SESSION_PATH_RECONCILE_INTERVAL: Duration = Duration::from_secs(10);
const LOG_TUI_EVENT_LIMIT: usize = 200;

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
    let mut runtime_paths = FollowedLogPaths::new(runtime_log_paths_for_follow());
    let mut session_paths = FollowedLogPaths::with_refresh_interval(
        stream_session_log_paths(),
        SESSION_PATH_RECONCILE_INTERVAL,
    );
    let (initial_runtime_paths, initial_session_paths) = bounded_followed_log_paths(
        runtime_paths.refresh(runtime_log_paths_for_follow),
        session_paths.refresh(stream_session_log_paths),
    );
    let mut followed_runtime_logs = followed_log_map(&initial_runtime_paths);
    let mut followed_session_logs = followed_log_map(&initial_session_paths);
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

    let mut runtime_paths = FollowedLogPaths::new(runtime_log_paths_for_follow());
    let mut session_paths = FollowedLogPaths::with_refresh_interval(
        stream_session_log_paths(),
        SESSION_PATH_RECONCILE_INTERVAL,
    );
    let (initial_runtime_paths, initial_session_paths) = bounded_followed_log_paths(
        runtime_paths.refresh(runtime_log_paths_for_follow),
        session_paths.refresh(stream_session_log_paths),
    );
    let mut followed_runtime_logs = followed_log_map(&initial_runtime_paths);
    let mut followed_session_logs = followed_log_map(&initial_session_paths);

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
            &throughput,
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
                    throughput.display_for_profile(Instant::now(), header_profile.as_deref()),
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
        let message = "Waiting for transcript, upstream payload, or token events.".to_string();
        crate::app_commands::print_user_stderr_panel(
            "Prodex Log",
            std::slice::from_ref(&message),
            std::slice::from_ref(&message),
        )?;
        return Ok(());
    }
    for item in items {
        print_log_stream_item(&item, json)?;
    }
    Ok(())
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
    let (current_runtime_paths, current_session_paths) = bounded_followed_log_paths(
        runtime_paths.refresh(runtime_log_paths_for_follow),
        session_paths.refresh(stream_session_log_paths),
    );
    retain_followed_logs(followed_runtime_logs, &current_runtime_paths);
    retain_followed_logs(followed_session_logs, &current_session_paths);
    for path in &current_runtime_paths {
        let state = followed_runtime_logs
            .entry(path.clone())
            .or_insert_with(|| FollowedLog::at_end(path));
        for event in collect_new_runtime_log_stream_items(path, state, true)? {
            print_log_stream_item(&event, json)?;
        }
    }
    for path in &current_session_paths {
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
    let (current_runtime_paths, current_session_paths) = bounded_followed_log_paths(
        runtime_paths.refresh(runtime_log_paths_for_follow),
        session_paths.refresh(stream_session_log_paths),
    );
    retain_followed_logs(followed_runtime_logs, &current_runtime_paths);
    retain_followed_logs(followed_session_logs, &current_session_paths);
    for path in &current_runtime_paths {
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
    for path in &current_session_paths {
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
    throughput: &OutputThroughput,
    header_profile: &mut Option<String>,
    header_detail: &mut Option<LogTuiHeaderDetail>,
    header_refresh_at: &mut Instant,
) {
    let latest_profile = throughput
        .active_profile()
        .or_else(|| latest_log_stream_profile(items).map(str::to_string));
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
        let message = "No transcript, upstream payload, or token events found.".to_string();
        crate::app_commands::print_user_stdout_panel(
            "Prodex Log",
            &[("Status".to_string(), "no events found".to_string())],
            std::slice::from_ref(&message),
        )?;
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
    if let LogStreamItem::LoadObservation(observation) = item {
        let key = load_observation_key(&observation);
        let previous = match items.back() {
            Some(LogStreamItem::LoadAggregate(aggregate)) => Some(aggregate),
            _ => None,
        };
        let plan = LogLoadAggregate::plan_observation(
            previous,
            &observation.event_name,
            &key,
            observation.run_id.as_deref(),
            now,
        );
        if plan.routine {
            return;
        }
        // ponytail: keep run IDs in a bounded vector; use a counter if identity detail stops helping.
        if plan.coalesce {
            let Some(LogStreamItem::LoadAggregate(aggregate)) = items.back_mut() else {
                panic!("Mojo log-load plan selected a missing aggregate");
            };
            aggregate.apply_plan(observation.event, key, observation.run_id, now, plan);
            return;
        }
        items.push_back(LogStreamItem::LoadAggregate(LogLoadAggregate::from_plan(
            observation.event,
            key,
            observation.run_id,
            now,
            plan,
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
    throughput_display: Option<OutputThroughputDisplay>,
) {
    render::render_log_stream_tui(frame, items, state, header_detail, throughput_display);
}

#[cfg(test)]
#[path = "log_command_tui/tests.rs"]
mod tests;
