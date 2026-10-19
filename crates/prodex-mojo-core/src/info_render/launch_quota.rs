//! Typed decoding of compiled preflight plans; message policy stays in Mojo.

use super::{InfoRuntimeLaunchSelectedProfileStatus, MojoError, render_bytes};

pub(super) const LAUNCH_NO_READY: i64 = 32;
pub(super) const LAUNCH_BLOCKED: i64 = 33;

/// Messages to apply when preflight found no ready profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLaunchNoReadyProfilesPlan {
    Blocked {
        blocked_message: String,
        no_ready_message: String,
        inspect_hint: String,
        error_message: String,
    },
    ProbeFailed {
        warning_message: String,
        continue_message: String,
    },
}

/// A pre-launch rotation or a stop with its ordered messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLaunchBlockedSelectedProfilePlan {
    Rotate {
        blocked_message: String,
        next_profile: String,
        rotate_message: String,
    },
    Stop {
        blocked_message: String,
        messages: Vec<String>,
        error_message: String,
    },
}

fn plan_fields(output: &[u8]) -> Result<(u8, Vec<String>), MojoError> {
    let kind = *output.first().ok_or(MojoError::InvalidOutput)?;
    let mut fields = Vec::new();
    let mut cursor = 1;
    while cursor < output.len() {
        if fields.len() == 5 {
            return Err(MojoError::InvalidOutput);
        }
        let length = super::status::read_status_length(output, &mut cursor, Some(20))?;
        let end = cursor.checked_add(length).ok_or(MojoError::InvalidOutput)?;
        fields.push(
            std::str::from_utf8(output.get(cursor..end).ok_or(MojoError::InvalidOutput)?)
                .map_err(|_| MojoError::InvalidOutput)?
                .to_string(),
        );
        cursor = end;
    }
    Ok((kind, fields))
}

pub fn no_ready_runtime_profiles_plan(
    profile: &str,
    status: InfoRuntimeLaunchSelectedProfileStatus<'_>,
) -> Result<RuntimeLaunchNoReadyProfilesPlan, MojoError> {
    let (failed, detail) = match status {
        InfoRuntimeLaunchSelectedProfileStatus::Blocked { blocked_summary } => (0, blocked_summary),
        InfoRuntimeLaunchSelectedProfileStatus::ProbeFailed { error } => (1, error),
        InfoRuntimeLaunchSelectedProfileStatus::Ready => return Err(MojoError::InvalidInput),
    };
    let (kind, fields) = plan_fields(&render_bytes(
        LAUNCH_NO_READY,
        &[],
        &[],
        &[profile, "", "", detail],
        failed,
    )?)?;
    match kind {
        b'0' => {
            let [
                blocked_message,
                no_ready_message,
                inspect_hint,
                error_message,
            ] = <[String; 4]>::try_from(fields).map_err(|_| MojoError::InvalidOutput)?;
            Ok(RuntimeLaunchNoReadyProfilesPlan::Blocked {
                blocked_message,
                no_ready_message,
                inspect_hint,
                error_message,
            })
        }
        b'1' => {
            let [warning_message, continue_message] =
                <[String; 2]>::try_from(fields).map_err(|_| MojoError::InvalidOutput)?;
            Ok(RuntimeLaunchNoReadyProfilesPlan::ProbeFailed {
                warning_message,
                continue_message,
            })
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn blocked_selected_runtime_profile_plan(
    profile: &str,
    blocked_summary: &str,
    allow_auto_rotate: bool,
    alternatives: &[String],
) -> Result<RuntimeLaunchBlockedSelectedProfilePlan, MojoError> {
    let mut texts = vec![profile, "", "", blocked_summary];
    texts.extend(alternatives.iter().map(String::as_str));
    let (kind, fields) = plan_fields(&render_bytes(
        LAUNCH_BLOCKED,
        &[],
        &[],
        &texts,
        u64::from(allow_auto_rotate),
    )?)?;
    match kind {
        b'0' => {
            let [blocked_message, next_profile, rotate_message] =
                <[String; 3]>::try_from(fields).map_err(|_| MojoError::InvalidOutput)?;
            Ok(RuntimeLaunchBlockedSelectedProfilePlan::Rotate {
                blocked_message,
                next_profile,
                rotate_message,
            })
        }
        b'1' if (3..=5).contains(&fields.len()) => {
            let mut fields = fields.into_iter();
            let blocked_message = fields.next().ok_or(MojoError::InvalidOutput)?;
            let error_message = fields.next().ok_or(MojoError::InvalidOutput)?;
            Ok(RuntimeLaunchBlockedSelectedProfilePlan::Stop {
                blocked_message,
                error_message,
                messages: fields.collect(),
            })
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "launch_quota_tests.rs"]
mod tests;
