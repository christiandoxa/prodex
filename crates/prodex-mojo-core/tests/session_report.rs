use prodex_mojo_core::json::session_report_update_json;

#[test]
fn session_meta_repair_plan_preserves_structural_and_repair_precedence() {
    let structural = r#"{"timestamp":"","type":"\u0073ession_meta","payload":{"id":"","timestamp":"","cwd":"","originator":"","cli_version":""}}"#;
    let plan = session_report_update_json(structural).unwrap();
    assert!(plan.starts_rollout_metadata);
    assert_eq!(plan.repair_timestamp, None);
    assert_eq!(plan.repair_cwd, None);
    assert_eq!(plan.repair_model_provider, None);

    let repair = r#"{"timestamp":" root ","cwd":"root-cwd","model_provider":"root-provider","payload":{"timestamp":"payload-ts","cwd":" payload-cwd ","model_provider":" payload-provider "}}"#;
    let plan = session_report_update_json(repair).unwrap();
    let token = |span: Option<(usize, usize)>| span.map(|(start, end)| &repair[start..end]);
    assert_eq!(token(plan.repair_timestamp), Some(r#"" root ""#));
    assert_eq!(token(plan.repair_cwd), Some(r#"" payload-cwd ""#));
    assert_eq!(
        token(plan.repair_model_provider),
        Some(r#"" payload-provider ""#)
    );
}
