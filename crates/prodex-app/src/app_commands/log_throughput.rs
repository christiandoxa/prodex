#[cfg(test)]
use super::log_throughput_state::OUTPUT_THROUGHPUT_MIN_SAMPLE;
pub(crate) use super::log_throughput_state::OutputThroughput;
#[cfg(test)]
use crate::reports::InfoTokenUsageEvent;
pub(super) fn format_output_tokens_per_second(rate: Option<f64>) -> String {
    let Some(rate) = rate.filter(|rate| rate.is_finite() && *rate > 0.0) else {
        return "— t/s".to_string();
    };
    if rate >= 100.0 {
        format!("{rate:.0} t/s")
    } else {
        let rounded = (rate * 10.0).round() / 10.0;
        if rounded.fract() == 0.0 {
            format!("{rounded:.0} t/s")
        } else {
            format!("{rounded:.1} t/s")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InfoTokenUsageEvent, OUTPUT_THROUGHPUT_MIN_SAMPLE, OutputThroughput,
        format_output_tokens_per_second,
    };
    use std::path::Path;
    use std::time::{Duration, Instant};

    fn usage(profile: &str, request: Option<u64>, output_tokens: u64) -> InfoTokenUsageEvent {
        InfoTokenUsageEvent {
            profile: profile.to_string(),
            request,
            output_tokens,
            ..InfoTokenUsageEvent::default()
        }
    }

    fn active_rate(throughput: &mut OutputThroughput, now: Instant) -> Option<f64> {
        throughput.active_rate_for_profile(now, None)
    }

    fn display_rate(throughput: &mut OutputThroughput, now: Instant) -> Option<f64> {
        throughput.display_rate_for_profile(now, None)
    }

    #[test]
    fn output_throughput_uses_recent_output_deltas_only() {
        let path = Path::new("/tmp/runtime-a.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &usage("main", Some(7), 50), start);
        throughput.observe_token_usage(
            path,
            &usage("main", Some(7), 150),
            start + Duration::from_secs(1),
        );

        assert_eq!(
            active_rate(&mut throughput, start + Duration::from_secs(1)),
            Some(100.0)
        );
        assert_eq!(format_output_tokens_per_second(Some(100.0)), "100 t/s");
    }

    #[test]
    fn output_throughput_stays_blank_until_a_valid_sample_window() {
        let path = Path::new("/tmp/runtime-b.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &usage("main", Some(8), 1), start);
        assert!(active_rate(&mut throughput, start + Duration::from_millis(1)).is_none());
        assert_eq!(format_output_tokens_per_second(None), "— t/s");

        throughput.observe_token_usage(
            path,
            &usage("main", Some(8), 2),
            start + OUTPUT_THROUGHPUT_MIN_SAMPLE,
        );
        assert!(
            throughput
                .active_rate_for_profile(start + OUTPUT_THROUGHPUT_MIN_SAMPLE, None)
                .is_some()
        );
    }

    #[test]
    fn output_throughput_rejects_zero_elapsed_and_counter_resets() {
        let path = Path::new("/tmp/runtime-c.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &usage("main", Some(9), 10), start);
        throughput.observe_token_usage(path, &usage("main", Some(9), 10), start);
        assert!(active_rate(&mut throughput, start).is_none());

        let event = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(9),
            output_tokens: 100,
            ..InfoTokenUsageEvent::default()
        };
        throughput.observe_token_usage(path, &event, start);
        let reset_event = InfoTokenUsageEvent {
            output_tokens: 0,
            ..event
        };
        throughput.observe_token_usage(path, &reset_event, start + Duration::from_secs(1));
        assert!(active_rate(&mut throughput, start + Duration::from_secs(1)).is_none());
    }

    #[test]
    fn final_only_usage_retains_a_brief_average_without_claiming_active_generation() {
        let path = Path::new("/tmp/runtime-d.log");
        let now = Instant::now();
        let event = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(10),
            output_tokens: 1_000,
            generation_ms: Some(10_000),
            output_tokens_per_second: Some(100.0),
            ..InfoTokenUsageEvent::default()
        };
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &event, now);
        throughput.finish(path, &event);

        assert!(active_rate(&mut throughput, now).is_none());
        assert_eq!(display_rate(&mut throughput, Instant::now()), Some(100.0));
        for rate in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            assert_eq!(format_output_tokens_per_second(Some(rate)), "— t/s");
        }
        assert_eq!(format_output_tokens_per_second(Some(0.7)), "0.7 t/s");
    }

    #[test]
    fn duplicate_live_and_disk_samples_do_not_downgrade_idle_rate() {
        let live = Path::new("broker:runtime:instance");
        let disk = Path::new("/tmp/runtime-duplicate.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        let first = InfoTokenUsageEvent {
            timestamp: "2026-08-28 12:00:00.000 +07:00".to_string(),
            ..usage("main", Some(12), 100)
        };
        let second = InfoTokenUsageEvent {
            timestamp: "2026-08-28 12:00:01.000 +07:00".to_string(),
            generation_ms: Some(1_000),
            output_tokens_per_second: Some(100.0),
            ..usage("main", Some(12), 200)
        };

        throughput.observe_token_usage(live, &first, start);
        throughput.observe_token_usage(live, &second, start + Duration::from_secs(1));
        assert_eq!(
            active_rate(&mut throughput, start + Duration::from_secs(1)),
            Some(100.0)
        );

        throughput.observe_token_usage(disk, &first, start + Duration::from_secs(2));
        throughput.observe_token_usage(disk, &second, start + Duration::from_secs(4));
        throughput.finish(
            disk,
            &InfoTokenUsageEvent {
                output_tokens_per_second: Some(40.0),
                ..second.clone()
            },
        );

        assert_eq!(
            display_rate(&mut throughput, start + Duration::from_secs(4)),
            Some(100.0)
        );
    }

    #[test]
    fn later_turn_refreshes_the_header_rate() {
        let path = Path::new("/tmp/runtime-later-turn.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &usage("main", Some(1), 100), start);
        throughput.observe_token_usage(
            path,
            &usage("main", Some(1), 200),
            start + Duration::from_secs(1),
        );
        throughput.finish(
            path,
            &InfoTokenUsageEvent {
                profile: "main".to_string(),
                request: Some(1),
                output_tokens: 200,
                output_tokens_per_second: Some(100.0),
                ..InfoTokenUsageEvent::default()
            },
        );

        throughput.observe_token_usage(
            path,
            &usage("main", Some(2), 10),
            start + Duration::from_secs(2),
        );
        throughput.observe_token_usage(
            path,
            &usage("main", Some(2), 40),
            start + Duration::from_secs(3),
        );

        assert_eq!(
            display_rate(&mut throughput, start + Duration::from_secs(3)),
            Some(30.0)
        );
    }

    #[test]
    fn profile_rotation_keeps_token_counters_separate() {
        let path = Path::new("/tmp/runtime-profile-rotation.log");
        let start = Instant::now();
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &usage("main", Some(3), 100), start);
        throughput.observe_token_usage(
            path,
            &usage("main", Some(3), 200),
            start + Duration::from_secs(1),
        );
        throughput.finish(
            path,
            &InfoTokenUsageEvent {
                profile: "main".to_string(),
                request: Some(3),
                output_tokens: 200,
                output_tokens_per_second: Some(100.0),
                ..InfoTokenUsageEvent::default()
            },
        );

        throughput.observe_token_usage(
            path,
            &usage("backup", Some(4), 5),
            start + Duration::from_secs(2),
        );
        throughput.observe_token_usage(
            path,
            &usage("backup", Some(4), 25),
            start + Duration::from_secs(3),
        );

        assert_eq!(
            active_rate(&mut throughput, start + Duration::from_secs(3)),
            Some(20.0)
        );
    }

    #[test]
    fn profile_specific_display_does_not_reuse_another_profile_rate() {
        let path = Path::new("/tmp/runtime-profile-display.log");
        let now = Instant::now();
        let mut throughput = OutputThroughput::default();
        let main = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(20),
            output_tokens: 100,
            generation_ms: Some(1_000),
            output_tokens_per_second: Some(100.0),
            ..InfoTokenUsageEvent::default()
        };
        throughput.observe_token_usage(path, &main, now);
        throughput.finish(path, &main);
        throughput.observe_token_usage(
            path,
            &usage("backup", Some(21), 1),
            now + Duration::from_secs(1),
        );

        assert_eq!(
            throughput.display_rate_for_profile(now + Duration::from_secs(1), Some("backup")),
            None
        );
        assert_eq!(
            throughput.display_rate_for_profile(now + Duration::from_secs(1), Some("main")),
            Some(100.0)
        );
    }

    #[test]
    fn invalid_final_duration_does_not_replace_a_rate() {
        let path = Path::new("/tmp/runtime-invalid-duration.log");
        let now = Instant::now();
        let event = InfoTokenUsageEvent {
            profile: "main".to_string(),
            request: Some(5),
            output_tokens: 50,
            generation_ms: Some(0),
            output_tokens_per_second: Some(50.0),
            ..InfoTokenUsageEvent::default()
        };
        let mut throughput = OutputThroughput::default();
        throughput.observe_token_usage(path, &event, now);
        throughput.finish(path, &event);

        assert!(display_rate(&mut throughput, now).is_none());
    }

    #[test]
    fn historical_rate_seeds_sticky_display_without_starting_live_sampling() {
        let path = Path::new("/tmp/runtime-history.log");
        let mut throughput = OutputThroughput::default();
        throughput.observe_historical(
            path,
            &InfoTokenUsageEvent {
                timestamp: "2026-08-28 12:00:00".to_string(),
                profile: "main".to_string(),
                request: Some(13),
                output_tokens: 500,
                output_tokens_per_second: Some(66.3),
                ..InfoTokenUsageEvent::default()
            },
        );

        assert_eq!(display_rate(&mut throughput, Instant::now()), Some(66.3));
    }

    #[test]
    fn historical_rates_remain_available_per_profile() {
        let path = Path::new("/tmp/runtime-profile-history.log");
        let mut throughput = OutputThroughput::default();
        throughput.observe_historical(
            path,
            &InfoTokenUsageEvent {
                timestamp: "2026-08-28 12:00:02".to_string(),
                profile: "backup".to_string(),
                request: Some(14),
                output_tokens: 200,
                output_tokens_per_second: Some(20.0),
                ..InfoTokenUsageEvent::default()
            },
        );
        throughput.observe_historical(
            path,
            &InfoTokenUsageEvent {
                timestamp: "2026-08-28 12:00:01".to_string(),
                profile: "main".to_string(),
                request: Some(15),
                output_tokens: 100,
                output_tokens_per_second: Some(10.0),
                ..InfoTokenUsageEvent::default()
            },
        );

        assert_eq!(
            throughput.display_rate_for_profile(Instant::now(), Some("backup")),
            Some(20.0)
        );
        assert_eq!(
            throughput.display_rate_for_profile(Instant::now(), Some("main")),
            Some(10.0)
        );
        assert_eq!(display_rate(&mut throughput, Instant::now()), Some(20.0));
    }

    #[test]
    fn historical_final_then_live_replay_keeps_authoritative_idle_rate() {
        let disk = Path::new("/tmp/runtime-history-replay.log");
        let live = Path::new("broker:runtime:instance");
        let start = Instant::now();
        let completed = InfoTokenUsageEvent {
            timestamp: "2026-08-28 12:00:01".to_string(),
            profile: "main".to_string(),
            request: Some(16),
            output_tokens: 200,
            generation_ms: Some(2_500),
            output_tokens_per_second: Some(80.0),
            ..InfoTokenUsageEvent::default()
        };
        let mut throughput = OutputThroughput::default();
        throughput.observe_historical(disk, &completed);
        throughput.observe_token_usage(
            live,
            &InfoTokenUsageEvent {
                output_tokens: 100,
                generation_ms: None,
                output_tokens_per_second: None,
                ..completed.clone()
            },
            start,
        );
        throughput.observe_token_usage(
            live,
            &InfoTokenUsageEvent {
                output_tokens: 200,
                generation_ms: None,
                output_tokens_per_second: None,
                ..completed.clone()
            },
            start + Duration::from_secs(1),
        );
        throughput.observe_token_usage(live, &completed, start + Duration::from_secs(1));
        throughput.finish(live, &completed);

        assert!(active_rate(&mut throughput, start + Duration::from_secs(1)).is_none());
        assert_eq!(
            display_rate(&mut throughput, start + Duration::from_secs(1)),
            Some(80.0)
        );
    }
}
