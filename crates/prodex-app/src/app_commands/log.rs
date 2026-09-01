pub(crate) use self::log_command_tui::handle_log;
#[cfg(test)]
use self::log_command_tui::{log_snapshot_items, log_stream_tui_text};
pub(crate) use self::log_follow::{
    FollowedLog, FollowedLogPaths, collect_new_followed_lines, retain_followed_logs,
};
pub(crate) use self::log_live::LiveRuntimeLogSource;
use self::log_paths::recent_session_log_paths;
#[cfg(test)]
use self::log_stream::log_stream_item_json;
pub(crate) use self::log_stream::{
    LogLoadAggregate, LogLoadObservation, LogStreamItem, collect_new_runtime_log_stream_items,
    collect_new_runtime_log_stream_items_for_tui_with_throughput,
    collect_new_runtime_log_stream_items_with_throughput, collect_runtime_log_line,
    is_routine_load_event, local_token_usage_event, log_event_label, print_log_stream_item,
    print_token_usage_event, print_transcript_event, print_upstream_payload_event,
};
pub(crate) use self::log_transcript::{TranscriptEvent, transcript_events_from_session_line};
#[cfg(test)]
use crate::app_commands::log_format::local_log_timestamp;
#[cfg(test)]
use crate::reports::InfoTokenUsageEvent;
use anyhow::Result;
#[cfg(test)]
use std::collections::VecDeque;
#[cfg(test)]
use std::env;
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::io::Write;
#[cfg(test)]
use std::time::SystemTime;
#[cfg(test)]
use std::time::UNIX_EPOCH;

#[path = "log_command_tui.rs"]
mod log_command_tui;
#[path = "log_follow.rs"]
mod log_follow;
#[path = "log_live.rs"]
mod log_live;
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

const LOG_SNAPSHOT_TAIL_BYTES: usize = 1024 * 1024;
const SESSION_SNAPSHOT_TAIL_BYTES: usize = 2 * 1024 * 1024;

pub(crate) fn no_color_requested() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

#[cfg(test)]
pub(crate) use self::log_transcript::read_new_transcript_events;
pub(crate) use self::log_transcript::{collect_new_transcript_events, latest_transcript_event};
