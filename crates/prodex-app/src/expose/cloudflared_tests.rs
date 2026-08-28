use super::runtime::{
    CloudflaredTransport, expose_find_trycloudflare_url, expose_scan_cloudflared_output,
    start_cloudflared_tunnel,
};
use super::super_expose::ensure_cloudflared_available;
use crate::{TestEnvVarGuard, test_temp_root, write_test_python_executable};
use std::fs;
use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn expose_test_cloudflared_script(root: &std::path::Path) -> TestEnvVarGuard {
    let script = root.join(if cfg!(windows) {
        "cloudflared.py"
    } else {
        "cloudflared"
    });
    TestEnvVarGuard::set("PRODEX_TEST_CLOUDFLARED_SCRIPT", &script.to_string_lossy())
}

#[test]
fn cloudflared_reader_accepts_bounded_split_lines_and_flushes_eof() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader = io::Cursor::new(
        b"noise https://not-cloudflare.example\npartial https://split.trycloudflare.com".to_vec(),
    );
    let thread = expose_scan_cloudflared_output(reader, sender);
    thread.join().unwrap();
    assert_eq!(receiver.recv().unwrap(), "https://split.trycloudflare.com");
    assert!(receiver.try_recv().is_err());
}

#[test]
fn fake_cloudflared_is_version_checked_and_terminated_as_one_child() {
    let root = test_temp_root().join(format!("prodex-cloudflared-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let executable = write_test_python_executable(
        &root,
        "cloudflared",
        r#"#!/usr/bin/env python3
import sys
import time

if len(sys.argv) > 1 and sys.argv[1] == "--version":
    print("cloudflared version 2026.1", flush=True)
    raise SystemExit(0)
if len(sys.argv) > 2 and sys.argv[1] == "tunnel" and sys.argv[2] == "--help":
    print("--config --url", flush=True)
    raise SystemExit(0)
print("unrelated https://evil.example", flush=True)
print("https://fixture.trycloudflare.com", file=sys.stderr, flush=True)
print("Registered tunnel connection protocol=quic", file=sys.stderr, flush=True)
time.sleep(30)
"#,
    );
    let _script = expose_test_cloudflared_script(&root);
    let path = std::env::join_paths(std::iter::once(root.clone()).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let _path = TestEnvVarGuard::set("PATH", &path.to_string_lossy());
    ensure_cloudflared_available().unwrap();
    let mut tunnel = start_cloudflared_tunnel("http://127.0.0.1:12345").unwrap();
    assert_eq!(
        tunnel.url.as_deref(),
        Some("https://fixture.trycloudflare.com")
    );
    tunnel.shutdown();
    assert!(tunnel.child.try_wait().unwrap().is_some());
    assert!(!expose_find_trycloudflare_url("https://evil.example").is_some());
    let _ = executable;
}

#[test]
fn quick_tunnel_auto_protocol_falls_back_to_http2_without_inheriting_quic() {
    let root = test_temp_root().join(format!("prodex-cloudflared-auto-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let marker = root.join("args.txt");
    write_test_python_executable(
        &root,
        "cloudflared",
        r#"#!/usr/bin/env python3
import os
import sys
import time

if len(sys.argv) > 1 and sys.argv[1] == "--version":
    print("cloudflared version 2026.6.0", flush=True)
elif len(sys.argv) > 2 and sys.argv[1] == "tunnel" and sys.argv[2] == "--help":
    print("--config --protocol --url", flush=True)
else:
    protocol = sys.argv[sys.argv.index("--protocol") + 1]
    with open(os.environ["PRODEX_TEST_CLOUDFLARED_ARGS"], "a", encoding="utf-8") as file:
        file.write(protocol + ":" + str(os.environ.get("TUNNEL_TRANSPORT_PROTOCOL")) + "\n")
    if protocol == "auto":
        print("https://auto.trycloudflare.com", flush=True)
        print("Failed to dial a quic connection", file=sys.stderr, flush=True)
        time.sleep(30)
    else:
        print("https://http2.trycloudflare.com", flush=True)
        print("Registered tunnel connection protocol=http2", flush=True)
        time.sleep(30)
"#,
    );
    let _env_lock = TestEnvVarGuard::lock();
    let _script = expose_test_cloudflared_script(&root);
    let _args = TestEnvVarGuard::set("PRODEX_TEST_CLOUDFLARED_ARGS", &marker.to_string_lossy());
    let _transport = TestEnvVarGuard::set("TUNNEL_TRANSPORT_PROTOCOL", "quic");
    let path = std::env::join_paths(std::iter::once(root.clone()).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let _path = TestEnvVarGuard::set("PATH", &path.to_string_lossy());

    let mut tunnel = start_cloudflared_tunnel("http://127.0.0.1:12345").unwrap();
    assert_eq!(
        tunnel.effective_transport,
        Some(CloudflaredTransport::Http2)
    );
    let marker_contents = fs::read_to_string(&marker).unwrap().replace("\r\n", "\n");
    assert_eq!(marker_contents, "auto:None\nhttp2:None\n");
    assert_eq!(
        tunnel.url.as_deref(),
        Some("https://http2.trycloudflare.com")
    );
    tunnel.shutdown();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cloudflared_early_exit_is_detected_without_waiting_for_the_full_timeout() {
    let root = test_temp_root().join(format!("prodex-cloudflared-early-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    write_test_python_executable(
        &root,
        "cloudflared",
        r#"#!/usr/bin/env python3
import sys

if len(sys.argv) > 1 and sys.argv[1] == "--version":
    print("cloudflared version 2026.1", flush=True)
elif len(sys.argv) > 2 and sys.argv[1] == "tunnel" and sys.argv[2] == "--help":
    print("--config --url", flush=True)
    raise SystemExit(0)
else:
    print("startup failed", file=sys.stderr, flush=True)
    raise SystemExit(1)
"#,
    );
    let _script = expose_test_cloudflared_script(&root);
    let path = std::env::join_paths(std::iter::once(root.clone()).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let _path = TestEnvVarGuard::set("PATH", &path.to_string_lossy());
    let started = Instant::now();
    let mut tunnel = start_cloudflared_tunnel("http://127.0.0.1:12345").unwrap();
    assert!(tunnel.url.is_none());
    assert!(started.elapsed() < Duration::from_secs(2));
    tunnel.shutdown();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn quick_tunnel_ignores_existing_cloudflared_config_and_cleans_private_config() {
    let root = test_temp_root().join(format!("prodex-cloudflared-config-{}", std::process::id()));
    let home = root.join("home");
    let user_config = home.join(".cloudflared/config.yaml");
    fs::create_dir_all(user_config.parent().unwrap()).unwrap();
    fs::write(
        &user_config,
        "tunnel: named-tunnel\ncredentials-file: secret.json\n",
    )
    .unwrap();
    let script = write_test_python_executable(
        &root,
        "cloudflared",
        r#"#!/usr/bin/env python3
import sys
import time

if len(sys.argv) > 1 and sys.argv[1] == "--version":
    print("cloudflared version 2026.1", flush=True)
elif len(sys.argv) > 2 and sys.argv[1] == "tunnel" and sys.argv[2] == "--help":
    print("--config --url", flush=True)
else:
    print("https://isolated.trycloudflare.com", flush=True)
    print("Registered tunnel connection protocol=http2", flush=True)
    print("https://isolated.trycloudflare.com", file=sys.stderr, flush=True)
    print("Registered tunnel connection protocol=http2", file=sys.stderr, flush=True)
    time.sleep(1)
"#,
    );
    let _env_lock = TestEnvVarGuard::lock();
    #[cfg(windows)]
    let _home = TestEnvVarGuard::set("HOME", &home.to_string_lossy());
    #[cfg(not(windows))]
    let _home = TestEnvVarGuard::set_home(&home);
    let _script = TestEnvVarGuard::set("PRODEX_TEST_CLOUDFLARED_SCRIPT", &script.to_string_lossy());
    let path = std::env::join_paths(std::iter::once(root.clone()).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let _path = TestEnvVarGuard::set("PATH", &path.to_string_lossy());

    let mut tunnel = start_cloudflared_tunnel("http://127.0.0.1:12345").unwrap();
    let isolated_config = tunnel.config_path().to_path_buf();
    assert!(!isolated_config.starts_with(home.join(".cloudflared")));
    assert_eq!(
        tunnel.effective_transport,
        Some(CloudflaredTransport::Http2)
    );
    assert_eq!(
        tunnel.url.as_deref(),
        Some("https://isolated.trycloudflare.com")
    );
    assert!(isolated_config.exists());
    assert_eq!(
        fs::read_to_string(&user_config).unwrap(),
        "tunnel: named-tunnel\ncredentials-file: secret.json\n"
    );
    tunnel.shutdown();
    assert!(!isolated_config.exists());
}
