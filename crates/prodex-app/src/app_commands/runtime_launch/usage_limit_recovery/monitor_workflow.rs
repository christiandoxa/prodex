use super::*;

pub(crate) fn wait_for_runtime_recovery_round() -> bool {
    crate::print_launch_status("transient profile pool unavailable; retrying in 5 seconds");
    #[cfg(any(unix, windows))]
    {
        let Ok(_sigint) = crate::InteractiveSigintGuard::install() else {
            std::thread::sleep(GOAL_USAGE_LIMIT_RETRY_INTERVAL);
            return true;
        };
        let deadline = Instant::now() + GOAL_USAGE_LIMIT_RETRY_INTERVAL;
        while Instant::now() < deadline {
            if crate::InteractiveSigintGuard::count() > 0 {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        true
    }
    #[cfg(not(any(unix, windows)))]
    {
        std::thread::sleep(GOAL_USAGE_LIMIT_RETRY_INTERVAL);
        true
    }
}

impl GoalUsageLimitMonitor {
    pub(super) fn observe_workflow_recovery(
        &mut self,
        session_id: &str,
    ) -> Result<Option<RuntimeWorkflowRecoveryClass>> {
        self.workflow_activity_observed = false;
        if self
            .workflow_scan_retry_at
            .is_some_and(|retry_at| Instant::now() < retry_at)
        {
            return Ok(None);
        }
        let Some(path) = self.workflow_session_path(session_id)? else {
            return Ok(None);
        };
        let mut recovery = None;
        let mut clear_pending = false;
        let mut model = self.workflow_model.clone();
        let mut evidence = self.workflow_evidence;
        let scan = match prodex_session_store::session_file_scan_since(
            &path,
            self.session_scan_offset,
            |line| {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                    return false;
                };
                if !workflow::record_matches_session(&value, session_id) {
                    return false;
                }
                if let Some(observed) = runtime_workflow_effective_model(&value) {
                    model = Some(observed);
                }
                observe_runtime_workflow_evidence(&value, &mut evidence);
                let class = runtime_workflow_recovery_class(&value, session_id);
                let action = prodex_mojo_core::launch::recovery_observation_action(
                    workflow::live_recovery_observation_kind(&value),
                    true,
                    class.is_some(),
                )
                .expect("Mojo native recovery observation returned invalid output");
                match action {
                    prodex_mojo_core::launch::RecoveryObservationAction::Ignore => {}
                    prodex_mojo_core::launch::RecoveryObservationAction::Clear => {
                        recovery = None;
                        clear_pending = true;
                    }
                    prodex_mojo_core::launch::RecoveryObservationAction::Arm => {
                        recovery = class;
                        clear_pending = false;
                    }
                }
                // Drain complete bounded tail records. Stopping at the first
                // failure can miss a subsequent retry, completion or user stop.
                false
            },
        ) {
            Ok(scan) => scan,
            Err(_) => {
                // A transient file/decoder failure must not mute recovery for
                // the rest of the native session. Keep cursor and evidence,
                // and retry on a bounded schedule without replaying history.
                self.workflow_scan_retry_at =
                    Some(Instant::now() + GOAL_USAGE_LIMIT_RETRY_INTERVAL);
                return Ok(None);
            }
        };
        self.workflow_scan_retry_at = None;
        self.session_scan_offset = scan.complete_offset;
        self.workflow_model = model;
        self.workflow_evidence = evidence;
        self.workflow_activity_observed = clear_pending;
        if clear_pending {
            self.usage_limit_pending = false;
            self.workflow_recovery_class = None;
            self.session_usage_limit_reported = false;
        }
        Ok(recovery)
    }

    fn workflow_session_path(&mut self, session_id: &str) -> Result<Option<PathBuf>> {
        if let Some(path) = self.session_path.as_ref()
            && path.exists()
        {
            return Ok(Some(path.clone()));
        }
        let state = AppState::load_and_repair(&self.paths)?;
        let report = match prodex_session_store::resolve_session_report_by_id_in_store(
            &self.paths.shared_codex_root,
            &state,
            session_id,
        ) {
            Ok(report) => report,
            Err(prodex_session_store::SessionResolveError::Missing { .. }) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if self.workflow_model.is_none() {
            self.workflow_model = report.last_model().map(ToOwned::to_owned);
        }
        let path = PathBuf::from(report.path);
        self.session_path = Some(path.clone());
        Ok(Some(path))
    }

    pub(crate) fn recovery_failure_class(&self) -> &'static str {
        self.workflow_recovery_class
            .map(RuntimeWorkflowRecoveryClass::as_str)
            .unwrap_or("usage_limit")
    }

    pub(crate) fn recovery_retries_after_pool_round(&self) -> bool {
        self.workflow_recovery_class
            .is_some_and(RuntimeWorkflowRecoveryClass::retries_after_pool_round)
    }

    pub(crate) fn has_session_goal(&self) -> bool {
        self.session_goal_present
    }

    pub(crate) fn recovery_model(&self) -> Option<&str> {
        self.workflow_model.as_deref()
    }

    pub(crate) fn recovery_evidence(&self) -> RuntimeWorkflowEvidence {
        self.workflow_evidence
    }
}
