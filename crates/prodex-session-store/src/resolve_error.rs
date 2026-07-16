use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub enum SessionResolveError {
    Missing {
        selector: String,
    },
    Ambiguous {
        selector: String,
        matches: Vec<String>,
    },
}

impl fmt::Debug for SessionResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { .. } => formatter
                .debug_struct("Missing")
                .field("selector", &"<redacted>")
                .finish(),
            Self::Ambiguous { matches, .. } => formatter
                .debug_struct("Ambiguous")
                .field("selector", &"<redacted>")
                .field("match_count", &matches.len())
                .finish(),
        }
    }
}

impl fmt::Display for SessionResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { selector } => {
                let _ = selector;
                write!(formatter, "session could not be resolved")
            }
            Self::Ambiguous { selector, matches } => {
                let _ = (selector, matches);
                write!(formatter, "session selector is ambiguous")
            }
        }
    }
}

impl std::error::Error for SessionResolveError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionResolveErrorStatus {
    NotFound,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionResolveErrorResponsePlan {
    pub status: SessionResolveErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_session_resolve_error_response(
    error: &SessionResolveError,
) -> SessionResolveErrorResponsePlan {
    match error {
        SessionResolveError::Missing { .. } => SessionResolveErrorResponsePlan {
            status: SessionResolveErrorStatus::NotFound,
            code: "session_not_found",
            message: "session could not be resolved",
        },
        SessionResolveError::Ambiguous { .. } => SessionResolveErrorResponsePlan {
            status: SessionResolveErrorStatus::Conflict,
            code: "session_selector_ambiguous",
            message: "session selector is ambiguous",
        },
    }
}
