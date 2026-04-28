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

## Runtime Proxy Keys

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
| `runtime_proxy.sse_lookahead_timeout_ms` | `PRODEX_RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS` | `1000` | Pre-commit SSE lookahead timeout. |
| `runtime_proxy.prefetch_backpressure_retry_ms` | `PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS` | `10` | Retry delay while stream prefetch is backpressured. |
| `runtime_proxy.prefetch_backpressure_timeout_ms` | `PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS` | `1000` | Max wait for stream prefetch backpressure to clear. |
| `runtime_proxy.prefetch_max_buffered_bytes` | `PRODEX_RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES` | `786432` | Max buffered prefetch bytes before backpressure. |
| `runtime_proxy.websocket_connect_timeout_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS` | `15000` | Upstream websocket connect timeout. |
| `runtime_proxy.websocket_happy_eyeballs_delay_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS` | `200` | Delay before alternate websocket TCP connect attempt. |
| `runtime_proxy.websocket_precommit_progress_timeout_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS` | `8000` | Websocket pre-commit progress timeout. |
| `runtime_proxy.websocket_previous_response_reuse_stale_ms` | `PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS` | `60000` | Window for reusing a websocket previous-response binding before treating it as stale. |
| `runtime_proxy.broker_ready_timeout_ms` | `PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS` | `15000` | Startup wait for the runtime broker to become ready. |
| `runtime_proxy.broker_health_connect_timeout_ms` | `PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS` | `750` | Broker health check connect timeout. |
| `runtime_proxy.broker_health_read_timeout_ms` | `PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS` | `1500` | Broker health check read timeout. |
| `runtime_proxy.sync_probe_pressure_pause_ms` | `PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS` | `5` | Pause before synchronous probe work when local pressure is detected. |
| `runtime_proxy.responses_critical_floor_percent` | `PRODEX_RUNTIME_PROXY_RESPONSES_CRITICAL_FLOOR_PERCENT` | `2` | Minimum remaining Responses quota percentage treated as critical; valid range `1..10`. |
| `runtime_proxy.startup_sync_probe_warm_limit` | `PRODEX_RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT` | `1` | Startup synchronous quota probe warm-up limit, capped internally at `3`. |

Positive integer values are required for numeric policy keys, except `responses_critical_floor_percent`, which must be between `1` and `10`.
Some effective values are clamped after env or policy resolution to protect runtime bounds.

## Example

```toml
version = 1

[runtime]
log_format = "json"
log_dir = "runtime-logs"

[runtime_proxy]
worker_count = 16
active_request_limit = 128
responses_active_limit = 96
profile_inflight_soft_limit = 6
profile_inflight_hard_limit = 10
```
