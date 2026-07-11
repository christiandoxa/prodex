use super::*;

pub(super) struct RuntimeProxyProfileHarness {
    _temp_dir: TestDir,
    shared: RuntimeRotationProxyShared,
}

impl RuntimeProxyProfileHarness {
    pub(super) fn shared(&self) -> &RuntimeRotationProxyShared {
        &self.shared
    }
}

pub(super) struct RuntimeProxyProfileHarnessBuilder {
    profiles: BTreeMap<String, RuntimeProxyProfileSpec>,
    active_profile: Option<String>,
    current_profile: Option<String>,
    upstream_base_url: String,
    profile_usage_snapshots: BTreeMap<String, RuntimeProfileUsageSnapshot>,
    active_request_limit: usize,
}

struct RuntimeProxyProfileSpec {
    account_id: String,
    email: Option<String>,
}

impl RuntimeProxyProfileHarnessBuilder {
    pub(super) fn single_openai_profile(name: &str, account_id: &str, email: &str) -> Self {
        Self::new()
            .openai_profile(name, account_id, Some(email))
            .active_profile(name)
            .current_profile(name)
    }

    pub(super) fn new() -> Self {
        Self {
            profiles: BTreeMap::new(),
            active_profile: None,
            current_profile: None,
            upstream_base_url: "http://127.0.0.1:1/backend-api".to_string(),
            profile_usage_snapshots: BTreeMap::new(),
            active_request_limit: usize::MAX,
        }
    }

    pub(super) fn openai_profile(
        mut self,
        name: &str,
        account_id: &str,
        email: Option<&str>,
    ) -> Self {
        self.profiles.insert(
            name.to_string(),
            RuntimeProxyProfileSpec {
                account_id: account_id.to_string(),
                email: email.map(str::to_string),
            },
        );
        self
    }

    pub(super) fn active_profile(mut self, profile_name: &str) -> Self {
        self.active_profile = Some(profile_name.to_string());
        self
    }

    pub(super) fn current_profile(mut self, profile_name: &str) -> Self {
        self.current_profile = Some(profile_name.to_string());
        self
    }

    pub(super) fn upstream_base_url(mut self, upstream_base_url: impl Into<String>) -> Self {
        self.upstream_base_url = upstream_base_url.into();
        self
    }

    pub(super) fn active_request_limit(mut self, active_request_limit: usize) -> Self {
        self.active_request_limit = active_request_limit;
        self
    }

    pub(super) fn profile_usage_snapshot(
        mut self,
        profile_name: &str,
        snapshot: RuntimeProfileUsageSnapshot,
    ) -> Self {
        self.profile_usage_snapshots
            .insert(profile_name.to_string(), snapshot);
        self
    }

    pub(super) fn build(self) -> RuntimeProxyProfileHarness {
        let temp_dir = TestDir::isolated();
        let paths = AppPaths {
            root: temp_dir.path.join("prodex"),
            state_file: temp_dir.path.join("prodex/state.json"),
            managed_profiles_root: temp_dir.path.join("prodex/profiles"),
            shared_codex_root: temp_dir.path.join("shared"),
            legacy_shared_codex_root: temp_dir.path.join("prodex/shared"),
        };

        let profiles = self
            .profiles
            .into_iter()
            .map(|(profile_name, profile)| {
                let codex_home = temp_dir.path.join(format!("homes/{profile_name}"));
                write_auth_json(&codex_home.join("auth.json"), &profile.account_id);
                (
                    profile_name,
                    ProfileEntry {
                        codex_home,
                        managed: true,
                        email: profile.email,
                        provider: ProfileProvider::Openai,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        let active_profile = self
            .active_profile
            .or_else(|| profiles.keys().next().cloned());
        let current_profile = self
            .current_profile
            .or_else(|| active_profile.clone())
            .unwrap_or_else(|| "main".to_string());

        let runtime = RuntimeRotationState {
            paths: paths.clone(),
            state: AppState {
                active_profile,
                profiles,
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: self.upstream_base_url,
            include_code_review: false,
            current_profile,
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings: BTreeMap::new(),
            session_id_bindings: BTreeMap::new(),
            continuation_statuses: RuntimeContinuationStatuses::default(),
            profile_probe_cache: BTreeMap::new(),
            profile_usage_snapshots: self.profile_usage_snapshots,
            profile_retry_backoff_until: BTreeMap::new(),
            profile_transport_backoff_until: BTreeMap::new(),
            profile_route_circuit_open_until: BTreeMap::new(),
            profile_inflight: BTreeMap::new(),
            profile_health: BTreeMap::new(),
        };
        let shared = runtime_rotation_proxy_shared(&temp_dir, runtime, self.active_request_limit);

        RuntimeProxyProfileHarness {
            _temp_dir: temp_dir,
            shared,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeQuotaWindowSnapshot {
    status: RuntimeQuotaWindowStatus,
    remaining_percent: i64,
    reset_offset_seconds: i64,
}

pub(super) fn quota_window_ready(
    remaining_percent: i64,
    reset_offset_seconds: i64,
) -> RuntimeQuotaWindowSnapshot {
    RuntimeQuotaWindowSnapshot {
        status: RuntimeQuotaWindowStatus::Ready,
        remaining_percent,
        reset_offset_seconds,
    }
}

pub(super) fn quota_window_exhausted(reset_offset_seconds: i64) -> RuntimeQuotaWindowSnapshot {
    RuntimeQuotaWindowSnapshot {
        status: RuntimeQuotaWindowStatus::Exhausted,
        remaining_percent: 0,
        reset_offset_seconds,
    }
}

pub(super) fn runtime_usage_snapshot(
    five_hour: RuntimeQuotaWindowSnapshot,
    weekly: RuntimeQuotaWindowSnapshot,
) -> RuntimeProfileUsageSnapshot {
    let now = Local::now().timestamp();
    RuntimeProfileUsageSnapshot {
        checked_at: now,
        five_hour_status: five_hour.status,
        five_hour_remaining_percent: five_hour.remaining_percent,
        five_hour_reset_at: now + five_hour.reset_offset_seconds,
        weekly_status: weekly.status,
        weekly_remaining_percent: weekly.remaining_percent,
        weekly_reset_at: now + weekly.reset_offset_seconds,
    }
}
