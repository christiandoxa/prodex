use anyhow::{Context, Result};
use prodex_mcp_stdio::{read_mcp_message, write_mcp_message};
use reqwest::blocking::Client;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::env;
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_MEMORY_LIMIT: i64 = 20;
const PRODEX_MEMORY_BACKEND_ENV: &str = "PRODEX_MEMORY_BACKEND";
const PRODEX_MEM0_API_URL_ENV: &str = "PRODEX_MEM0_API_URL";
const PRODEX_MEM0_API_KEY_ENV: &str = "PRODEX_MEM0_API_KEY";
const MEMORY_MCP_INVALID_PARAMS_MESSAGE: &str = "invalid memory tool parameters";
const MEMORY_MCP_METHOD_NOT_FOUND_MESSAGE: &str = "method not found";

pub fn default_memory_store_path(prodex_home: &Path) -> PathBuf {
    prodex_home.join("memory").join("prodex-memory.sqlite")
}

pub fn memory_store_ready(path: &Path) -> Result<()> {
    let conn = open_memory_store(path)?;
    initialize_memory_store(&conn)
}

pub fn run_memory_mcp_stdio(store: &Path) -> Result<()> {
    let backend = MemoryBackend::from_env_or_sqlite(store)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    while let Some((request, framing)) = read_mcp_message(&mut reader)? {
        let Some(response) = handle_mcp_request(&backend, request)? else {
            continue;
        };
        write_mcp_message(&mut writer, &response, framing)?;
    }
    Ok(())
}

enum MemoryBackend {
    Sqlite(Connection),
    Mem0(Mem0MemoryClient),
}

impl MemoryBackend {
    fn from_env_or_sqlite(store: &Path) -> Result<Self> {
        let backend = env::var(PRODEX_MEMORY_BACKEND_ENV)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if backend == "mem0" {
            return Ok(Self::Mem0(Mem0MemoryClient::from_env()?));
        }

        let conn = open_memory_store(store)?;
        initialize_memory_store(&conn)?;
        Ok(Self::Sqlite(conn))
    }
}

struct Mem0MemoryClient {
    api_url: String,
    api_key: String,
    client: Client,
}

impl Mem0MemoryClient {
    fn from_env() -> Result<Self> {
        let api_url = env::var(PRODEX_MEM0_API_URL_ENV)
            .with_context(|| format!("{PRODEX_MEM0_API_URL_ENV} is required for mem0 memory"))?
            .trim()
            .trim_end_matches('/')
            .to_string();
        if api_url.is_empty() {
            anyhow::bail!("{PRODEX_MEM0_API_URL_ENV} cannot be empty");
        }
        let api_key = env::var(PRODEX_MEM0_API_KEY_ENV)
            .with_context(|| format!("{PRODEX_MEM0_API_KEY_ENV} is required for mem0 memory"))?
            .trim()
            .to_string();
        if api_key.is_empty() {
            anyhow::bail!("{PRODEX_MEM0_API_KEY_ENV} cannot be empty");
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to build Mem0 HTTP client")?;
        Ok(Self {
            api_url,
            api_key,
            client,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/{}", self.api_url, path.trim_start_matches('/'))
    }

    fn endpoint_with_query(&self, path: &str, params: &[(&str, &str)]) -> String {
        if params.is_empty() {
            return self.endpoint(path);
        }
        let query = params
            .iter()
            .map(|(key, value)| {
                format!(
                    "{}={}",
                    percent_encode_query_component(key),
                    percent_encode_query_component(value)
                )
            })
            .collect::<Vec<_>>()
            .join("&");
        format!("{}?{query}", self.endpoint(path))
    }

    fn get_with_query(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> reqwest::blocking::RequestBuilder {
        self.client
            .get(self.endpoint_with_query(path, params))
            .header("X-API-Key", &self.api_key)
    }

    fn post(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client
            .post(self.endpoint(path))
            .header("X-API-Key", &self.api_key)
    }

    fn delete(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client
            .delete(self.endpoint(path))
            .header("X-API-Key", &self.api_key)
    }
}

fn open_memory_store(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Connection::open(path).with_context(|| format!("failed to open {}", path.display()))
}

fn initialize_memory_store(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS memories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scope TEXT NOT NULL DEFAULT 'default',
            text TEXT NOT NULL,
            tags TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS memories_scope_updated_idx
            ON memories(scope, updated_at DESC);
        "#,
    )
    .context("failed to initialize memory store")?;
    Ok(())
}

fn handle_mcp_request(backend: &MemoryBackend, request: Value) -> Result<Option<Value>> {
    let id = request.get("id").cloned();
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if id.is_none() {
        return Ok(None);
    }
    let id = id.unwrap_or(Value::Null);
    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "prodex-memory", "version": env!("CARGO_PKG_VERSION") }
        }),
        "ping" => json!({}),
        "tools/list" => json!({ "tools": memory_tools() }),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            match handle_tool_call(backend, params) {
                Ok(result) => result,
                Err(_err) => {
                    return Ok(Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32602, "message": MEMORY_MCP_INVALID_PARAMS_MESSAGE }
                    })));
                }
            }
        }
        _ => {
            return Ok(Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": MEMORY_MCP_METHOD_NOT_FOUND_MESSAGE }
            })));
        }
    };
    Ok(Some(
        json!({ "jsonrpc": "2.0", "id": id, "result": result }),
    ))
}

fn memory_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "memory_add",
            "description": "Store a local Prodex memory. Use for stable user preferences, project facts, and reusable context.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "scope": { "type": "string", "default": "default" },
                    "tags": { "type": "string", "description": "Optional comma-separated tags." }
                },
                "required": ["text"]
            }
        }),
        json!({
            "name": "memory_search",
            "description": "Search local Prodex memories by substring.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "scope": { "type": "string", "default": "default" },
                    "limit": { "type": "integer", "default": DEFAULT_MEMORY_LIMIT }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "memory_context",
            "description": "Return recent local Prodex memories for the current scope.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope": { "type": "string", "default": "default" },
                    "limit": { "type": "integer", "default": DEFAULT_MEMORY_LIMIT }
                }
            }
        }),
        json!({
            "name": "memory_delete",
            "description": "Delete a local Prodex memory by id.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer" } },
                "required": ["id"]
            }
        }),
    ]
}

fn handle_tool_call(backend: &MemoryBackend, params: Value) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let output = match name {
        "memory_add" => match backend {
            MemoryBackend::Sqlite(conn) => memory_add(conn, &args)?,
            MemoryBackend::Mem0(client) => mem0_memory_add(client, &args)?,
        },
        "memory_search" => match backend {
            MemoryBackend::Sqlite(conn) => memory_search(conn, &args, false)?,
            MemoryBackend::Mem0(client) => mem0_memory_search(client, &args)?,
        },
        "memory_context" => match backend {
            MemoryBackend::Sqlite(conn) => memory_search(conn, &args, true)?,
            MemoryBackend::Mem0(client) => mem0_memory_context(client, &args)?,
        },
        "memory_delete" => match backend {
            MemoryBackend::Sqlite(conn) => memory_delete(conn, &args)?,
            MemoryBackend::Mem0(client) => mem0_memory_delete(client, &args)?,
        },
        _ => anyhow::bail!("unknown memory tool"),
    };
    Ok(json!({ "content": [{ "type": "text", "text": output }] }))
}

fn mem0_memory_add(client: &Mem0MemoryClient, args: &Value) -> Result<String> {
    let text = required_string(args, "text")?;
    let scope = optional_string(args, "scope").unwrap_or_else(|| "default".to_string());
    let tags = optional_string(args, "tags");
    let mut metadata = serde_json::Map::new();
    if let Some(tags) = tags {
        metadata.insert("tags".to_string(), json!(tags));
    }
    let response = client
        .post("/memories")
        .json(&json!({
            "messages": [{ "role": "user", "content": text }],
            "user_id": scope,
            "metadata": metadata,
            "infer": false,
        }))
        .send()
        .context("failed to call Mem0 /memories")?;
    let status = response.status();
    let body: Value = response
        .json()
        .with_context(|| format!("failed to parse Mem0 /memories response ({status})"))?;
    if !status.is_success() {
        anyhow::bail!("Mem0 /memories returned {status}: {body}");
    }
    Ok(format!(
        "stored Mem0 memory: {}",
        summarize_mem0_value(&body)
    ))
}

fn mem0_memory_search(client: &Mem0MemoryClient, args: &Value) -> Result<String> {
    let query = required_string(args, "query")?;
    let scope = optional_string(args, "scope").unwrap_or_else(|| "default".to_string());
    let limit = optional_i64(args, "limit")
        .unwrap_or(DEFAULT_MEMORY_LIMIT)
        .clamp(1, 100);
    let response = client
        .post("/search")
        .json(&json!({
            "query": query,
            "filters": { "user_id": scope },
            "top_k": limit,
        }))
        .send()
        .context("failed to call Mem0 /search")?;
    let status = response.status();
    let body: Value = response
        .json()
        .with_context(|| format!("failed to parse Mem0 /search response ({status})"))?;
    if !status.is_success() {
        anyhow::bail!("Mem0 /search returned {status}: {body}");
    }
    format_mem0_memories(&body, limit as usize)
}

fn mem0_memory_context(client: &Mem0MemoryClient, args: &Value) -> Result<String> {
    let scope = optional_string(args, "scope").unwrap_or_else(|| "default".to_string());
    let limit = optional_i64(args, "limit")
        .unwrap_or(DEFAULT_MEMORY_LIMIT)
        .clamp(1, 100);
    let response = client
        .get_with_query("/memories", &[("user_id", scope.as_str())])
        .send()
        .context("failed to call Mem0 /memories")?;
    let status = response.status();
    let body: Value = response
        .json()
        .with_context(|| format!("failed to parse Mem0 /memories response ({status})"))?;
    if !status.is_success() {
        anyhow::bail!("Mem0 /memories returned {status}: {body}");
    }
    format_mem0_memories(&body, limit as usize)
}

fn mem0_memory_delete(client: &Mem0MemoryClient, args: &Value) -> Result<String> {
    let id = required_string(args, "id").or_else(|_| {
        optional_i64(args, "id")
            .map(|id| id.to_string())
            .ok_or_else(|| anyhow::anyhow!("missing id"))
    })?;
    let response = client
        .delete(&format!("/memories/{id}"))
        .send()
        .context("failed to call Mem0 delete memory")?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("Mem0 delete returned {status}: {body}");
    }
    Ok(format!("deleted Mem0 memory id={id}"))
}

fn format_mem0_memories(value: &Value, limit: usize) -> Result<String> {
    let rows = mem0_result_rows(value);
    if rows.is_empty() {
        return Ok("no memories found".to_string());
    }
    let mut output = Vec::new();
    for row in rows.into_iter().take(limit) {
        let id = row
            .get("id")
            .or_else(|| row.get("memory_id"))
            .map(mem0_scalar)
            .unwrap_or_else(|| "-".to_string());
        let text = row
            .get("memory")
            .or_else(|| row.get("text"))
            .or_else(|| row.get("data"))
            .map(mem0_scalar)
            .unwrap_or_else(|| summarize_mem0_value(row));
        let score = row
            .get("score")
            .or_else(|| row.pointer("/score_details/score"))
            .map(mem0_scalar)
            .map(|score| format!(" score={score}"))
            .unwrap_or_default();
        output.push(format!("#{id} {text}{score}"));
    }
    Ok(output.join("\n"))
}

fn mem0_result_rows(value: &Value) -> Vec<&Value> {
    if let Some(rows) = value.as_array() {
        return rows.iter().collect();
    }
    if let Some(rows) = value.get("results").and_then(Value::as_array) {
        return rows.iter().collect();
    }
    if let Some(rows) = value.get("memories").and_then(Value::as_array) {
        return rows.iter().collect();
    }
    Vec::new()
}

fn summarize_mem0_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn mem0_scalar(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

fn percent_encode_query_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

fn memory_add(conn: &Connection, args: &Value) -> Result<String> {
    let text = required_string(args, "text")?;
    let scope = optional_string(args, "scope").unwrap_or_else(|| "default".to_string());
    let tags = optional_string(args, "tags").unwrap_or_default();
    let now = now_epoch_seconds();
    conn.execute(
        "INSERT INTO memories (scope, text, tags, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![scope, text, tags, now, now],
    )
    .context("failed to insert memory")?;
    Ok(format!("stored memory id={}", conn.last_insert_rowid()))
}

fn memory_search(conn: &Connection, args: &Value, recent_only: bool) -> Result<String> {
    let scope = optional_string(args, "scope").unwrap_or_else(|| "default".to_string());
    let limit = optional_i64(args, "limit")
        .unwrap_or(DEFAULT_MEMORY_LIMIT)
        .clamp(1, 100);
    let rows = if recent_only {
        query_memories(
            conn,
            "SELECT id, scope, text, tags, updated_at FROM memories WHERE scope = ?1 ORDER BY updated_at DESC LIMIT ?2",
            params![scope, limit],
        )?
    } else {
        let query = required_string(args, "query")?;
        let pattern = format!("%{query}%");
        query_memories(
            conn,
            "SELECT id, scope, text, tags, updated_at FROM memories WHERE scope = ?1 AND (text LIKE ?2 OR tags LIKE ?2) ORDER BY updated_at DESC LIMIT ?3",
            params![scope, pattern, limit],
        )?
    };
    if rows.is_empty() {
        return Ok("no memories found".to_string());
    }
    Ok(rows.join("\n"))
}

fn memory_delete(conn: &Connection, args: &Value) -> Result<String> {
    let id = optional_i64(args, "id").ok_or_else(|| anyhow::anyhow!("missing id"))?;
    let deleted = conn
        .execute("DELETE FROM memories WHERE id = ?1", params![id])
        .context("failed to delete memory")?;
    if deleted == 0 {
        Ok(format!("memory id={id} not found"))
    } else {
        Ok(format!("deleted memory id={id}"))
    }
}

fn query_memories<P>(conn: &Connection, sql: &str, params: P) -> Result<Vec<String>>
where
    P: rusqlite::Params,
{
    let mut stmt = conn
        .prepare(sql)
        .context("failed to prepare memory query")?;
    let rows = stmt
        .query_map(params, |row| {
            let id: i64 = row.get(0)?;
            let scope: String = row.get(1)?;
            let text: String = row.get(2)?;
            let tags: String = row.get(3)?;
            let updated_at: i64 = row.get(4)?;
            let tag_suffix = if tags.trim().is_empty() {
                String::new()
            } else {
                format!(" tags={tags}")
            };
            Ok(format!(
                "#{id} [{scope}] {text}{tag_suffix} updated_at={updated_at}"
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read memory rows")?;
    Ok(rows)
}

fn required_string(args: &Value, key: &str) -> Result<String> {
    optional_string(args, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing {key}"))
}

fn optional_string(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn optional_i64(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(Value::as_i64)
}

fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_mcp_stdio::McpMessageFraming;

    #[test]
    fn memory_store_adds_and_searches_rows() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_memory_store(&conn).unwrap();
        let backend = MemoryBackend::Sqlite(conn);
        let add = handle_tool_call(
            &backend,
            json!({
                "name": "memory_add",
                "arguments": { "text": "Prefer terse replies", "tags": "style" }
            }),
        )
        .unwrap();
        assert!(
            add["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("stored memory id=")
        );
        let search = handle_tool_call(
            &backend,
            json!({
                "name": "memory_search",
                "arguments": { "query": "terse" }
            }),
        )
        .unwrap();
        assert!(
            search["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Prefer terse replies")
        );
    }

    #[test]
    fn mcp_initialize_returns_server_info() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_memory_store(&conn).unwrap();
        let backend = MemoryBackend::Sqlite(conn);
        let response = handle_mcp_request(
            &backend,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
        )
        .unwrap()
        .unwrap();
        assert_eq!(response["result"]["serverInfo"]["name"], "prodex-memory");
    }

    #[test]
    fn mcp_response_uses_request_framing() {
        let response = json!({"jsonrpc":"2.0","id":1,"result":{}});

        let mut json_line = Vec::new();
        write_mcp_message(&mut json_line, &response, McpMessageFraming::JsonLine).unwrap();
        assert_eq!(json_line.last(), Some(&b'\n'));
        assert!(!json_line.starts_with(b"Content-Length:"));

        let mut content_length = Vec::new();
        write_mcp_message(
            &mut content_length,
            &response,
            McpMessageFraming::ContentLength,
        )
        .unwrap();
        assert!(content_length.starts_with(b"Content-Length:"));
    }

    #[test]
    fn mcp_invalid_params_error_is_stable_and_redacted() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_memory_store(&conn).unwrap();
        let backend = MemoryBackend::Sqlite(conn);
        let response = handle_mcp_request(
            &backend,
            json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"tools/call",
                "params": { "name": "memory_add", "arguments": { "text": 7, "secret": "secret-token-123" } }
            }),
        )
        .unwrap()
        .unwrap();

        assert_eq!(response["error"]["code"], -32602);
        assert_eq!(
            response["error"]["message"],
            MEMORY_MCP_INVALID_PARAMS_MESSAGE
        );
        assert!(!response.to_string().contains("secret-token-123"));
    }

    #[test]
    fn mcp_unknown_tool_error_is_stable_and_redacted() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_memory_store(&conn).unwrap();
        let backend = MemoryBackend::Sqlite(conn);
        let response = handle_mcp_request(
            &backend,
            json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"tools/call",
                "params": { "name": "secret-token-123", "arguments": {} }
            }),
        )
        .unwrap()
        .unwrap();

        assert_eq!(response["error"]["code"], -32602);
        assert_eq!(
            response["error"]["message"],
            MEMORY_MCP_INVALID_PARAMS_MESSAGE
        );
        assert!(!response.to_string().contains("secret-token-123"));
    }

    #[test]
    fn mcp_unknown_method_error_is_stable_and_redacted() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_memory_store(&conn).unwrap();
        let backend = MemoryBackend::Sqlite(conn);
        let response = handle_mcp_request(
            &backend,
            json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":"secret-token-123"
            }),
        )
        .unwrap()
        .unwrap();

        assert_eq!(response["error"]["code"], -32601);
        assert_eq!(
            response["error"]["message"],
            MEMORY_MCP_METHOD_NOT_FOUND_MESSAGE
        );
        assert!(!response.to_string().contains("secret-token-123"));
    }

    #[test]
    fn mem0_result_formatter_handles_search_payload() {
        let output = format_mem0_memories(
            &json!({
                "results": [
                    {"id": "abc", "memory": "Use terse replies", "score": 0.9}
                ]
            }),
            10,
        )
        .unwrap();
        assert!(output.contains("#abc Use terse replies score=0.9"));
    }

    #[test]
    fn mem0_add_uses_raw_no_infer_payload() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_ip().unwrap();
        let (body_tx, body_rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let mut request = server.recv().unwrap();
            let mut body = Vec::new();
            std::io::Read::read_to_end(&mut request.as_reader(), &mut body).unwrap();
            body_tx.send(body).unwrap();
            let mut response = tiny_http::Response::from_string(r#"{"id":"mem0-test"}"#);
            response.add_header(
                tiny_http::Header::from_bytes("content-type", "application/json").unwrap(),
            );
            let _ = request.respond(response);
        });
        let client = Mem0MemoryClient {
            api_url: format!("http://{addr}"),
            api_key: "test-key".to_string(),
            client: Client::new(),
        };

        mem0_memory_add(
            &client,
            &json!({
                "text": "Prefer terse replies",
                "scope": "default",
            }),
        )
        .unwrap();
        thread.join().unwrap();
        let body = body_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["infer"], false);
        assert_eq!(body["user_id"], "default");
        assert_eq!(body["messages"][0]["content"], "Prefer terse replies");
    }
}
