use super::*;

pub fn collect_info_token_usage_summary_from_text(text: &str) -> InfoTokenUsageSummary {
    let mut summary = InfoTokenUsageSummary::default();
    for line in text.lines() {
        let Some((profile, usage)) = info_token_usage_from_line(line) else {
            continue;
        };
        summary.event_count += 1;
        summary.total.add(usage);
        let profile = summary.by_profile.entry(profile).or_default();
        profile.event_count += 1;
        profile.total.add(usage);
    }
    summary
}

pub fn collect_info_token_usage_summary_from_texts<I, S>(
    log_count: usize,
    texts: I,
) -> InfoTokenUsageSummary
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut summary = InfoTokenUsageSummary {
        log_count,
        ..InfoTokenUsageSummary::default()
    };
    for text in texts {
        summary.merge(collect_info_token_usage_summary_from_text(text.as_ref()));
    }
    summary
}

impl InfoTokenUsageSummary {
    pub fn merge(&mut self, other: InfoTokenUsageSummary) {
        self.event_count += other.event_count;
        self.total.add(other.total);
        for (name, other_profile) in other.by_profile {
            let profile = self.by_profile.entry(name).or_default();
            profile.event_count += other_profile.event_count;
            profile.total.add(other_profile.total);
        }
    }
}

impl InfoTokenUsageCounts {
    fn add(&mut self, other: InfoTokenUsageCounts) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(other.reasoning_tokens);
    }
}

pub fn info_token_usage_from_line(line: &str) -> Option<(String, InfoTokenUsageCounts)> {
    let event = info_token_usage_event_from_line(line)?;
    Some((
        event.profile,
        InfoTokenUsageCounts {
            input_tokens: event.input_tokens,
            cached_input_tokens: event.cached_input_tokens,
            output_tokens: event.output_tokens,
            reasoning_tokens: event.reasoning_tokens,
        },
    ))
}

pub fn info_token_usage_event_from_line(line: &str) -> Option<InfoTokenUsageEvent> {
    if !line.contains("token_usage") {
        return None;
    }
    let fields = info_runtime_parse_fields(line);
    Some(InfoTokenUsageEvent {
        timestamp: info_token_usage_timestamp(line),
        request: fields.get("request").and_then(|value| value.parse().ok()),
        profile: fields
            .get("profile")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        transport: fields
            .get("transport")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        source: fields
            .get("source")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        input_tokens: fields.get("input_tokens")?.parse::<u64>().ok()?,
        cached_input_tokens: fields
            .get("cached_input_tokens")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_default(),
        output_tokens: fields
            .get("output_tokens")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_default(),
        reasoning_tokens: fields
            .get("reasoning_tokens")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_default(),
    })
}

fn info_token_usage_timestamp(line: &str) -> String {
    if let Some(timestamp) = line
        .strip_prefix('[')
        .and_then(|line| line.split_once(']'))
        .map(|(timestamp, _)| timestamp)
    {
        return timestamp.to_string();
    }
    serde_json::from_str::<serde_json::Value>(line)
        .ok()
        .and_then(|value| value.get("timestamp")?.as_str().map(str::to_string))
        .unwrap_or_default()
}

pub fn format_info_token_usage_summary(summary: &InfoTokenUsageSummary) -> String {
    let by_profile =
        summary
            .by_profile
            .iter()
            .map(|(profile, data)| terminal_ui::TokenUsageProfileDisplay {
                profile,
                total: token_usage_counts(data.total),
            });

    terminal_ui::format_info_token_usage_summary_display(
        summary.event_count,
        summary.log_count,
        token_usage_counts(summary.total),
        by_profile,
    )
}

fn token_usage_counts(usage: InfoTokenUsageCounts) -> terminal_ui::TokenUsageCounts {
    terminal_ui::TokenUsageCounts {
        input_tokens: usage.input_tokens,
        cached_input_tokens: usage.cached_input_tokens,
        output_tokens: usage.output_tokens,
        reasoning_tokens: usage.reasoning_tokens,
    }
}
