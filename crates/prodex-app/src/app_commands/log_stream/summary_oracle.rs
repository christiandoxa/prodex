use super::*;

pub(super) fn operational_event_summary(
    event: &str,
    source: &str,
    fields: &BTreeMap<String, String>,
) -> String {
    let mut details = Vec::new();
    match source {
        "request" | "model" | "route" | "mcp" | "agent" | "tool" | "event" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "provider", "provider");
            add_log_detail(&mut details, fields, "model", "model");
            add_log_detail(&mut details, fields, "from_model", "from");
            add_log_detail(&mut details, fields, "to_model", "to");
            add_log_detail(&mut details, fields, "effort", "effort");
            add_log_detail(&mut details, fields, "transport", "transport");
            add_log_detail(&mut details, fields, "method", "method");
            add_log_detail(&mut details, fields, "command", "command");
            add_log_detail(&mut details, fields, "cwd", "cwd");
            add_log_detail(&mut details, fields, "arg_count", "args");
            add_log_detail(&mut details, fields, "env_count", "env");
            add_log_detail(&mut details, fields, "stdin_bytes", "stdin_bytes");
            add_log_detail(&mut details, fields, "timeout_ms", "timeout_ms");
            add_log_endpoint_detail(&mut details, fields, "path", "path");
            add_log_endpoint_detail(&mut details, fields, "url", "path");
            add_log_detail(&mut details, fields, "tool_surface", "tools");
            add_log_detail(&mut details, fields, "continuation", "continuation");
            add_log_detail(&mut details, fields, "status", "status");
            add_log_detail(&mut details, fields, "class", "class");
            add_log_detail(&mut details, fields, "event_type", "event");
            add_log_detail(&mut details, fields, "state", "state");
            add_log_detail(&mut details, fields, "code", "code");
            add_log_detail(&mut details, fields, "reason", "reason");
            add_log_detail(&mut details, fields, "elapsed_ms", "latency_ms");
            add_log_detail(&mut details, fields, "duration_ms", "duration_ms");
            add_log_detail(&mut details, fields, "exit_code", "exit");
            add_log_detail(&mut details, fields, "exit_status", "exit");
            add_log_detail(&mut details, fields, "outcome", "outcome");
            add_log_detail(&mut details, fields, "active", "active");
            add_log_detail(&mut details, fields, "limit", "limit");
            add_log_detail(&mut details, fields, "count", "count");
            add_log_detail(&mut details, fields, "dropped", "dropped");
        }
        "quota" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "quota_band", "band");
            add_log_percent_detail(&mut details, fields, "five_hour_remaining", "5h");
            add_log_percent_detail(&mut details, fields, "weekly_remaining", "week");
            add_log_detail(&mut details, fields, "reason", "reason");
            add_log_detail(&mut details, fields, "until", "until");
        }
        "retry" | "backoff" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "provider", "provider");
            add_log_detail(&mut details, fields, "reason", "reason");
            add_log_detail(&mut details, fields, "class", "class");
            add_log_detail(&mut details, fields, "attempt", "attempt");
            add_log_detail(&mut details, fields, "retry_index", "retry");
            add_log_detail(&mut details, fields, "seconds", "backoff_s");
            add_log_detail(&mut details, fields, "until", "until");
        }
        "health" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "score", "score");
            add_log_detail(&mut details, fields, "delta", "delta");
            add_log_detail(&mut details, fields, "reason", "reason");
        }
        "upstream" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "transport", "transport");
            add_log_detail(&mut details, fields, "method", "method");
            add_log_endpoint_detail(&mut details, fields, "url", "path");
            add_log_detail(&mut details, fields, "status", "status");
            add_log_detail(&mut details, fields, "elapsed_ms", "latency_ms");
            add_log_detail(&mut details, fields, "reason", "reason");
        }
        "stream" | "response" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "transport", "transport");
            if event == "first_local_chunk" {
                add_log_detail(&mut details, fields, "elapsed_ms", "ttft_ms");
            } else {
                add_log_detail(&mut details, fields, "elapsed_ms", "latency_ms");
            }
            add_log_detail(&mut details, fields, "chunks", "chunks");
            add_log_detail(&mut details, fields, "bytes", "bytes");
            add_log_detail(&mut details, fields, "status", "status");
            add_log_detail(&mut details, fields, "event_type", "event");
        }
        "smart" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "decision", "decision");
            add_log_detail(&mut details, fields, "tier", "tier");
            add_log_detail(&mut details, fields, "rewrite_kind", "rewrite");
            add_log_detail(&mut details, fields, "tokens_before", "tokens_before");
            add_log_detail(&mut details, fields, "tokens_after", "tokens_after");
            add_log_detail(&mut details, fields, "body_bytes_saved", "bytes_saved");
            add_log_percent_detail(&mut details, fields, "rewrite_ratio_percent", "rewrite");
            add_log_detail(
                &mut details,
                fields,
                "tool_outputs_condensed",
                "tools_condensed",
            );
            add_log_detail(&mut details, fields, "rehydrated_refs", "rehydrated");
            add_log_detail(&mut details, fields, "pressure_band", "pressure");
            add_log_detail(&mut details, fields, "self_check", "check");
            add_log_detail(&mut details, fields, "reason", "reason");
        }
        "compact" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "provider", "provider");
            add_log_detail(&mut details, fields, "status", "status");
            add_log_detail(&mut details, fields, "decision", "decision");
            add_log_detail(&mut details, fields, "exit", "exit");
            add_log_detail(&mut details, fields, "reason", "reason");
            add_log_detail(&mut details, fields, "attempts", "attempts");
            add_log_detail(&mut details, fields, "elapsed_ms", "latency_ms");
        }
        "load" => {
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "lane", "lane");
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "active", "active");
            add_log_detail(&mut details, fields, "limit", "limit");
            add_log_detail(&mut details, fields, "hard_limit", "limit");
            add_log_detail(&mut details, fields, "reason", "reason");
        }
        "terminal" | "error" => {
            add_log_detail(&mut details, fields, "profile", "profile");
            add_log_detail(&mut details, fields, "route", "route");
            add_log_detail(&mut details, fields, "transport", "transport");
            add_log_detail(&mut details, fields, "stage", "stage");
            add_log_detail(&mut details, fields, "event_type", "event");
            add_log_detail(&mut details, fields, "status", "status");
            add_log_detail(&mut details, fields, "class", "class");
            add_log_detail(&mut details, fields, "reason", "reason");
            add_log_detail(&mut details, fields, "outcome", "outcome");
            add_log_detail(&mut details, fields, "exit_code", "exit");
            add_log_detail(&mut details, fields, "exit_status", "exit");
            add_log_detail(&mut details, fields, "dropped", "dropped");
        }
        _ => {}
    }
    join_log_details(&human_event_name(event), details)
}
