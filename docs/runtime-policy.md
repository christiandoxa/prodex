# Runtime Policy Reference

Prodex reads `policy.toml` from the Prodex root, usually `~/.prodex/policy.toml` unless `PRODEX_HOME` is set.
Environment variables override policy values, and unset values fall back to built-in defaults.

Relative `runtime.log_dir` values are resolved under the Prodex root. `PRODEX_RUNTIME_LOG_DIR` is used as provided.
Use `prodex info` for effective tuning values and `prodex doctor --runtime --json` for the resolved runtime log directory, format, and current `log_path`.

```bash
prodex doctor --runtime --json
prodex doctor --runtime --json | jq -r '.log_path'
prodex doctor --runtime --json | jq -r '.runtime_logs.directory'
```

Defaults below are production defaults. Test builds use smaller timeouts and limits in several places.

## Runtime Keys

| Policy key | Environment override | Default | Meaning |
| --- | --- | --- | --- |
| `runtime.log_dir` | `PRODEX_RUNTIME_LOG_DIR` | OS temp directory, usually `/tmp` on Linux | Directory for `prodex-runtime-latest.path` and per-run `prodex-runtime-*.log` files. |
| `runtime.log_format` | `PRODEX_RUNTIME_LOG_FORMAT` | `text` | Runtime proxy log format. Valid values: `text`, `json`. |

## Gateway Keys

`prodex gateway` runs a standalone OpenAI-compatible HTTP gateway.
Native OpenAI-compatible upstreams are passed through for `/v1/responses`, `/v1/chat/completions`, `/v1/embeddings`, `/v1/images/*`, `/v1/audio/*`, `/v1/batches`, `/v1/rerank`, `/v1/a2a`, `/v1/messages`, and `/v1/models`.
Provider bridges translate `/v1/responses` where supported and pass native-compatible side endpoints through to the selected upstream.

| Policy key | Environment override | Default | Meaning |
| --- | --- | --- | --- |
| `gateway.listen_addr` | none | `127.0.0.1:4000` | Gateway bind address. Non-loopback binds require `--auth-token` or `PRODEX_GATEWAY_TOKEN`. |
| `gateway.provider` | CLI `--provider` | OpenAI-compatible upstream | Provider preset: `anthropic`, `copilot`, `deepseek`, or `gemini`. |
| `gateway.base_url` | CLI `--base-url`; `OPENAI_BASE_URL` for OpenAI-compatible mode | Provider default, or `https://api.openai.com/v1` | Upstream base URL. OpenAI-compatible mode appends `/v1` when the URL has no path. |
| `gateway.require_auth` | none | `false` | Require gateway bearer auth even on loopback. Token value comes from `--auth-token` or `PRODEX_GATEWAY_TOKEN`. |
| `gateway.route_aliases` | none | empty | Declarative model aliases. Matching request `model` values are rewritten according to each alias `strategy`. |
| `gateway.route_aliases[].strategy` | none | `fallback` | Routing strategy for the alias: `fallback` rewrites to `combo:...`, `round-robin` selects one target by request id, `least-busy` selects the target with the fewest in-flight gateway requests, `first` always picks the first target. |
| `gateway.route_aliases[].model_metrics` | none | empty | Optional per-model routing hints for metric strategies: cost, latency, RPM limit, and TPM limit. |
| `gateway.observability.sinks` | none | `runtime-log` | Enabled gateway observability sinks. `runtime-log` is always enabled; `jsonl` and `http` are enabled automatically when their target fields are set. |
| `gateway.observability.call_id_header` | none | `x-prodex-call-id` | Response header containing a stable per-request call id such as `prodex-42`. |
| `gateway.observability.jsonl_path` | none | empty | Optional JSONL export path for structured `gateway_spend` events. Relative paths are resolved under the Prodex root. |
| `gateway.observability.http_endpoint` | none | empty | Optional HTTP JSON export endpoint for structured `gateway_spend` events. |
| `gateway.observability.http_schema` | none | `generic` | HTTP export payload schema: `generic`, `otel`, `datadog`, or `langfuse`. |
| `gateway.observability.http_bearer_token_env` | none | empty | Environment variable name containing a bearer token for `gateway.observability.http_endpoint`. |
| `gateway.guardrails.blocked_keywords` | none | empty | Case-insensitive pre-call keyword blocks applied before upstream send. |
| `gateway.guardrails.blocked_output_keywords` | none | empty | Case-insensitive output keyword blocks. Buffered responses are replaced with `403 policy_violation`; streaming responses are stopped and logged when a keyword is observed. |
| `gateway.guardrails.allowed_models` | none | empty | Optional pre-call allowlist for request `model` values, checked before route alias rewrite. |
| `gateway.guardrails.presidio_redaction` | CLI `--presidio` / `--no-presidio` | `false` | Enable Presidio request-body redaction for gateway traffic. |
| `gateway.guardrails.prompt_injection_detection` | none | `false` | Enable built-in prompt-injection heuristic checks before upstream send. |
| `gateway.guardrails.webhook_url` | none | empty | Optional external guardrail HTTP endpoint. Prodex sends base64 request/response bodies and expects JSON such as `{"allow": false, "reason": "...", "message": "..."}` to block. |
| `gateway.guardrails.webhook_phases` | none | both phases | External guardrail phases: `pre` for requests before upstream send, `post` for buffered responses before returning to caller. |
| `gateway.guardrails.webhook_bearer_token_env` | none | empty | Environment variable name containing a bearer token for `gateway.guardrails.webhook_url`. |
| `gateway.guardrails.webhook_fail_closed` | none | `false` | Block when the external guardrail endpoint fails or returns non-2xx. |

Example:

```toml
[gateway]
listen_addr = "127.0.0.1:4000"
provider = "gemini"
require_auth = true

[[gateway.route_aliases]]
alias = "prodex-fast"
models = ["gemini-3-flash", "gemini-2.5-flash"]
strategy = "fallback"

[[gateway.route_aliases.model_metrics]]
model = "gemini-3-flash"
input_cost_per_million_microusd = 100
output_cost_per_million_microusd = 200
latency_ms = 300
rpm_limit = 60
tpm_limit = 100000

[gateway.observability]
sinks = ["runtime-log", "jsonl", "http"]
call_id_header = "x-prodex-call-id"
jsonl_path = "gateway-spend.jsonl"
http_endpoint = "https://otel-collector.example/v1/events"
http_schema = "otel"
http_bearer_token_env = "PRODEX_GATEWAY_OBSERVABILITY_TOKEN"

[gateway.guardrails]
blocked_keywords = ["secret project"]
blocked_output_keywords = ["do not reveal"]
allowed_models = ["prodex-fast"]
presidio_redaction = true
prompt_injection_detection = true
webhook_url = "https://guardrails.example/check"
webhook_phases = ["pre", "post"]
webhook_bearer_token_env = "PRODEX_GATEWAY_GUARDRAIL_TOKEN"
webhook_fail_closed = true
```

## Runtime Proxy Keys

`runtime_proxy.preset` selects a conservative preset before individual `runtime_proxy` keys are applied.
Valid values are `low`, `default`, `many-terminals`, and `aggressive`; `PRODEX_RUNTIME_PROXY_PRESET` selects the preset from the environment.
Specific environment overrides for individual keys still have highest priority.
Unknown policy preset values are rejected when `policy.toml` is parsed. Unknown environment preset values are ignored so the configured policy or built-in defaults still apply.
The preset changes only local concurrency and admission tuning; transport timeouts remain on their normal defaults unless configured directly.

## Runtime Proxy Contract

`runtime_proxy` tuning must preserve these invariants:

- Prodex stays a scoped Codex gateway, not a general-purpose LLM SDK.
- Profile selection must be visible through policy, `prodex info`, `prodex doctor`, and runtime logs.
- Pre-commit retry and fallback paths must stay bounded per request.
- Runtime hot paths must avoid broad disk reads, quota probes, or blocking state saves.
- Quota, budget, transport, and local pressure signals must stay classified separately.
- Selection, admission, affinity, backoff, and first-chunk events must be structured in runtime logs.
- Upstream HTTP/WebSocket connection reuse should be preserved where it does not change Codex semantics.
- Secrets remain profile-isolated, redacted in diagnostics, and covered by audit events for Prodex-owned mutations.

<!-- BEGIN GENERATED RUNTIME_PROXY_KEYS -->
| Policy key | Environment override | Default | Meaning |
| --- | --- | --- | --- |
| `runtime_proxy.worker_count` | `PRODEX_RUNTIME_PROXY_WORKER_COUNT` | CPU parallelism clamped to `4..12` | Short-lived proxy worker pool size. |
| `runtime_proxy.long_lived_worker_count` | `PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT` | `parallelism * 2` clamped to `8..24` | Worker pool for long-lived streams and websocket work. |
| `runtime_proxy.probe_refresh_worker_count` | `PRODEX_RUNTIME_PROBE_REFRESH_WORKER_COUNT` | CPU parallelism clamped to `2..4` | Background profile probe refresh workers. |
| `runtime_proxy.async_worker_count` | `PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT` | CPU parallelism clamped to `2..4` | Async runtime worker count. |
| `runtime_proxy.long_lived_queue_capacity` | `PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY` | `long_lived_worker_count * 8` clamped to `128..1024` | Queue capacity for long-lived proxy work. |
| `runtime_proxy.active_request_limit` | `PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT` | `worker_count + long_lived_worker_count * 3` clamped to `64..512` | Global local admission cap for fresh runtime proxy requests. |
| `runtime_proxy.responses_active_limit` | `PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT` | `75%` of global limit, clamped to `4..global` | Lane cap for main Responses traffic. |
| `runtime_proxy.compact_active_limit` | `PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT` | `25%` of global limit, clamped to `2..6` | Lane cap for `/responses/compact`. |
| `runtime_proxy.websocket_active_limit` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT` | `long_lived_worker_count` clamped to `2..global` | Lane cap for websocket transport. |
| `runtime_proxy.standard_active_limit` | `PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT` | `worker_count / 2` clamped to `2..8` | Lane cap for other unary proxy traffic. |
| `runtime_proxy.profile_inflight_soft_limit` | `PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT` | `4` | Fresh selection starts penalizing profiles above this in-flight count. |
| `runtime_proxy.profile_inflight_hard_limit` | `PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT` | `8` | Fresh selection avoids profiles above this in-flight count; hard affinity still wins. |
| `runtime_proxy.admission_wait_budget_ms` | `PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS` | `750` | Normal wait budget for local admission pressure. |
| `runtime_proxy.pressure_admission_wait_budget_ms` | `PRODEX_RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS` | `200` | Shorter admission wait budget when proxy is already under pressure. |
| `runtime_proxy.long_lived_queue_wait_budget_ms` | `PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS` | `750` | Normal wait budget for long-lived queue pressure. |
| `runtime_proxy.pressure_long_lived_queue_wait_budget_ms` | `PRODEX_RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS` | `200` | Shorter long-lived queue wait budget under pressure. |
| `runtime_proxy.http_connect_timeout_ms` | `PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS` | `5000` | Upstream HTTP connect timeout. |
| `runtime_proxy.stream_idle_timeout_ms` | `PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS` | `300000` | Responses stream idle timeout, aligned with Codex behavior. |
| `runtime_proxy.compact_request_timeout_ms` | `PRODEX_RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS` | `90000` | Total request timeout for unary remote compact calls before Codex can observe failure and retry. |
| `runtime_proxy.sse_lookahead_timeout_ms` | `PRODEX_RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS` | `1000` | Pre-commit SSE lookahead timeout. |
| `runtime_proxy.prefetch_backpressure_retry_ms` | `PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS` | `10` | Retry delay while stream prefetch is backpressured. |
| `runtime_proxy.prefetch_backpressure_timeout_ms` | `PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS` | `1000` | Max wait for stream prefetch backpressure to clear. |
| `runtime_proxy.prefetch_max_buffered_bytes` | `PRODEX_RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES` | `786432` | Max buffered prefetch bytes before backpressure. |
| `runtime_proxy.websocket_connect_timeout_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS` | `15000` | Upstream websocket connect timeout. |
| `runtime_proxy.websocket_happy_eyeballs_delay_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS` | `200` | Delay before alternate websocket TCP connect attempt. |
| `runtime_proxy.websocket_precommit_progress_timeout_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS` | `8000` | Websocket pre-commit progress timeout. |
| `runtime_proxy.websocket_connect_worker_count` | `PRODEX_RUNTIME_WEBSOCKET_CONNECT_WORKER_COUNT` | CPU parallelism clamped to `4..16` | Worker count for bounded websocket TCP connect executor. |
| `runtime_proxy.websocket_connect_queue_capacity` | `PRODEX_RUNTIME_WEBSOCKET_CONNECT_QUEUE_CAPACITY` | `websocket_connect_worker_count * 8` clamped to `32..128` | Bounded queue capacity for websocket TCP connect work; effective value is at least the worker count. |
| `runtime_proxy.websocket_connect_overflow_capacity` | `PRODEX_RUNTIME_WEBSOCKET_CONNECT_OVERFLOW_CAPACITY` | `websocket_connect_queue_capacity * 4` clamped to `32..512` | Overflow queue capacity for websocket TCP connect work after the bounded queue fills; `0` disables overflow buffering. |
| `runtime_proxy.websocket_dns_worker_count` | `PRODEX_RUNTIME_WEBSOCKET_DNS_WORKER_COUNT` | CPU parallelism clamped to `2..8` | Worker count for bounded websocket DNS resolution executor. |
| `runtime_proxy.websocket_dns_queue_capacity` | `PRODEX_RUNTIME_WEBSOCKET_DNS_QUEUE_CAPACITY` | `websocket_dns_worker_count * 4` clamped to `16..64` | Bounded queue capacity for websocket DNS resolution work; effective value is at least the worker count. |
| `runtime_proxy.websocket_dns_overflow_capacity` | `PRODEX_RUNTIME_WEBSOCKET_DNS_OVERFLOW_CAPACITY` | `websocket_dns_queue_capacity * 2` clamped to `16..128` | Overflow queue capacity for websocket DNS resolution work after the bounded queue fills; `0` disables overflow buffering. |
| `runtime_proxy.websocket_previous_response_reuse_stale_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS` | `60000` | Window for reusing a websocket previous-response binding before treating it as stale. |
| `runtime_proxy.broker_ready_timeout_ms` | `PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS` | `15000` | Startup wait for the runtime broker to become ready. |
| `runtime_proxy.broker_health_connect_timeout_ms` | `PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS` | `750` | Broker health check connect timeout. |
| `runtime_proxy.broker_health_read_timeout_ms` | `PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS` | `1500` | Broker health check read timeout. |
| `runtime_proxy.sync_probe_pressure_pause_ms` | `PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS` | `5` | Pause before synchronous probe work when local pressure is detected. |
| `runtime_proxy.responses_critical_floor_percent` | `PRODEX_RUNTIME_PROXY_RESPONSES_CRITICAL_FLOOR_PERCENT` | `2` | Minimum remaining Responses quota percentage treated as critical; valid range `1..10`. |
| `runtime_proxy.startup_sync_probe_warm_limit` | `PRODEX_RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT` | `1` | Startup synchronous quota probe warm-up limit, capped internally at `3`. |
<!-- END GENERATED RUNTIME_PROXY_KEYS -->

Positive integer values are required for numeric policy keys, except websocket overflow capacity keys, which may be `0`, and `responses_critical_floor_percent`, which must be between `1` and `10`.
Some effective values are clamped after env or policy resolution to protect runtime bounds.

## Example

```toml
version = 1

[runtime]
log_format = "json"
log_dir = "runtime-logs"

[runtime_proxy]
preset = "many-terminals"
worker_count = 16
active_request_limit = 128
responses_active_limit = 96
profile_inflight_soft_limit = 6
profile_inflight_hard_limit = 10
```
