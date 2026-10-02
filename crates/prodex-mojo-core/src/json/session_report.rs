use super::{
    JsonKind, JsonNode, SessionReportMetadataPlan, StringView, ffi_nodes, signed, status,
    validated_optional_node_index,
};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_session_report_update_v1(
        abi_version: i64,
        nodes_address: u64,
        nodes_count: i64,
        raw_address: u64,
        raw_length: i64,
        number_values_address: u64,
        number_valid_address: u64,
        output_address: u64,
    ) -> i64;

    fn prodex_session_report_sort_v1(
        keys_address: u64,
        keys_count: i64,
        output_address: u64,
    ) -> i64;

    fn prodex_session_report_timestamp_sort_key_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
}

#[repr(C)]
struct SessionReportOrderKeyFfi {
    updated_sort_key: i64,
    id: StringView,
    path: StringView,
}

const _: () = {
    assert!(std::mem::size_of::<SessionReportOrderKeyFfi>() == 40);
    assert!(std::mem::align_of::<SessionReportOrderKeyFfi>() == 8);
};

/// Metadata update and timestamp decisions returned for a session value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionReportUpdatePlan {
    /// Existing metadata fields selected by the session-report planner.
    pub metadata: SessionReportMetadataPlan,
    /// Resume ID node eligible to replace the report ID.
    pub update_resume_id: Option<usize>,
    /// Numeric timestamp selected by the report fallback precedence.
    pub numeric_timestamp: Option<i64>,
    /// Parsed key to apply when a timestamp update has a sortable value.
    pub updated_sort_key: Option<i64>,
}

/// Sort key input for deterministic session-report ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionReportOrderKey<'a> {
    pub updated_sort_key: i64,
    pub id: &'a str,
    pub path: &'a str,
}

/// Plan session-report DTO updates from a Serde-built JSON tree.
///
/// `number_values` is aligned with `nodes`; `Some` contains Serde's `as_i64`
/// result for number nodes and `None` represents all other values.
pub fn session_report_update_plan(
    nodes: &[JsonNode<'_>],
    number_values: &[Option<i64>],
) -> Result<SessionReportUpdatePlan, MojoError> {
    if number_values.len() != nodes.len()
        || number_values
            .iter()
            .zip(nodes)
            .any(|(number, node)| number.is_some() && !matches!(node.kind, JsonKind::Number))
    {
        return Err(MojoError::InvalidInput);
    }

    let input = ffi_nodes(nodes, "")?;
    let values = number_values
        .iter()
        .map(|value| value.unwrap_or_default())
        .collect::<Vec<_>>();
    let valid = number_values
        .iter()
        .map(|value| i64::from(value.is_some()))
        .collect::<Vec<_>>();
    let mut output = [-1_i64; 13];
    status(unsafe {
        prodex_session_report_update_v1(
            1,
            input.as_ptr() as u64,
            signed(input.len())?,
            0,
            0,
            values.as_ptr() as u64,
            valid.as_ptr() as u64,
            output.as_mut_ptr() as u64,
        )
    })?;

    if !(0..=3).contains(&output[0]) {
        return Err(MojoError::InvalidOutput);
    }
    let metadata = SessionReportMetadataPlan {
        type_class: output[0],
        resume_id: validated_optional_node_index(output[1], nodes.len())?,
        model: validated_optional_node_index(output[2], nodes.len())?,
        effort: validated_optional_node_index(output[3], nodes.len())?,
        thread_name: validated_optional_node_index(output[4], nodes.len())?,
        cwd: validated_optional_node_index(output[5], nodes.len())?,
        updated_at: validated_optional_node_index(output[6], nodes.len())?,
        parent_thread_id: validated_optional_node_index(output[7], nodes.len())?,
        model_provider: validated_optional_node_index(output[8], nodes.len())?,
    };
    let update_resume_id = validated_optional_node_index(output[9], nodes.len())?;
    if update_resume_id.is_some() && update_resume_id != metadata.resume_id {
        return Err(MojoError::InvalidOutput);
    }
    let numeric_timestamp_index = validated_optional_node_index(output[10], nodes.len())?;
    let numeric_timestamp = numeric_timestamp_index
        .map(|index| number_values[index].ok_or(MojoError::InvalidOutput))
        .transpose()?;
    let updated_sort_key = match output[11] {
        0 if output[12] == 0 => None,
        1 => Some(output[12]),
        _ => return Err(MojoError::InvalidOutput),
    };

    Ok(SessionReportUpdatePlan {
        metadata,
        update_resume_id,
        numeric_timestamp,
        updated_sort_key,
    })
}

/// Parse the sortable key used for a session-report timestamp label.
pub fn session_report_timestamp_sort_key(value: &str) -> Result<Option<i64>, MojoError> {
    let mut output = [0_i64; 2];
    status(unsafe {
        prodex_session_report_timestamp_sort_key_v1(
            1,
            value.as_ptr() as u64,
            signed(value.len())?,
            output.as_mut_ptr() as u64,
        )
    })?;
    match output[0] {
        0 if output[1] == 0 => Ok(None),
        1 => Ok(Some(output[1])),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Return indices in the report order: newest first, then ID and path.
pub fn session_report_order(keys: &[SessionReportOrderKey<'_>]) -> Result<Vec<usize>, MojoError> {
    if keys.len() > i64::MAX as usize / std::mem::size_of::<SessionReportOrderKeyFfi>() {
        return Err(MojoError::InvalidInput);
    }
    let input = keys
        .iter()
        .map(|key| {
            signed(key.id.len())?;
            signed(key.path.len())?;
            Ok(SessionReportOrderKeyFfi {
                updated_sort_key: key.updated_sort_key,
                id: key.id.into(),
                path: key.path.into(),
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(keys.len())
        .map_err(|_| MojoError::Capacity)?;
    output.resize(keys.len(), -1_i64);
    status(unsafe {
        prodex_session_report_sort_v1(
            input.as_ptr() as u64,
            signed(input.len())?,
            output.as_mut_ptr() as u64,
        )
    })?;

    let mut seen = vec![false; keys.len()];
    for index in &output {
        let index = usize::try_from(*index).map_err(|_| MojoError::InvalidOutput)?;
        let present = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
        if *present {
            return Err(MojoError::InvalidOutput);
        }
        *present = true;
    }
    Ok(output.into_iter().map(|index| index as usize).collect())
}

#[cfg(all(test, prodex_mojo_active))]
mod tests {
    use super::*;

    fn node(
        kind: JsonKind,
        key: &'static str,
        text: &'static str,
        parent: Option<usize>,
        first_child: Option<usize>,
        next_sibling: Option<usize>,
    ) -> JsonNode<'static> {
        JsonNode {
            kind,
            first_child,
            next_sibling,
            parent,
            key,
            text,
            raw_start: 0,
            raw_length: 0,
        }
    }

    fn direct_update_output(nodes: &[JsonNode<'_>], number_values: &[Option<i64>]) -> [i64; 13] {
        let input = ffi_nodes(nodes, "").unwrap();
        let values = number_values
            .iter()
            .map(|value| value.unwrap_or_default())
            .collect::<Vec<_>>();
        let valid = number_values
            .iter()
            .map(|value| i64::from(value.is_some()))
            .collect::<Vec<_>>();
        let mut output = [-1_i64; 13];
        status(unsafe {
            prodex_session_report_update_v1(
                1,
                input.as_ptr() as u64,
                signed(input.len()).unwrap(),
                0,
                0,
                values.as_ptr() as u64,
                valid.as_ptr() as u64,
                output.as_mut_ptr() as u64,
            )
        })
        .unwrap();
        output
    }

    #[test]
    fn update_plan_matches_real_mojo_export_for_id_and_numeric_timestamp() {
        let nodes = [
            node(JsonKind::Object, "", "", None, Some(1), None),
            node(
                JsonKind::String,
                "type",
                "session_meta",
                Some(0),
                None,
                Some(2),
            ),
            node(JsonKind::Number, "updated_at", "", Some(0), None, Some(3)),
            node(JsonKind::Number, "ts", "", Some(0), None, Some(4)),
            node(JsonKind::Object, "payload", "", Some(0), Some(5), None),
            node(JsonKind::String, "id", "session-one", Some(4), None, None),
        ];
        let number_values = [None, None, Some(123), Some(456), None, None];
        let plan = session_report_update_plan(&nodes, &number_values).unwrap();
        let raw = direct_update_output(&nodes, &number_values);

        assert_eq!(raw, [1, 5, -1, -1, -1, -1, -1, -1, -1, 5, 2, 1, 123]);
        assert_eq!(
            plan,
            SessionReportUpdatePlan {
                metadata: SessionReportMetadataPlan {
                    type_class: 1,
                    resume_id: Some(5),
                    model: None,
                    effort: None,
                    thread_name: None,
                    cwd: None,
                    updated_at: None,
                    parent_thread_id: None,
                    model_provider: None,
                },
                update_resume_id: Some(5),
                numeric_timestamp: Some(123),
                updated_sort_key: Some(123),
            }
        );
    }

    #[test]
    fn timestamp_plan_matches_real_mojo_export_for_rfc3339_offset() {
        let nodes = [
            node(JsonKind::Object, "", "", None, Some(1), None),
            node(
                JsonKind::String,
                "updated_at",
                "1970-01-01T01:00:00+01:00",
                Some(0),
                None,
                None,
            ),
        ];
        let number_values = [None, None];
        let plan = session_report_update_plan(&nodes, &number_values).unwrap();
        let raw = direct_update_output(&nodes, &number_values);

        assert_eq!(raw, [0, -1, -1, -1, -1, -1, 1, -1, -1, -1, -1, 1, 0]);
        assert_eq!(plan.metadata.updated_at, Some(1));
        assert_eq!(plan.updated_sort_key, Some(0));
        assert_eq!(
            session_report_timestamp_sort_key("-9223372036854775808"),
            Ok(Some(i64::MIN))
        );
        assert_eq!(
            session_report_timestamp_sort_key("1969-12-31T23:59:59Z"),
            Ok(Some(-1))
        );
        assert_eq!(
            session_report_timestamp_sort_key("2000-02-29T00:00:00Z"),
            Ok(Some(951_782_400))
        );
        assert_eq!(session_report_timestamp_sort_key("+17"), Ok(Some(17)));
        assert_eq!(session_report_timestamp_sort_key(" 17 "), Ok(None));
        assert_eq!(
            session_report_timestamp_sort_key("1970-02-29T00:00:00Z"),
            Ok(None)
        );
    }

    #[test]
    fn report_order_matches_real_mojo_export_and_keeps_equal_key_input_order() {
        let keys = [
            SessionReportOrderKey {
                updated_sort_key: 10,
                id: "b",
                path: "same",
            },
            SessionReportOrderKey {
                updated_sort_key: 10,
                id: "a",
                path: "path-b",
            },
            SessionReportOrderKey {
                updated_sort_key: 10,
                id: "a",
                path: "path-a",
            },
            SessionReportOrderKey {
                updated_sort_key: 10,
                id: "a",
                path: "path-a",
            },
            SessionReportOrderKey {
                updated_sort_key: 11,
                id: "z",
                path: "newest",
            },
        ];
        let expected = [4, 2, 3, 1, 0];
        let adapted = session_report_order(&keys).unwrap();
        let input = keys
            .iter()
            .map(|key| SessionReportOrderKeyFfi {
                updated_sort_key: key.updated_sort_key,
                id: key.id.into(),
                path: key.path.into(),
            })
            .collect::<Vec<_>>();
        let mut raw = [-1_i64; 5];
        status(unsafe {
            prodex_session_report_sort_v1(
                input.as_ptr() as u64,
                signed(input.len()).unwrap(),
                raw.as_mut_ptr() as u64,
            )
        })
        .unwrap();

        assert_eq!(adapted, expected);
        assert_eq!(raw, [4, 2, 3, 1, 0]);
    }
}
