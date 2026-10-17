//! Compiled Kiro ACP activity ledger truncation and label inheritance.
//! Rust owns only BTreeMap/Vec storage and renders the Mojo-chosen item.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_kiro_acp_tool_activity_ledger_v1(
        abi_version: i64,
        id_address: u64,
        id_length: i64,
        already_truncated: i64,
        activity_count: u64,
        label_count: u64,
        previous_present: i64,
        title_present: i64,
        kind_present: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KiroAcpToolActivityFacts<'a> {
    pub tool_call_id: &'a str,
    pub already_truncated: bool,
    pub activity_count: usize,
    pub label_count: usize,
    pub previous_present: bool,
    pub title_present: bool,
    pub kind_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KiroAcpToolActivityAction {
    Ignore,
    Append,
    AppendTruncated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KiroAcpToolActivityPlan {
    pub action: KiroAcpToolActivityAction,
    pub record_label: bool,
    pub use_previous_title: bool,
    pub use_previous_kind: bool,
}

/// The compiled owner enforces the 128-event cap, one terminal truncation
/// marker, bounded IDs and previous-label inheritance. There is no fallback.
pub fn kiro_acp_tool_activity_plan(
    facts: KiroAcpToolActivityFacts<'_>,
) -> Result<KiroAcpToolActivityPlan, MojoError> {
    let mut output = [-1_i64; 4];
    let status = unsafe {
        prodex_kiro_acp_tool_activity_ledger_v1(
            1,
            facts.tool_call_id.as_ptr() as usize as u64,
            i64::try_from(facts.tool_call_id.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(facts.already_truncated),
            u64::try_from(facts.activity_count).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(facts.label_count).map_err(|_| MojoError::InvalidInput)?,
            i64::from(facts.previous_present),
            i64::from(facts.title_present),
            i64::from(facts.kind_present),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let action = match output[0] {
        0 => KiroAcpToolActivityAction::Ignore,
        1 => KiroAcpToolActivityAction::Append,
        2 => KiroAcpToolActivityAction::AppendTruncated,
        _ => return Err(MojoError::InvalidOutput),
    };
    let decode = |flag| match flag {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    };
    let plan = KiroAcpToolActivityPlan {
        action,
        record_label: decode(output[1])?,
        use_previous_title: decode(output[2])?,
        use_previous_kind: decode(output[3])?,
    };
    if (plan.action != KiroAcpToolActivityAction::Append
        && (plan.record_label || plan.use_previous_title || plan.use_previous_kind))
        || ((plan.use_previous_title || plan.use_previous_kind) && !facts.previous_present)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> KiroAcpToolActivityFacts<'static> {
        KiroAcpToolActivityFacts {
            tool_call_id: "call-1",
            already_truncated: false,
            activity_count: 0,
            label_count: 0,
            previous_present: false,
            title_present: true,
            kind_present: true,
        }
    }

    #[test]
    fn mojo_acp_tool_activity_ledger_caps_events_and_preserves_previous_labels() {
        let plan = kiro_acp_tool_activity_plan(facts()).unwrap();
        assert_eq!(
            plan,
            KiroAcpToolActivityPlan {
                action: KiroAcpToolActivityAction::Append,
                record_label: true,
                use_previous_title: false,
                use_previous_kind: false,
            }
        );

        let plan = kiro_acp_tool_activity_plan(KiroAcpToolActivityFacts {
            previous_present: true,
            title_present: false,
            kind_present: false,
            label_count: 128,
            activity_count: 126,
            ..facts()
        })
        .unwrap();
        assert_eq!(plan.action, KiroAcpToolActivityAction::Append);
        assert!(plan.record_label);
        assert!(plan.use_previous_title);
        assert!(plan.use_previous_kind);

        let plan = kiro_acp_tool_activity_plan(KiroAcpToolActivityFacts {
            label_count: 128,
            activity_count: 126,
            ..facts()
        })
        .unwrap();
        assert!(!plan.record_label);

        for count in [127, 128, 10_000] {
            let plan = kiro_acp_tool_activity_plan(KiroAcpToolActivityFacts {
                activity_count: count,
                previous_present: true,
                title_present: false,
                kind_present: false,
                ..facts()
            })
            .unwrap();
            assert_eq!(plan.action, KiroAcpToolActivityAction::AppendTruncated);
            assert!(!plan.record_label);
            assert!(!plan.use_previous_title);
            assert!(!plan.use_previous_kind);
        }
        let plan = kiro_acp_tool_activity_plan(KiroAcpToolActivityFacts {
            already_truncated: true,
            ..facts()
        })
        .unwrap();
        assert_eq!(plan.action, KiroAcpToolActivityAction::Ignore);
    }

    #[test]
    fn mojo_acp_tool_activity_ledger_bounds_identifier_bytes_not_scalars() {
        for (id, expected) in [
            ("x".repeat(256), true),
            ("x".repeat(257), false),
            ("東".repeat(85), true),
            ("東".repeat(86), false),
            ("".to_string(), true),
        ] {
            let plan = kiro_acp_tool_activity_plan(KiroAcpToolActivityFacts {
                tool_call_id: &id,
                previous_present: true,
                title_present: false,
                kind_present: false,
                ..facts()
            })
            .unwrap();
            assert_eq!(plan.record_label, expected, "id bytes={}", id.len());
            assert_eq!(plan.use_previous_title, expected);
            assert_eq!(plan.use_previous_kind, expected);
        }
    }

    #[test]
    fn mojo_acp_tool_activity_ledger_abi_rejects_invalid_values_before_output() {
        let id = b"call-1";
        let addr = id.as_ptr() as usize as u64;
        let mut output = [71_i64; 4];
        let out = output.as_mut_ptr() as usize as u64;
        let call = |version, id_addr, id_len, truncated, previous, title, kind, dst, capacity| unsafe {
            prodex_kiro_acp_tool_activity_ledger_v1(
                version, id_addr, id_len, truncated, 0, 0, previous, title, kind, dst, capacity,
            )
        };
        assert_eq!(call(2, addr, 6, 0, 0, 1, 1, out, 4), 4);
        assert_eq!(call(1, 0, 6, 0, 0, 1, 1, out, 4), 1);
        assert_eq!(call(1, addr, -1, 0, 0, 1, 1, out, 4), 1);
        assert_eq!(call(1, addr, 6, 2, 0, 1, 1, out, 4), 1);
        assert_eq!(call(1, addr, 6, 0, -1, 1, 1, out, 4), 1);
        assert_eq!(call(1, addr, 6, 0, 0, 2, 1, out, 4), 1);
        assert_eq!(call(1, addr, 6, 0, 0, 1, 2, out, 4), 1);
        assert_eq!(call(1, addr, 6, 0, 0, 1, 1, 0, 4), 1);
        assert_eq!(call(1, addr, 6, 0, 0, 1, 1, out, 3), 1);
        let invalid = [0xff_u8];
        assert_eq!(
            call(1, invalid.as_ptr() as usize as u64, 1, 0, 0, 1, 1, out, 4),
            1
        );
        assert_eq!(output, [71; 4]);
    }
}
