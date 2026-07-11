use std::fmt;

use serde::{Deserialize, Serialize};

use crate::PolicyRevisionId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Passing,
    Degraded,
    Failing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthProbeKind {
    Livez,
    Readyz,
    Startupz,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub state: HealthState,
    pub message: Option<String>,
}

impl fmt::Debug for HealthCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthCheck")
            .field("name", &self.name)
            .field("state", &self.state)
            .field("message", &self.message.as_deref().map(|_| "<redacted>"))
            .finish()
    }
}

impl HealthCheck {
    pub fn new(
        name: impl Into<String>,
        state: HealthState,
        message: Option<impl Into<String>>,
    ) -> Self {
        Self {
            name: name.into(),
            state,
            message: message.map(Into::into),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub live: bool,
    pub startup_complete: bool,
    pub draining: bool,
    pub active_policy_revision: Option<PolicyRevisionId>,
    pub checks: Vec<HealthCheck>,
}

impl fmt::Debug for HealthSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthSnapshot")
            .field("live", &self.live)
            .field("startup_complete", &self.startup_complete)
            .field("draining", &self.draining)
            .field(
                "active_policy_revision",
                &self.active_policy_revision.as_ref().map(|_| "<redacted>"),
            )
            .field("check_count", &self.checks.len())
            .finish()
    }
}

impl HealthSnapshot {
    pub fn new(
        live: bool,
        startup_complete: bool,
        draining: bool,
        active_policy_revision: Option<PolicyRevisionId>,
        checks: Vec<HealthCheck>,
    ) -> Self {
        Self {
            live,
            startup_complete,
            draining,
            active_policy_revision,
            checks,
        }
    }

    pub fn livez(&self) -> HealthState {
        if self.live {
            HealthState::Passing
        } else {
            HealthState::Failing
        }
    }

    pub fn startupz(&self) -> HealthState {
        if self.live && self.startup_complete {
            HealthState::Passing
        } else {
            HealthState::Failing
        }
    }

    pub fn readyz(&self) -> HealthState {
        if !self.live
            || !self.startup_complete
            || self.draining
            || self.active_policy_revision.is_none()
        {
            return HealthState::Failing;
        }
        if self
            .checks
            .iter()
            .any(|check| check.state == HealthState::Failing)
        {
            HealthState::Failing
        } else if self
            .checks
            .iter()
            .any(|check| check.state == HealthState::Degraded)
        {
            HealthState::Degraded
        } else {
            HealthState::Passing
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthProbeResponsePlan {
    pub probe: HealthProbeKind,
    pub state: HealthState,
    pub ready: bool,
    pub active_policy_revision: Option<PolicyRevisionId>,
    pub code: &'static str,
    pub message: &'static str,
}

impl fmt::Debug for HealthProbeResponsePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HealthProbeResponsePlan")
            .field("probe", &self.probe)
            .field("state", &self.state)
            .field("ready", &self.ready)
            .field(
                "active_policy_revision",
                &self.active_policy_revision.map(|_| "<redacted>"),
            )
            .field("code", &self.code)
            .field("message", &self.message)
            .finish()
    }
}

pub fn plan_health_probe_response(
    probe: HealthProbeKind,
    snapshot: &HealthSnapshot,
) -> HealthProbeResponsePlan {
    let state = match probe {
        HealthProbeKind::Livez => snapshot.livez(),
        HealthProbeKind::Readyz => snapshot.readyz(),
        HealthProbeKind::Startupz => snapshot.startupz(),
    };
    let ready = matches!(state, HealthState::Passing | HealthState::Degraded);
    let (code, message) = match (probe, state) {
        (HealthProbeKind::Livez, HealthState::Passing) => {
            ("health_livez_passing", "process is live")
        }
        (HealthProbeKind::Livez, _) => ("health_livez_failing", "process is not live"),
        (HealthProbeKind::Readyz, HealthState::Passing) => {
            ("health_readyz_passing", "service is ready")
        }
        (HealthProbeKind::Readyz, HealthState::Degraded) => (
            "health_readyz_degraded",
            "service is ready with degraded dependencies",
        ),
        (HealthProbeKind::Readyz, HealthState::Failing) => {
            ("health_readyz_failing", "service is not ready")
        }
        (HealthProbeKind::Startupz, HealthState::Passing) => {
            ("health_startupz_passing", "startup is complete")
        }
        (HealthProbeKind::Startupz, _) => ("health_startupz_failing", "startup is not complete"),
    };
    HealthProbeResponsePlan {
        probe,
        state,
        ready,
        active_policy_revision: snapshot.active_policy_revision,
        code,
        message,
    }
}
