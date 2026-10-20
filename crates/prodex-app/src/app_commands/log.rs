pub(crate) use self::log_command_tui::handle_log;
#[cfg(test)]
use self::log_command_tui::{log_snapshot_items, log_stream_tui_text};
pub(crate) use self::log_follow::{
    FollowedLog, FollowedLogPaths, collect_new_followed_lines, followed_log_map,
    retain_followed_logs,
};
pub(crate) use self::log_live::{LiveRuntimeLogSource, collect_live_log_items};
pub(crate) use self::log_load::{LogLoadAggregate, LogLoadObservation, is_routine_load_event};
use self::log_paths::recent_session_log_paths;
#[cfg(test)]
use self::log_stream::log_stream_item_json;
pub(crate) use self::log_stream::{
    LogStreamItem, collect_new_runtime_log_stream_items,
    collect_new_runtime_log_stream_items_for_tui_with_throughput,
    collect_new_runtime_log_stream_items_with_throughput, local_token_usage_event, log_event_label,
    print_log_stream_item, print_token_usage_event, print_transcript_event,
    print_upstream_payload_event,
};
pub(crate) use self::log_transcript::{
    TranscriptEvent, transcript_events_from_session_line, transcript_exact_visible_user_message,
};
#[cfg(test)]
use crate::app_commands::log_format::local_log_timestamp;
#[cfg(test)]
use crate::reports::InfoTokenUsageEvent;
use anyhow::Result;
#[cfg(test)]
use std::collections::VecDeque;
#[cfg(test)]
use std::env;
use std::fs;
#[cfg(test)]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
#[path = "log_completeness_tests.rs"]
mod completeness_tests;
#[cfg(test)]
#[path = "log_descriptor_tests.rs"]
mod descriptor_tests;
#[path = "log_command_tui.rs"]
mod log_command_tui;
#[path = "log_follow.rs"]
mod log_follow;
#[path = "log_live.rs"]
mod log_live;
#[path = "log_load.rs"]
mod log_load;
#[path = "log_paths.rs"]
mod log_paths;
#[path = "log_stream.rs"]
mod log_stream;
#[path = "log_transcript.rs"]
mod log_transcript;
#[path = "log_transcript_text.rs"]
mod log_transcript_text;
#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "log_throughput_tests.rs"]
mod throughput_tests;

const LOG_SNAPSHOT_TAIL_BYTES: usize = 1024 * 1024;
const SESSION_SNAPSHOT_TAIL_BYTES: usize = 2 * 1024 * 1024;
// ponytail: one shared 32-file budget across runtime and session followers; raise only with
// measured low-RLIMIT headroom.
pub(super) const LOG_FOLLOW_MAX_FILES: usize = 32;

pub(super) fn runtime_log_paths_for_follow() -> Vec<std::path::PathBuf> {
    super::collect_recent_runtime_log_paths(LOG_FOLLOW_MAX_FILES)
}

pub(super) fn bounded_followed_log_paths(
    runtime_paths: &[PathBuf],
    session_paths: &[PathBuf],
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    // Filesystem metadata and original PathBufs remain host-owned. Mojo
    // exclusively orders and balances runtime/session descriptor candidates.
    let input = runtime_paths
        .iter()
        .map(|path| (path, 0_u8))
        .chain(session_paths.iter().map(|path| (path, 1_u8)))
        .map(|(path, source)| {
            let modified = path_modified_time(path);
            let (before_epoch, absolute) = match modified.duration_since(UNIX_EPOCH) {
                Ok(duration) => (false, duration),
                Err(error) => (true, error.duration()),
            };
            prodex_mojo_core::log::LogFollowSelectionInput {
                path: path.as_os_str().as_encoded_bytes(),
                source,
                modified_before_unix_epoch: before_epoch,
                modified_abs_seconds: absolute.as_secs(),
                modified_abs_nanos: absolute.subsec_nanos(),
            }
        })
        .collect::<Vec<_>>();
    let plan = prodex_mojo_core::log::log_follow_selection_plan(&input, LOG_FOLLOW_MAX_FILES)
        .expect("compiled Mojo combined log-follow selection returned invalid output");
    let runtime = runtime_paths
        .iter()
        .zip(&plan.selected[..runtime_paths.len()])
        .filter(|(_, selected)| **selected)
        .map(|(path, _)| path.clone())
        .collect();
    let sessions = session_paths
        .iter()
        .zip(&plan.selected[runtime_paths.len()..])
        .filter(|(_, selected)| **selected)
        .map(|(path, _)| path.clone())
        .collect();
    (runtime, sessions)
}

fn path_modified_time(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(UNIX_EPOCH)
}

pub(crate) fn no_color_requested() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

#[cfg(test)]
pub(crate) use self::log_transcript::read_new_transcript_events;
pub(crate) use self::log_transcript::{collect_new_transcript_events, latest_transcript_event};
