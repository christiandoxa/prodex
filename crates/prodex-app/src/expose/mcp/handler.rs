use super::super::http::ExposeHttpRequest;
use super::super::run_manager::ExposeRunManager;
use super::super::session::{expose_digest_eq, expose_token_digest};
use super::super::session_prompt_write::{
    PromptOutputReadRequest, SESSION_PROMPT_WRITE_MAX_MESSAGE_BYTES, SessionPromptWriteRequest,
    SessionPromptWriteService,
};
use super::super::ui::{ExposeHttpResponse, expose_mcp_empty_response, expose_text_response};
use super::protocol::{
    jsonrpc_result, mcp_accept_allowed, mcp_capability_segment, mcp_content_type_allowed,
    mcp_error_response, mcp_json_error, mcp_json_nesting_within_limit, mcp_json_response,
    mcp_origin_allowed, request_id, validate_configured_main_effort, validate_mcp_request_headers,
};
use super::tools::{
    apply_provider_override, event_page_limit, main_provider, mcp_tools, optional_process_id,
    optional_string, required_run_id, required_string, run_summary_json, tool_result,
    validate_tool_arguments, value_u64, value_usize,
};
use super::{
    ExposeMcpEndpoint, ExposeMcpEndpointInit, MCP_CURRENT_PROTOCOL_VERSION,
    MCP_ERROR_HEADER_MISMATCH, MCP_ERROR_UNSUPPORTED_VERSION, MCP_MAX_CURSOR_BYTES,
    MCP_MAX_JSON_NESTING, MCP_MAX_MODEL_BYTES, MCP_MAX_OUTPUT_EVENTS, MCP_MAX_OUTPUT_WAIT_MS,
    MCP_MAX_PROFILE_BYTES, MCP_MAX_TASK_BYTES, MCP_PROTOCOL_VERSIONS, MCP_RATE_LIMIT,
    MCP_RATE_WINDOW, McpRateLimit,
};
use prodex_cli::SuperArgs;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn mcp_binding_key(request: &ExposeHttpRequest) -> String {
    let session = request.header("Mcp-Session-Id").unwrap_or("stateless");
    expose_token_digest(session)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl ExposeMcpEndpoint {
    pub(crate) fn new(
        capability: &str,
        instance_id: String,
        workspace_root: std::path::PathBuf,
        workspace_name: String,
        display_name: String,
        defaults: SuperArgs,
    ) -> Arc<Self> {
        let writer_workspace_root = workspace_root.clone();
        let run_manager =
            ExposeRunManager::new(workspace_root, instance_id.clone(), workspace_name.clone());
        Self::from_run_manager(ExposeMcpEndpointInit {
            capability: capability.to_string(),
            instance_id,
            workspace_name,
            display_name,
            defaults,
            run_manager,
            workspace_root: writer_workspace_root,
            session_prompt_write: Arc::new(SessionPromptWriteService::default()),
        })
    }

    #[cfg(test)]
    pub(crate) fn new_with_run_manager(
        capability: &str,
        instance_id: String,
        workspace_name: String,
        display_name: String,
        defaults: SuperArgs,
        run_manager: ExposeRunManager,
    ) -> Arc<Self> {
        Self::from_run_manager(ExposeMcpEndpointInit {
            capability: capability.to_string(),
            instance_id,
            workspace_name,
            display_name,
            defaults,
            run_manager,
            workspace_root: std::env::current_dir().unwrap_or_default(),
            session_prompt_write: Arc::new(SessionPromptWriteService::default()),
        })
    }

    #[cfg(test)]
    pub(crate) fn new_with_run_manager_and_writer(init: ExposeMcpEndpointInit) -> Arc<Self> {
        Self::from_run_manager(init)
    }

    fn from_run_manager(init: ExposeMcpEndpointInit) -> Arc<Self> {
        Arc::new(Self {
            capability_digest: expose_token_digest(&init.capability),
            openai_relay: Mutex::new(None),
            run_manager: init.run_manager,
            server_name: format!("Prodex Super — {}", init.display_name),
            workspace_name: init.workspace_name,
            instance_id: init.instance_id,
            defaults: init.defaults,
            workspace_root: init.workspace_root,
            session_prompt_write: init.session_prompt_write,
            rate: Mutex::new(McpRateLimit {
                started: Instant::now(),
                requests: 0,
            }),
        })
    }

    pub(super) fn matches_target(&self, target: &str) -> bool {
        let Some(capability) = mcp_capability_segment(target) else {
            return false;
        };
        expose_digest_eq(&self.capability_digest, &expose_token_digest(capability))
    }

    pub(in crate::expose) fn install_openai_relay(
        &self,
        mcp_url: &super::PublicMcpEndpoint,
    ) -> anyhow::Result<String> {
        let relay = super::OpenAiMcpRelay::new(mcp_url)?;
        let endpoint = relay.endpoint.clone();
        *self
            .openai_relay
            .lock()
            .map_err(|_| anyhow::anyhow!("OpenAI MCP relay state unavailable"))? = Some(relay);
        Ok(endpoint)
    }

    pub(in crate::expose) fn openai_relay_target(&self, request_target: &str) -> Option<String> {
        self.openai_relay
            .lock()
            .ok()?
            .as_ref()
            .filter(|relay| relay.request_target == request_target)
            .map(|relay| relay.mcp_target.clone())
    }

    pub(in crate::expose) fn clear_openai_relay(&self) {
        let _ = self.openai_relay.lock().map(|mut relay| *relay = None);
    }

    pub(super) fn handle(&self, request: ExposeHttpRequest, host: &str) {
        if !self.matches_target(request.target()) {
            let _ = request.respond(expose_text_response(404, "not found"));
            return;
        }
        if (request.has_header("Origin") && request.header("Origin").is_none())
            || !mcp_origin_allowed(host, request.header("Origin"))
        {
            let _ = request.respond(mcp_error_response(403, None, -32003, "origin rejected"));
            return;
        }
        if request.method() != "POST" {
            let _ = request.respond(mcp_error_response(405, None, -32600, "method not allowed"));
            return;
        }
        if !self.admit_request() {
            let _ = request.respond(mcp_error_response(
                429,
                None,
                -32029,
                "request rate limit exceeded",
            ));
            return;
        }
        if !mcp_content_type_allowed(request.header("Content-Type")) {
            let _ = request.respond(mcp_error_response(
                415,
                None,
                -32600,
                "content type must be application/json",
            ));
            return;
        }
        if !mcp_accept_allowed(request.header("Accept")) {
            let _ = request.respond(mcp_error_response(
                406,
                None,
                -32600,
                "accept must include application/json",
            ));
            return;
        }
        let response = self.dispatch(request.body(), &request);
        let _ = request.respond(response);
    }

    fn admit_request(&self) -> bool {
        let Ok(mut rate) = self.rate.lock() else {
            return false;
        };
        if rate.started.elapsed() >= MCP_RATE_WINDOW {
            rate.started = Instant::now();
            rate.requests = 0;
        }
        if rate.requests >= MCP_RATE_LIMIT {
            return false;
        }
        rate.requests += 1;
        true
    }

    fn dispatch(&self, body: &[u8], request: &ExposeHttpRequest) -> ExposeHttpResponse {
        if !mcp_json_nesting_within_limit(body, MCP_MAX_JSON_NESTING) {
            return mcp_error_response(400, None, -32700, "parse error");
        }
        let message = match serde_json::from_slice::<Value>(body) {
            Ok(Value::Object(message)) => message,
            Ok(Value::Array(_)) => {
                return mcp_error_response(400, None, -32600, "batch requests are unsupported");
            }
            Ok(_) | Err(_) => {
                return mcp_error_response(400, None, -32700, "parse error");
            }
        };
        if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return mcp_error_response(400, request_id(&message), -32600, "invalid request");
        }
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return mcp_error_response(400, request_id(&message), -32600, "invalid request");
        };
        if let Some(response) = validate_mcp_request_headers(&message, method, request) {
            return response;
        }
        let id = request_id(&message);
        if !message.contains_key("id") {
            return if matches!(
                method,
                "notifications/initialized" | "notifications/cancelled"
            ) {
                expose_mcp_empty_response(202)
            } else {
                mcp_error_response(400, None, -32601, "notification is unsupported")
            };
        }
        if id.is_none() {
            return mcp_error_response(400, None, -32600, "invalid request id");
        }
        let params = message.get("params").unwrap_or(&Value::Null);
        match method {
            "server/discover" => self.server_discover(id),
            "initialize" => self.initialize(id, params, request.header("MCP-Protocol-Version")),
            "ping" => mcp_json_response(200, jsonrpc_result(id, json!({}))),
            "tools/list" => self.tools_list(id),
            "tools/call" => self.tools_call(id, params, &mcp_binding_key(request)),
            _ => mcp_error_response(404, id, -32601, "method not found"),
        }
    }

    fn initialize(
        &self,
        id: Option<Value>,
        params: &Value,
        header_version: Option<&str>,
    ) -> ExposeHttpResponse {
        let Some(params) = params.as_object() else {
            return mcp_error_response(400, id, -32602, "initialize params are required");
        };
        let Some(version) = params.get("protocolVersion").and_then(Value::as_str) else {
            return mcp_error_response(400, id, -32602, "protocolVersion is required");
        };
        if header_version.is_some_and(|header| header != version) {
            return mcp_error_response(
                400,
                id,
                MCP_ERROR_HEADER_MISMATCH,
                "protocol version header mismatch",
            );
        }
        if !MCP_PROTOCOL_VERSIONS.contains(&version) {
            return mcp_json_error(
                400,
                id,
                MCP_ERROR_UNSUPPORTED_VERSION,
                "unsupported protocol version",
                Some(json!({ "supported": MCP_PROTOCOL_VERSIONS, "requested": version })),
            );
        }
        mcp_json_response(
            200,
            jsonrpc_result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {
                        "name": self.server_name,
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": self.instructions(),
                }),
            ),
        )
    }

    fn server_discover(&self, id: Option<Value>) -> ExposeHttpResponse {
        mcp_json_response(
            200,
            jsonrpc_result(
                id,
                json!({
                    "resultType": "complete",
                    "supportedVersions": [MCP_CURRENT_PROTOCOL_VERSION],
                    "capabilities": {"tools": {"listChanged": false}},
                    "instructions": self.instructions(),
                    "ttlMs": 300_000,
                    "cacheScope": "private",
                    "_meta": {"io.modelcontextprotocol/serverInfo": {"name": self.server_name, "version": env!("CARGO_PKG_VERSION")}},
                }),
            ),
        )
    }

    fn instructions(&self) -> String {
        format!(
            "This is a local full-access Prodex Super runtime starting in {:?} (instance {}). The initial directory is context, not a filesystem jail: runs retain normal OS-user filesystem, process, network, Git, and local-tool authority. For development requests, resolve one compatible existing plain prodex s with prodex_session_prompt_write first, then read its exact returned PID, thread_id, and cursor. Start one prodex_super_start fallback only after authoritative no_session; never run both paths in parallel or treat ambiguity, stale identity, addressability, queue, source, or verification errors as no_session. A fresh idle prodex s needs no manual bootstrap prompt. Include consequential external actions in the user's task, and poll an existing run instead of starting duplicates. The expose URL is ephemeral capability authentication; anyone with it can control this instance.",
            self.workspace_name, self.instance_id
        )
    }

    fn tools_list(&self, id: Option<Value>) -> ExposeHttpResponse {
        mcp_json_response(
            200,
            jsonrpc_result(
                id,
                json!({
                    "resultType": "complete",
                    "tools": mcp_tools(),
                    "ttlMs": 300_000,
                    "cacheScope": "private",
                    "_meta": {"io.modelcontextprotocol/serverInfo": {"name": self.server_name, "version": env!("CARGO_PKG_VERSION")}},
                }),
            ),
        )
    }

    fn tools_call(
        &self,
        id: Option<Value>,
        params: &Value,
        binding_key: &str,
    ) -> ExposeHttpResponse {
        let Some(params) = params.as_object() else {
            return mcp_error_response(400, id, -32602, "tool parameters are required");
        };
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return mcp_error_response(400, id, -32602, "tool name is required");
        };
        let empty_arguments = Value::Object(Default::default());
        let arguments = params.get("arguments").unwrap_or(&empty_arguments);
        if !arguments.is_object() {
            return mcp_error_response(400, id, -32602, "tool arguments must be an object");
        }
        if let Err(message) = validate_tool_arguments(name, arguments) {
            return mcp_error_response(400, id, -32602, &message);
        }
        let result = match name {
            "prodex_super_start" => self.start_tool(arguments),
            "prodex_super_status" => self.status_tool(arguments),
            "prodex_super_events" => self.events_tool(arguments),
            "prodex_super_result" => self.result_tool(arguments),
            "prodex_super_cancel" => self.cancel_tool(arguments),
            "prodex_session_prompt_write" => self.session_prompt_write_tool(arguments, binding_key),
            "prodex_session_output_read" => self.output_read_tool(arguments, binding_key),
            "prodex_super_list" => Ok(json!({
                "instance_id": self.instance_id,
                "runs": self.run_manager.list().iter().map(run_summary_json).collect::<Vec<_>>()
            })),
            _ => {
                return mcp_json_response(
                    200,
                    jsonrpc_result(id, tool_result(json!({"error": "tool not found"}), true)),
                );
            }
        };
        match result {
            Ok(value) => mcp_json_response(200, jsonrpc_result(id, tool_result(value, false))),
            Err(message) => mcp_json_response(
                200,
                jsonrpc_result(id, tool_result(json!({ "error": message }), true)),
            ),
        }
    }

    fn session_prompt_write_tool(
        &self,
        arguments: &Value,
        binding_key: &str,
    ) -> std::result::Result<Value, String> {
        let message =
            required_string(arguments, "message", SESSION_PROMPT_WRITE_MAX_MESSAGE_BYTES)?;
        if message.as_bytes().contains(&0) {
            return Err("message must not contain NUL".to_string());
        }
        let request = SessionPromptWriteRequest {
            workspace_root: self.workspace_root.clone(),
            message,
            cwd: optional_string(arguments, "cwd", 4096)?,
            prodex_pid: optional_process_id(arguments)?,
            thread_id: optional_string(arguments, "thread_id", 128)?,
            binding_key: binding_key.to_string(),
        };
        let result = self
            .session_prompt_write
            .write(request)
            .map_err(|error| error.as_str().to_string())?;
        Ok(json!({
            "status": "written",
            "prodex_pid": result.prodex_pid,
            "codex_pid": result.codex_pid,
            "thread_id": result.thread_id,
            "message_id": result.message_id,
            "queue_exit": result.queue_exit,
            "verification": result.verification,
        }))
    }

    fn output_read_tool(
        &self,
        arguments: &Value,
        binding_key: &str,
    ) -> std::result::Result<Value, String> {
        let limit = arguments
            .get("limit")
            .map(value_usize)
            .transpose()?
            .unwrap_or(MCP_MAX_OUTPUT_EVENTS);
        if !(1..=MCP_MAX_OUTPUT_EVENTS).contains(&limit) {
            return Err(format!(
                "limit must be between 1 and {MCP_MAX_OUTPUT_EVENTS}"
            ));
        }
        let wait_ms = arguments
            .get("wait_ms")
            .map(value_u64)
            .transpose()?
            .unwrap_or_default();
        if wait_ms > MCP_MAX_OUTPUT_WAIT_MS {
            return Err(format!(
                "wait_ms must be between 0 and {MCP_MAX_OUTPUT_WAIT_MS}"
            ));
        }
        let request = PromptOutputReadRequest {
            workspace_root: self.workspace_root.clone(),
            cursor: optional_string(arguments, "cursor", MCP_MAX_CURSOR_BYTES)?,
            limit,
            wait_ms,
            prodex_pid: optional_process_id(arguments)?,
            thread_id: optional_string(arguments, "thread_id", 128)?,
            binding_key: binding_key.to_string(),
        };
        let result = self
            .session_prompt_write
            .read_output(request)
            .map_err(|error| error.as_str().to_string())?;
        Ok(json!({
            "status": "ok",
            "prodex_pid": result.prodex_pid,
            "codex_pid": result.codex_pid,
            "thread_id": result.thread_id,
            "source": result.source,
            "events": result.events.into_iter().map(|event| json!({
                "sequence": event.sequence,
                "timestamp": event.timestamp,
                "kind": event.kind,
                "name": event.name,
                "status": event.status,
                "text": event.text,
            })).collect::<Vec<_>>(),
            "next_cursor": result.next_cursor,
            "has_more": result.has_more,
        }))
    }

    pub(crate) fn start_tool(&self, arguments: &Value) -> std::result::Result<Value, String> {
        let task = required_string(arguments, "task", MCP_MAX_TASK_BYTES)?;
        if task.as_bytes().contains(&0) {
            return Err("task must not contain NUL".to_string());
        }
        let mut args = self.defaults.clone();
        if let Some(provider) = optional_string(arguments, "provider", MCP_MAX_MODEL_BYTES)? {
            apply_provider_override(&mut args, &provider)?;
        }
        if let Some(model) = optional_string(arguments, "model", MCP_MAX_MODEL_BYTES)? {
            args.local_model = Some(model);
        }
        if let Some(effort) = optional_string(arguments, "reasoning_effort", MCP_MAX_MODEL_BYTES)? {
            let parsed = effort
                .parse::<prodex_cli::SubAgentReasoningEffort>()
                .map_err(|_| "reasoning_effort is unsupported".to_string())?;
            let provider = main_provider(&args);
            let configured_model =
                crate::codex_cli_config_override_value(&args.codex_args, "model");
            let model = args.local_model.as_deref().or(configured_model.as_deref());
            if !crate::canonical_sub_agent_efforts(provider, model).contains(&parsed) {
                return Err("reasoning_effort is unsupported for the selected model".to_string());
            }
            args.codex_args.extend([
                OsString::from("-c"),
                OsString::from(format!(
                    "model_reasoning_effort={}",
                    crate::runtime_catalog_config::toml_string_literal(&effort)
                )),
            ]);
        }
        if let Some(profile) = optional_string(arguments, "profile", MCP_MAX_PROFILE_BYTES)? {
            prodex_profile_identity::validate_profile_name(&profile)
                .map_err(|_| "profile is invalid".to_string())?;
            args.profile = Some(profile);
        }
        if let Some(sub_agents) = arguments.get("sub_agents")
            && !sub_agents.is_null()
        {
            let Some(sub_agents) = sub_agents.as_bool() else {
                return Err("sub_agents must be a boolean".to_string());
            };
            if sub_agents {
                args.sub_agent = true;
                args.no_sub_agent = false;
            } else {
                args.sub_agent = false;
                args.no_sub_agent = true;
                args.sub_agent_provider = None;
                args.sub_agent_model = None;
                args.sub_agent_model_reasoning_effort = None;
                args.sub_agent_url = None;
                args.sub_agent_max_concurrency = None;
            }
        }
        validate_configured_main_effort(&args)?;
        if args
            .cli
            .is_some_and(|agent| agent != prodex_cli::SuperCliAgent::Codex)
        {
            crate::runtime_gemini_cli::validate_super_native_cli_preflight(&args)
                .map_err(|error| error.to_string())?;
        } else {
            args.validate_urls()
                .map_err(|_| "run configuration is invalid".to_string())?;
        }
        let summary = self.run_manager.start(task, args).map_err(str::to_string)?;
        Ok(json!({
            "run_id": summary.run_id,
            "state": summary.state.as_str(),
        }))
    }

    fn status_tool(&self, arguments: &Value) -> std::result::Result<Value, String> {
        let run_id = required_run_id(arguments)?;
        let mut value = self.run_manager.status(&run_id).map_or_else(
            || json!({"run_id": run_id, "state": "unknown"}),
            |summary| run_summary_json(&summary),
        );
        value["instance_id"] = json!(self.instance_id);
        Ok(value)
    }

    fn events_tool(&self, arguments: &Value) -> std::result::Result<Value, String> {
        let run_id = required_run_id(arguments)?;
        let after_seq = arguments
            .get("after_seq")
            .map(value_u64)
            .transpose()?
            .unwrap_or(0);
        let limit = event_page_limit(arguments)?;
        let mut value = self.run_manager.events(&run_id, after_seq, limit).map_or_else(
            || json!({"run_id": run_id, "state": "unknown", "events": [], "next_seq": 0, "truncated": false}),
            |events| json!({
                "run_id": run_id,
                "events": events.events.iter().map(|event| json!({"seq": event.seq, "type": event.event_type, "text": event.text})).collect::<Vec<_>>(),
                "next_seq": events.next_seq,
                "truncated": events.truncated,
            }),
        );
        value["instance_id"] = json!(self.instance_id);
        Ok(value)
    }

    fn result_tool(&self, arguments: &Value) -> std::result::Result<Value, String> {
        let run_id = required_run_id(arguments)?;
        let mut value = self.run_manager.result(&run_id).map_or_else(
            || json!({"run_id": run_id, "state": "unknown"}),
            |result| {
                json!({
                    "run_id": result.summary.run_id,
                    "state": result.summary.state.as_str(),
                    "exit_status": result.summary.exit_status,
                    "output": result.output,
                    "output_truncated": result.output_truncated,
                    "created_at": result.summary.created_at,
                    "started_at": result.summary.started_at,
                    "finished_at": result.summary.finished_at,
                    "provider": result.summary.provider,
                    "model": result.summary.model,
                    "reasoning_effort": result.summary.reasoning_effort,
                })
            },
        );
        value["instance_id"] = json!(self.instance_id);
        Ok(value)
    }

    fn cancel_tool(&self, arguments: &Value) -> std::result::Result<Value, String> {
        let run_id = required_run_id(arguments)?;
        let mut value = self.run_manager.cancel(&run_id).map_or_else(
            || json!({"run_id": run_id, "state": "unknown"}),
            |summary| {
                json!({
                    "run_id": summary.run_id,
                    "state": summary.state.as_str(),
                    "cancellation_requested": summary.cancellation_requested,
                })
            },
        );
        value["instance_id"] = json!(self.instance_id);
        Ok(value)
    }
}
