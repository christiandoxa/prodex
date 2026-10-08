use super::{SessionReportMetadataPlan, StringView, signed, status, validated_optional_raw_span};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_session_report_update_json_v2(
        abi_version: i64,
        raw_address: u64,
        raw_length: i64,
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
    fn prodex_session_report_record_shape_v1(
        abi_version: i64,
        raw_address: u64,
        raw_length: i64,
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

/// Metadata and numeric-timestamp decisions returned for one raw session JSON value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionReportUpdatePlan {
    pub metadata: SessionReportMetadataPlan,
    pub update_resume_id: Option<(usize, usize)>,
    pub numeric_timestamp: Option<i64>,
    pub starts_rollout_metadata: bool,
    pub repair_timestamp: Option<(usize, usize)>,
    pub repair_cwd: Option<(usize, usize)>,
    pub repair_model_provider: Option<(usize, usize)>,
}

/// Sort key input for deterministic session-report ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionReportOrderKey<'a> {
    pub updated_sort_key: i64,
    pub id: &'a str,
    pub path: &'a str,
}

pub fn session_report_update_json(raw: &str) -> Result<SessionReportUpdatePlan, MojoError> {
    let mut output = [-1_i64; 28];
    status(unsafe {
        prodex_session_report_update_json_v2(
            2,
            raw.as_ptr() as u64,
            signed(raw.len())?,
            output.as_mut_ptr() as u64,
        )
    })?;
    if !(0..=3).contains(&output[0]) {
        return Err(MojoError::InvalidOutput);
    }
    let span = |slot: usize| validated_optional_raw_span(output[slot], output[slot + 1], raw);
    let metadata = SessionReportMetadataPlan {
        type_class: output[0],
        resume_id: span(1)?,
        model: span(3)?,
        effort: span(5)?,
        thread_name: span(7)?,
        cwd: span(9)?,
        updated_at: span(11)?,
        parent_thread_id: span(13)?,
        model_provider: span(15)?,
    };
    let update_resume_id = span(17)?;
    if update_resume_id.is_some() && update_resume_id != metadata.resume_id {
        return Err(MojoError::InvalidOutput);
    }
    let numeric_timestamp = match output[19] {
        0 if output[20] == 0 => None,
        1 => Some(output[20]),
        _ => return Err(MojoError::InvalidOutput),
    };
    let starts_rollout_metadata = match output[21] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(SessionReportUpdatePlan {
        metadata,
        update_resume_id,
        numeric_timestamp,
        starts_rollout_metadata,
        repair_timestamp: span(22)?,
        repair_cwd: span(24)?,
        repair_model_provider: span(26)?,
    })
}

pub fn session_report_metadata_json(raw: &str) -> Result<SessionReportMetadataPlan, MojoError> {
    Ok(session_report_update_json(raw)?.metadata)
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

/// Validate the bounded transcript record shape used by session output reads.
pub fn session_report_record_shape(raw: &str) -> Result<bool, MojoError> {
    let mut output = [-1_i64; 1];
    status(unsafe {
        prodex_session_report_record_shape_v1(
            1,
            raw.as_ptr() as u64,
            signed(raw.len())?,
            output.as_mut_ptr() as u64,
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
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

    #[test]
    fn update_plan_uses_raw_json_precedence_for_id_and_numeric_timestamp() {
        let raw =
            r#"{"type":"session_meta","updated_at":123,"ts":456,"payload":{"id":"session-one"}}"#;
        let plan = session_report_update_json(raw).unwrap();
        assert_eq!(plan.metadata.type_class, 1);
        assert_eq!(plan.update_resume_id, plan.metadata.resume_id);
        assert_eq!(plan.numeric_timestamp, Some(123));
        let (start, end) = plan.metadata.resume_id.unwrap();
        assert_eq!(&raw[start..end], r#""session-one""#);
    }

    #[test]
    fn raw_metadata_preserves_string_precedence_and_unicode_trim_contract() {
        let raw = r#"{"type":"turn_context","model":" root ","payload":{"model":" payload ","effort":" high ","metadata":{"thread_name":" nested "}},"thread_name":" root-name ","timestamp":"1970-01-01T01:00:00+01:00","model_provider":"prodex-openai"}"#;
        let plan = session_report_update_json(raw).unwrap();
        let token = |span: Option<(usize, usize)>| span.map(|(start, end)| &raw[start..end]);
        assert_eq!(plan.metadata.type_class, 2);
        assert_eq!(token(plan.metadata.model), Some(r#"" payload ""#));
        assert_eq!(token(plan.metadata.effort), Some(r#"" high ""#));
        assert_eq!(token(plan.metadata.thread_name), Some(r#"" nested ""#));
        assert_eq!(
            token(plan.metadata.updated_at),
            Some(r#""1970-01-01T01:00:00+01:00""#)
        );
        assert_eq!(
            token(plan.metadata.model_provider),
            Some(r#""prodex-openai""#)
        );
        assert_eq!(plan.numeric_timestamp, None);
    }

    #[test]
    fn timestamp_parser_stays_mojo_owned_for_rfc3339_and_integer_labels() {
        assert_eq!(
            session_report_timestamp_sort_key("1970-01-01T01:00:00+01:00"),
            Ok(Some(0))
        );
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

    #[test]
    fn record_shape_policy_keeps_valid_boundaries_and_rejects_malformed_records() {
        assert!(
            session_report_record_shape(
                r#"{"type":"event_msg","payload":{"type":"user_message","message":"x"}}"#
            )
            .unwrap()
        );
        assert!(session_report_record_shape(r#"{"type":"future_record","payload":null}"#).unwrap());
        assert!(
            !session_report_record_shape(
                r#"{"type":"event_msg","payload":{"message":"missing type"}}"#
            )
            .unwrap()
        );
        assert!(!session_report_record_shape("not-json").unwrap());
    }
}
