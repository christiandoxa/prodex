use super::{
    OPENAI_TUNNEL_CLIENT_READY_TIMEOUT, OpenAiTunnelFiles, ensure_openai_tunnel_available,
    parse_health_base_url, safe_client_version, safe_version_label, start_openai_tunnel,
    validate_openai_tunnel_id,
};
use crate::{TestEnvVarGuard, test_temp_root, write_test_python_executable};
use std::fs;
use std::path::Path;
use std::process::Output;
use std::time::{Duration, Instant};

const VALID_TUNNEL_ID: &str = "tunnel_0123456789abcdef0123456789abcdef";
const API_KEY_SENTINEL: &str = "runtime-key-sentinel";
const LOCAL_MCP_URL: &str = "http://127.0.0.1:43123/pdx/v1/capability-secret/mcp";

fn test_root(name: &str) -> std::path::PathBuf {
    let root = test_temp_root().join(format!(
        "prodex-openai-tunnel-{name}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("OpenAI tunnel test root should be created");
    root
}

fn fake_tunnel_client(root: &Path) -> std::path::PathBuf {
    write_test_python_executable(
        root,
        "tunnel-client",
        r#"#!/usr/bin/env python3
import http.server
import os
import sys

args = sys.argv[1:]
marker = os.environ.get("PRODEX_OPENAI_TUNNEL_MARKER")

def flag_value(name):
    try:
        return args[args.index(name) + 1]
    except (ValueError, IndexError):
        return ""

if "--version" in args:
    if marker:
        with open(marker, "w", encoding="utf-8") as file:
            file.write("CONTROL_PLANE_API_KEY_PRESENT=" + str("CONTROL_PLANE_API_KEY" in os.environ) + "\n")
    print("tunnel-client 0.0.13", flush=True)
    raise SystemExit(0)

config = flag_value("--config")
if marker:
    with open(marker, "w", encoding="utf-8") as file:
        file.write("argv=" + repr(args) + "\n")
        file.write("config=" + open(config, encoding="utf-8").read() + "\n")
        for name in [
            "CONTROL_PLANE_API_KEY",
            "OPENAI_API_KEY",
            "OPENAI_ADMIN_KEY",
            "MCP_SERVER_URL",
            "MCP_COMMAND",
            "CLOUDFLARED_MANAGED",
            "CLOUDFLARED_TUNNEL_TOKEN",
        ]:
            file.write(name + "_PRESENT=" + str(name in os.environ) + "\n")

if os.environ.get("PRODEX_OPENAI_TUNNEL_MODE") == "exit":
    raise SystemExit(17)

class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        status = 503 if os.environ.get("PRODEX_OPENAI_TUNNEL_MODE") == "not-ready" and self.path == "/readyz" else 200
        self.send_response(status)
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, format, *args):
        pass

server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
with open(flag_value("--health.url-file"), "w", encoding="utf-8") as file:
    file.write("http://127.0.0.1:" + str(server.server_port))
server.serve_forever()
"#,
    )
}

#[test]
fn validates_official_tunnel_id_shape() {
    assert!(validate_openai_tunnel_id(VALID_TUNNEL_ID).is_ok());
    assert!(validate_openai_tunnel_id("tunnel_").is_err());
    assert!(validate_openai_tunnel_id("tunnel_0123456789abcdef0123456789ABCDEFFF").is_err());
    assert!(
        validate_openai_tunnel_id("https://example.com/tunnel_0123456789abcdef0123456789abcdef")
            .is_err()
    );
}

#[test]
fn config_keeps_capability_url_outside_argv_and_yaml() {
    let mut files = OpenAiTunnelFiles::create(LOCAL_MCP_URL, VALID_TUNNEL_ID).unwrap();
    let config = fs::read_to_string(&files.config).unwrap();
    assert!(!config.contains(LOCAL_MCP_URL));
    assert!(config.contains("config_version: 1"));
    assert!(config.contains("url: 'file:"));
    assert!(config.contains(&files.mcp_url.display().to_string()));
    files.cleanup();
}

#[test]
fn health_url_parser_accepts_only_loopback_http_with_port() {
    assert_eq!(
        parse_health_base_url("http://127.0.0.1:1234/\n").unwrap(),
        "http://127.0.0.1:1234"
    );
    assert!(parse_health_base_url("https://127.0.0.1:1234").is_err());
    assert!(parse_health_base_url("http://example.com:1234").is_err());
    assert!(parse_health_base_url("http://127.0.0.1:0").is_err());
    assert!(parse_health_base_url("http://127.0.0.1:1234/healthz").is_err());
}

#[test]
fn missing_runtime_key_fails_without_echoing_a_secret() {
    let _env_lock = TestEnvVarGuard::lock();
    let _key = TestEnvVarGuard::unset("CONTROL_PLANE_API_KEY");
    let error = ensure_openai_tunnel_available(VALID_TUNNEL_ID).unwrap_err();
    assert!(error.to_string().contains("CONTROL_PLANE_API_KEY"));
    assert!(!error.to_string().contains(API_KEY_SENTINEL));
}

#[test]
fn missing_binary_fails_with_install_guidance_without_echoing_a_secret() {
    let _env_lock = TestEnvVarGuard::lock();
    let _key = TestEnvVarGuard::set("CONTROL_PLANE_API_KEY", API_KEY_SENTINEL);
    let _binary = TestEnvVarGuard::set(
        "PRODEX_TUNNEL_CLIENT_BIN",
        "/home/test-user/missing/tunnel-client",
    );
    let error = ensure_openai_tunnel_available(VALID_TUNNEL_ID).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("openai/tunnel-client v0.0.13"));
    assert!(message.contains("PRODEX_TUNNEL_CLIENT_BIN"));
    assert!(!message.contains(API_KEY_SENTINEL));
}

#[test]
fn version_probe_does_not_pass_runtime_key_to_an_untrusted_binary() {
    let root = test_root("version");
    let marker = root.join("marker.txt");
    let _env_lock = TestEnvVarGuard::lock();
    let binary = fake_tunnel_client(&root);
    let _binary = TestEnvVarGuard::set("PRODEX_TUNNEL_CLIENT_BIN", &binary.to_string_lossy());
    let _key = TestEnvVarGuard::set("CONTROL_PLANE_API_KEY", API_KEY_SENTINEL);
    let _marker = TestEnvVarGuard::set("PRODEX_OPENAI_TUNNEL_MARKER", &marker.to_string_lossy());
    let version = ensure_openai_tunnel_available(VALID_TUNNEL_ID).unwrap();
    assert_eq!(version, "0.0.13");
    assert_eq!(
        fs::read_to_string(&marker).unwrap(),
        "CONTROL_PLANE_API_KEY_PRESENT=False\n"
    );
    assert!(
        !fs::read_to_string(&marker)
            .unwrap()
            .contains(API_KEY_SENTINEL)
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fake_client_reaches_local_health_ready_and_keeps_secrets_out_of_argv_and_config() {
    let root = test_root("ready");
    let marker = root.join("marker.txt");
    let _env_lock = TestEnvVarGuard::lock();
    let binary = fake_tunnel_client(&root);
    let _binary = TestEnvVarGuard::set("PRODEX_TUNNEL_CLIENT_BIN", &binary.to_string_lossy());
    let _key = TestEnvVarGuard::set("CONTROL_PLANE_API_KEY", API_KEY_SENTINEL);
    let _marker = TestEnvVarGuard::set("PRODEX_OPENAI_TUNNEL_MARKER", &marker.to_string_lossy());
    let mut tunnel = start_openai_tunnel(
        LOCAL_MCP_URL,
        VALID_TUNNEL_ID.to_string(),
        "0.0.13".to_string(),
        &|| false,
    )
    .unwrap();
    assert_eq!(tunnel.status.tunnel_id, VALID_TUNNEL_ID);
    assert_eq!(tunnel.status.client_version, "0.0.13");
    let marker_contents = fs::read_to_string(&marker).unwrap();
    assert!(!marker_contents.contains(LOCAL_MCP_URL));
    assert!(!marker_contents.contains(API_KEY_SENTINEL));
    assert!(marker_contents.contains("--control-plane.api-key"));
    assert!(marker_contents.contains("env:CONTROL_PLANE_API_KEY"));
    assert!(marker_contents.contains("CONTROL_PLANE_API_KEY_PRESENT=True"));
    assert!(marker_contents.contains("OPENAI_API_KEY_PRESENT=False"));
    assert!(marker_contents.contains("OPENAI_ADMIN_KEY_PRESENT=False"));
    assert!(marker_contents.contains("MCP_SERVER_URL_PRESENT=False"));
    assert!(marker_contents.contains("CLOUDFLARED_MANAGED_PRESENT=False"));
    assert!(marker_contents.contains("CLOUDFLARED_TUNNEL_TOKEN_PRESENT=False"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&tunnel.files.directory)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&tunnel.files.config)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let directory = tunnel.files.directory.clone();
    tunnel.shutdown();
    assert!(tunnel.exited().is_some());
    assert!(!directory.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn child_exit_before_ready_is_reported_and_cleaned_up() {
    let root = test_root("exit");
    let marker = root.join("marker.txt");
    let _env_lock = TestEnvVarGuard::lock();
    let binary = fake_tunnel_client(&root);
    let _binary = TestEnvVarGuard::set("PRODEX_TUNNEL_CLIENT_BIN", &binary.to_string_lossy());
    let _key = TestEnvVarGuard::set("CONTROL_PLANE_API_KEY", API_KEY_SENTINEL);
    let _marker = TestEnvVarGuard::set("PRODEX_OPENAI_TUNNEL_MARKER", &marker.to_string_lossy());
    let _mode = TestEnvVarGuard::set("PRODEX_OPENAI_TUNNEL_MODE", "exit");
    let error = match start_openai_tunnel(
        LOCAL_MCP_URL,
        VALID_TUNNEL_ID.to_string(),
        "0.0.13".to_string(),
        &|| false,
    ) {
        Ok(_) => panic!("fake tunnel-client should exit before readiness"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("exited before local readiness"));
    assert!(!error.to_string().contains(API_KEY_SENTINEL));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn readiness_timeout_is_bounded_and_cleaned_up() {
    let root = test_root("not-ready");
    let _env_lock = TestEnvVarGuard::lock();
    let binary = fake_tunnel_client(&root);
    let _binary = TestEnvVarGuard::set("PRODEX_TUNNEL_CLIENT_BIN", &binary.to_string_lossy());
    let _key = TestEnvVarGuard::set("CONTROL_PLANE_API_KEY", API_KEY_SENTINEL);
    let _mode = TestEnvVarGuard::set("PRODEX_OPENAI_TUNNEL_MODE", "not-ready");
    let started = Instant::now();
    let error = match start_openai_tunnel(
        LOCAL_MCP_URL,
        VALID_TUNNEL_ID.to_string(),
        "0.0.13".to_string(),
        &|| false,
    ) {
        Ok(_) => panic!("not-ready tunnel-client should time out"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("did not become locally ready"));
    assert!(started.elapsed() < OPENAI_TUNNEL_CLIENT_READY_TIMEOUT + Duration::from_secs(2));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancellation_is_bounded_before_readiness() {
    let root = test_root("cancel");
    let _env_lock = TestEnvVarGuard::lock();
    let binary = fake_tunnel_client(&root);
    let _binary = TestEnvVarGuard::set("PRODEX_TUNNEL_CLIENT_BIN", &binary.to_string_lossy());
    let _key = TestEnvVarGuard::set("CONTROL_PLANE_API_KEY", API_KEY_SENTINEL);
    let error = match start_openai_tunnel(
        LOCAL_MCP_URL,
        VALID_TUNNEL_ID.to_string(),
        "0.0.13".to_string(),
        &|| true,
    ) {
        Ok(_) => panic!("cancelled tunnel-client should not become ready"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("startup cancelled"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn malformed_version_output_is_not_used_as_status() {
    let output = Output {
        status: std::process::ExitStatus::default(),
        stdout: b"runtime-key-sentinel\n".to_vec(),
        stderr: Vec::new(),
    };
    assert_eq!(safe_client_version(&output), None);
    assert_eq!(safe_version_label("runtime-key-sentinel"), "unknown");
}
