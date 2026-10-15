# App-server argument projection in Mojo

`prodex_runtime_overlay_app_server_config_v1` now owns the deterministic argument projection used by Super hook-trust preflight and the private session app-server companion. Mojo selects paired config and feature flags, model overrides, and hook-only inline config forwarding; Rust keeps `OsString` materialization, TOML quoting, process launch, and filesystem effects. The compiled adapter and app caller tests preserve the two existing argument shapes and reject non-UTF-8 paired model values without a Rust semantic fallback.
