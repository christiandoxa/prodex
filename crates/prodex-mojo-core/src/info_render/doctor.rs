use super::{MojoError, render};

const DOCTOR_VALUE_COLOR: i64 = 28;
const DOCTOR_VIEWPORT: i64 = 29;

/// The deterministic short-terminal policy for one already-built doctor body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoDoctorViewportPlan {
    pub visible_rows: usize,
    pub hidden_rows: usize,
    pub critical_index: Option<usize>,
}

/// Classify a doctor value without coupling the decision to Ratatui colors.
pub fn doctor_value_color(label: &str, value: &str) -> Result<u8, MojoError> {
    let output = render(DOCTOR_VALUE_COLOR, &[], &[], &[label, value], 0)?;
    let color = output.parse::<u8>().map_err(|_| MojoError::InvalidOutput)?;
    (color <= 3)
        .then_some(color)
        .ok_or(MojoError::InvalidOutput)
}

/// Select visible rows and preserve the first hidden critical diagnostic.
pub fn doctor_viewport_plan(
    max_rows: usize,
    critical_lines: &[bool],
) -> Result<InfoDoctorViewportPlan, MojoError> {
    let mut unsigned = Vec::with_capacity(2 + critical_lines.len());
    unsigned.push(u64::try_from(max_rows).map_err(|_| MojoError::InvalidInput)?);
    unsigned.push(u64::try_from(critical_lines.len()).map_err(|_| MojoError::InvalidInput)?);
    unsigned.extend(critical_lines.iter().map(|critical| u64::from(*critical)));
    let output = render(DOCTOR_VIEWPORT, &[], &unsigned, &[], 0)?;
    let values = output
        .split(',')
        .map(|value| value.parse::<i64>().map_err(|_| MojoError::InvalidOutput))
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != 3 || values[..2].iter().any(|value| *value < 0) || values[2] < -1 {
        return Err(MojoError::InvalidOutput);
    }
    let visible_rows = usize::try_from(values[0]).map_err(|_| MojoError::InvalidOutput)?;
    let hidden_rows = usize::try_from(values[1]).map_err(|_| MojoError::InvalidOutput)?;
    let critical_index = match values[2] {
        -1 => None,
        value => Some(usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?),
    };
    if visible_rows > max_rows
        || visible_rows > critical_lines.len()
        || hidden_rows != critical_lines.len().saturating_sub(visible_rows)
        || (hidden_rows == 0 && critical_index.is_some())
        || critical_index.is_some_and(|index| {
            index < visible_rows || index >= critical_lines.len() || !critical_lines[index]
        })
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(InfoDoctorViewportPlan {
        visible_rows,
        hidden_rows,
        critical_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_color_policy_keeps_precedence_and_ascii_matching() {
        assert_eq!(doctor_value_color("Quota", "blocked: warning").unwrap(), 1);
        assert_eq!(doctor_value_color("Runtime", "READY").unwrap(), 2);
        assert_eq!(doctor_value_color("Quota", "unknown 日本").unwrap(), 3);
        assert_eq!(doctor_value_color("Other", "unknown 日本").unwrap(), 0);
    }

    #[test]
    fn doctor_viewport_policy_preserves_hidden_critical_rows() {
        let plan = doctor_viewport_plan(3, &[false, false, true, false, true]).unwrap();
        assert_eq!(plan.visible_rows, 2);
        assert_eq!(plan.hidden_rows, 3);
        assert_eq!(plan.critical_index, Some(2));
        assert_eq!(
            doctor_viewport_plan(8, &[false, true]).unwrap(),
            InfoDoctorViewportPlan {
                visible_rows: 2,
                hidden_rows: 0,
                critical_index: None,
            }
        );
        assert_eq!(
            doctor_viewport_plan(0, &[true]).unwrap(),
            InfoDoctorViewportPlan {
                visible_rows: 0,
                hidden_rows: 1,
                critical_index: None,
            }
        );
        assert_eq!(
            doctor_viewport_plan(usize::MAX, &[]).unwrap(),
            InfoDoctorViewportPlan {
                visible_rows: 0,
                hidden_rows: 0,
                critical_index: None,
            }
        );
    }
}
