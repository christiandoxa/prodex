#[cfg(not(feature = "mojo"))]
mod implementation {
    pub(super) fn metric_name(plan: usize, slot: usize) -> &'static str {
        match (plan, slot) {
            (7, 0) => "prodex_api_admission_decisions_total",
            (17, 0) => "prodex_api_requests_total",
            (17, 1) => "prodex_api_request_duration_ms",
            (44, 0) => "prodex_secret_provider_operations_total",
            (50, 0) => "prodex_provider_requests_total",
            (50, 1) => "prodex_provider_request_duration_ms",
            (63, 0) => "prodex_inspection_events_total",
            (63, 1) => "prodex_inspection_duration_microseconds",
            _ => panic!("unsupported feature-off observability metric plan {plan}:{slot}"),
        }
    }

    pub(super) fn planned_metric_label(
        plan: usize,
        slot: usize,
        value: i64,
    ) -> (&'static str, &'static str) {
        match (plan, slot) {
            (7, 0) => ("api_admission_route", api_route(value)),
            (7, 1) => ("api_admission_result", admission_result(value)),
            (17, 0) => ("api_route", api_route(value)),
            (17, 1) => ("status_class", status_class(value)),
            (44, 0) => ("secret_backend", secret_backend(value)),
            (44, 1) => ("secret_operation", secret_operation(value)),
            (44, 2) => ("secret_result", secret_result(value)),
            (50, 0) => ("provider", provider(value)),
            (50, 1) => ("provider_result", provider_result(value)),
            (63, 0) => ("inspection_stage", inspection_stage(value)),
            (63, 1) => ("inspection_coverage", inspection_coverage(value)),
            (63, 2) => (
                "inspection_finding_category",
                inspection_finding_category(value),
            ),
            (63, 3) => (
                "inspection_masking_action",
                inspection_masking_action(value),
            ),
            (63, 4) => ("inspection_outcome", inspection_outcome(value)),
            _ => panic!("unsupported feature-off observability label plan {plan}:{slot}"),
        }
    }

    fn api_route(value: i64) -> &'static str {
        match value {
            0 => "responses",
            1 => "compact",
            2 => "websocket",
            3 => "control_plane",
            4 => "health",
            _ => panic!("invalid api route label value"),
        }
    }

    fn admission_result(value: i64) -> &'static str {
        match value {
            0 => "accepted",
            1 => "global_limit_reached",
            2 => "route_limit_reached",
            3 => "queue_full",
            4 => "draining",
            _ => panic!("invalid admission result label value"),
        }
    }

    fn status_class(value: i64) -> &'static str {
        match value {
            0 => "1xx",
            1 => "2xx",
            2 => "3xx",
            3 => "4xx",
            4 => "5xx",
            _ => panic!("invalid status class label value"),
        }
    }

    fn provider(value: i64) -> &'static str {
        match value {
            0 => "openai",
            1 => "anthropic",
            2 => "gemini",
            3 => "local",
            4 => "other",
            _ => panic!("invalid provider label value"),
        }
    }

    fn provider_result(value: i64) -> &'static str {
        match value {
            0 => "success",
            1 => "rate_limited",
            2 => "overloaded",
            3 => "provider_error",
            4 => "transport_error",
            _ => panic!("invalid provider result label value"),
        }
    }

    fn secret_backend(value: i64) -> &'static str {
        match value {
            0 => "file",
            1 => "keyring",
            2 => "external_manager",
            _ => panic!("invalid secret backend label value"),
        }
    }

    fn secret_operation(value: i64) -> &'static str {
        match value {
            0 => "read",
            1 => "write",
            2 => "delete",
            3 => "revision_lookup",
            _ => panic!("invalid secret operation label value"),
        }
    }

    fn secret_result(value: i64) -> &'static str {
        match value {
            0 => "success",
            1 => "not_found",
            2 => "unsupported",
            3 => "failed",
            _ => panic!("invalid secret result label value"),
        }
    }

    fn inspection_stage(value: i64) -> &'static str {
        match value {
            0 => "local",
            1 => "external",
            2 => "merge",
            3 => "request_enforcement",
            4 => "response_enforcement",
            _ => panic!("invalid inspection stage label value"),
        }
    }

    fn inspection_coverage(value: i64) -> &'static str {
        match value {
            0 => "full",
            1 => "partial",
            2 => "unsupported",
            _ => panic!("invalid inspection coverage label value"),
        }
    }

    fn inspection_finding_category(value: i64) -> &'static str {
        match value {
            0 => "none",
            1 => "personal_data",
            2 => "credential",
            3 => "financial",
            4 => "multiple",
            _ => panic!("invalid inspection finding label value"),
        }
    }

    fn inspection_masking_action(value: i64) -> &'static str {
        match value {
            0 => "none",
            1 => "masked",
            2 => "denied",
            _ => panic!("invalid inspection masking label value"),
        }
    }

    fn inspection_outcome(value: i64) -> &'static str {
        match value {
            0 => "allowed",
            1 => "denied",
            2 => "timeout",
            3 => "error",
            _ => panic!("invalid inspection outcome label value"),
        }
    }
}

#[cfg(not(feature = "mojo"))]
pub(super) fn metric_name(plan: usize, slot: usize) -> &'static str {
    implementation::metric_name(plan, slot)
}

#[cfg(not(feature = "mojo"))]
pub(super) fn planned_metric_label(
    plan: usize,
    slot: usize,
    value: i64,
) -> (&'static str, &'static str) {
    implementation::planned_metric_label(plan, slot, value)
}
