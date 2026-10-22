use super::{DispatchContext, McpRequestHeaders, parse_dispatch_request};
use crate::super_expose::{
    logging::ExposeAuditLog, optional_tools::ExposeOptionalTools, run::RunManager,
    session_prompt_write::SessionPromptWriteService,
};
use crate::test_support::{TestEnvVarGuard, test_temp_root};
use prodex_cli::SuperExposeMode;
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TestRoot(PathBuf);

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn with_context(run: impl FnOnce(&DispatchContext<'_>, &Path)) {
    let root = TestRoot(test_temp_root().join(format!("prodex-dispatch-reply-{}-{}",
        std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())));
    std::fs::create_dir_all(&root.0).unwrap();
    let logs = root.0.join("logs");
    let _lock = TestEnvVarGuard::lock();
    let _home = TestEnvVarGuard::set_home(&root.0);
    let _environment = [
        ("PRODEX_HOME", root.0.join("prodex-home")),
        ("CODEX_HOME", root.0.join("codex-home")),
        ("CLAUDE_CONFIG_DIR", root.0.join("claude-home")),
        ("PRODEX_RUNTIME_LOG_DIR", logs.clone()),
    ]
    .map(|(key, path)| TestEnvVarGuard::set(key, path.to_str().unwrap()));
    let _format = TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_FORMAT", "text");
    let audit = ExposeAuditLog::new().unwrap();
    let prodex_cli::Commands::Super(args) =
        prodex_cli::parse_cli_command_from(["prodex", "s", "--no-presidio"]).unwrap()
    else {
        panic!("expected Super arguments")
    };
    let manager = RunManager::new(
        root.0.clone(),
        *args,
        audit.clone(),
        "fixture-instance".into(),
    );
    let session = SessionPromptWriteService::default();
    let optional_tools = ExposeOptionalTools::empty_for_tests();
    let context = DispatchContext {
        manager: &manager,
        session_prompt_write: &session,
        instance_id: "fixture-instance",
        display_name: "fixture",
        workspace: &root.0,
        mode: SuperExposeMode::Full,
        optional_tools: &optional_tools,
        audit: &audit,
    };
    run(&context, &logs);
    audit.flush().unwrap();
}

fn headers() -> McpRequestHeaders {
    McpRequestHeaders {
        protocol_version: Some("2025-11-25".into()),
        mcp_method: None,
        mcp_name: None,
    }
}

fn audited_requests(logs: &Path) -> usize {
    std::fs::read_dir(logs)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "log"))
        .map(|path| {
            std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .filter(|line| line.contains(" super_expose_rpc "))
                .count()
        })
        .sum()
}

#[test]
fn dispatch_replies_preserve_status_payload_id_and_audit_eligibility() {
    with_context(|context, logs| {
        let cases = [
            (
                json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
                202,
                Value::Null,
                false,
            ),
            (
                json!({"jsonrpc":"2.0","method":"notifications/cancelled"}),
                202,
                Value::Null,
                false,
            ),
            (
                json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{"name":"x".repeat(129)}}),
                202,
                Value::Null,
                false,
            ),
            (
                json!({"jsonrpc":"2.0","method":"ping"}),
                400,
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32601,"message":"notification is unsupported"}}),
                false,
            ),
            (
                json!({"jsonrpc":"2.0","id":false,"method":"initialize"}),
                400,
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"invalid request id"}}),
                false,
            ),
            (
                json!({"jsonrpc":"2.0","id":7,"method":"initialize"}),
                400,
                json!({"jsonrpc":"2.0","id":7,"error":{"code":-32602,"message":"initialize params are required"}}),
                true,
            ),
            (
                json!({"jsonrpc":"2.0","id":"","method":"initialize","params":{}}),
                400,
                json!({"jsonrpc":"2.0","id":"","error":{"code":-32602,"message":"protocolVersion is required"}}),
                true,
            ),
            (
                json!({"jsonrpc":"2.0","id":"fixture-id","method":"tools/call"}),
                400,
                json!({"jsonrpc":"2.0","id":"fixture-id","error":{"code":-32602,"message":"tool parameters are required"}}),
                true,
            ),
            (
                json!({"jsonrpc":"2.0","id":0,"method":"tools/call","params":{}}),
                400,
                json!({"jsonrpc":"2.0","id":0,"error":{"code":-32602,"message":"tool name is required"}}),
                true,
            ),
            (
                json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"prodex_super_status","arguments":[]}}),
                400,
                json!({"jsonrpc":"2.0","id":9,"error":{"code":-32602,"message":"tool arguments must be an object"}}),
                true,
            ),
        ];
        let mut expected_audits = 0;
        for (body, expected_status, expected_body, audited) in cases {
            let response = match parse_dispatch_request(
                &serde_json::to_vec(&body).unwrap(),
                &headers(),
                context,
            ) {
                Err(response) => response,
                Ok(_) => panic!("expected an immediate reply"),
            };
            assert_eq!(response.status_code().0, expected_status);
            let mut bytes = Vec::new();
            response.into_reader().read_to_end(&mut bytes).unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes).unwrap(),
                expected_body
            );
            context.audit.flush().unwrap();
            expected_audits += usize::from(audited);
            assert_eq!(audited_requests(logs), expected_audits);
        }
    });
}

#[test]
fn dispatch_preserves_header_rejection_priority_and_valid_request_audit() {
    with_context(|context, logs| {
        let mut mismatch = headers();
        mismatch.mcp_method = Some("tools/list".into());
        let response = match parse_dispatch_request(
            br#"{"jsonrpc":"2.0","method":"ping"}"#,
            &mismatch,
            context,
        ) {
            Err(response) => response,
            Ok(_) => panic!("header mismatch must reject"),
        };
        assert_eq!(response.status_code().0, 400);
        let mut bytes = Vec::new();
        response.into_reader().read_to_end(&mut bytes).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap(),
            json!({
                "jsonrpc":"2.0","id":null,"error":{"code":-32020,"message":"Mcp-Method header mismatch"}
            })
        );
        context.audit.flush().unwrap();
        assert_eq!(audited_requests(logs), 0);
        let request = match parse_dispatch_request(
            br#"{"jsonrpc":"2.0","id":4,"method":"ping"}"#,
            &headers(),
            context,
        ) {
            Ok(request) => request,
            Err(_) => panic!("valid ping should continue to dispatch"),
        };
        assert_eq!(request.id, Some(json!(4)));
        context.audit.flush().unwrap();
        assert_eq!(audited_requests(logs), 1);
    });
}
