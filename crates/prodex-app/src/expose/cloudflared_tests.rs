use super::runtime::{
    CloudflaredTransport, expose_find_trycloudflare_url, expose_scan_cloudflared_output,
    start_cloudflared_tunnel,
};
use super::super_expose::ensure_cloudflared_available;
use crate::{TestEnvVarGuard, test_temp_root, write_test_python_executable};
use std::fs;
use std::io::{self, Read};
use std::net::{Shutdown, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
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
fn quick_tunnel_http2_does_not_become_ready_when_public_dns_and_doh_reset() {
    let root = test_temp_root().join(format!(
        "prodex-cloudflared-public-dns-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    write_test_python_executable(
        &root,
        "cloudflared",
        r#"#!/usr/bin/env python3
import sys
import time

if len(sys.argv) > 1 and sys.argv[1] == "--version":
    print("cloudflared version 2026.6.0", flush=True)
elif len(sys.argv) > 2 and sys.argv[1] == "tunnel" and sys.argv[2] == "--help":
    print("--config --protocol --url", flush=True)
else:
    protocol = sys.argv[sys.argv.index("--protocol") + 1]
    print("https://http2.trycloudflare.com", flush=True)
    if protocol == "auto":
        print("Failed to dial a quic connection", file=sys.stderr, flush=True)
    else:
        print("Registered tunnel connection protocol=http2", flush=True)
    time.sleep(30)
"#,
    );

    let _env_lock = TestEnvVarGuard::lock();
    let _script = expose_test_cloudflared_script(&root);
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

    let proxy_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_addr = proxy_listener.local_addr().unwrap();
    let proxy_failed = Arc::new(AtomicBool::new(false));
    let proxy_failed_thread = Arc::clone(&proxy_failed);
    let (request_sender, request_receiver) = mpsc::channel();
    let proxy_thread = thread::spawn(move || {
        proxy_listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let request = loop {
            match proxy_listener.accept() {
                Ok((mut stream, _)) => {
                    proxy_failed_thread.store(true, Ordering::SeqCst);
                    let mut request = [0_u8; 1024];
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                    let size = stream.read(&mut request).unwrap_or(0);
                    let _ = stream.shutdown(Shutdown::Both);
                    break request[..size].to_vec();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        proxy_failed_thread.store(true, Ordering::SeqCst);
                        break Vec::new();
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    proxy_failed_thread.store(true, Ordering::SeqCst);
                    break Vec::new();
                }
            }
        };
        let _ = request_sender.send(request);
    });
    let proxy = format!("http://{proxy_addr}");
    let _https_proxy = TestEnvVarGuard::set("HTTPS_PROXY", &proxy);
    let _no_proxy = TestEnvVarGuard::set("NO_PROXY", "");

    let public_url = "https://w4-dns-unavailable.invalid.trycloudflare.com/pdx/v1/fixture/mcp";
    let mut progress = |_phase: &str| {};
    let started = Instant::now();
    let error = super::verify_public_mcp_with_progress(public_url, &mut progress, &|| {
        proxy_failed.load(Ordering::SeqCst)
    })
    .unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(4));
    assert!(
        error
            .to_string()
            .contains("public MCP initialize cancelled")
    );
    let request = request_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap();
    assert!(
        String::from_utf8_lossy(&request).contains("CONNECT cloudflare-dns.com:443"),
        "public probe should try Cloudflare DoH after hostname resolution fails: {:?}",
        String::from_utf8_lossy(&request)
    );

    proxy_thread.join().unwrap();
    tunnel.shutdown();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn quick_tunnel_child_uses_private_home_and_explicit_auto_protocol() {
    let root = test_temp_root().join(format!(
        "prodex-cloudflared-config-boundary-{}",
        std::process::id()
    ));
    let user_home = root.join("user-home");
    fs::create_dir_all(user_home.join(".cloudflared")).unwrap();
    let user_config = user_home.join(".cloudflared/config.yaml");
    fs::write(
        &user_config,
        "tunnel: user-named-tunnel\nprotocol: quic\ncredentials-file: user.json\n",
    )
    .unwrap();
    let args_marker = root.join("child-args.txt");
    fs::create_dir_all(&root).unwrap();
    write_test_python_executable(
        &root,
        "cloudflared",
        r#"#!/usr/bin/env python3
import os
import sys

if len(sys.argv) > 1 and sys.argv[1] == "--version":
    print("cloudflared version 2026.8.2", flush=True)
elif len(sys.argv) > 2 and sys.argv[1] == "tunnel" and sys.argv[2] == "--help":
    print("--config --protocol --url", flush=True)
else:
    config = sys.argv[sys.argv.index("--config") + 1]
    origin = sys.argv[sys.argv.index("--url") + 1]
    protocol = sys.argv[sys.argv.index("--protocol") + 1]
    with open(os.environ["PRODEX_TEST_CLOUDFLARED_ARGS"], "w", encoding="utf-8") as file:
        file.write("protocol=" + protocol + "\n")
        file.write("origin=" + origin + "\n")
        file.write("config=" + config + "\n")
        file.write("home=" + os.environ.get("HOME", "") + "\n")
        file.write("userprofile=" + os.environ.get("USERPROFILE", "") + "\n")
        file.write("config_contents=" + open(config, encoding="utf-8").read() + "\n")
    print("https://isolated.trycloudflare.com", file=sys.stderr, flush=True)
    print("Registered tunnel connection protocol=quic", file=sys.stderr, flush=True)
    import time
    time.sleep(30)
"#,
    );
    let _env_lock = TestEnvVarGuard::lock();
    let (_home, _userprofile) = TestEnvVarGuard::set_home(&user_home);
    let _script = expose_test_cloudflared_script(&root);
    let _marker = TestEnvVarGuard::set(
        "PRODEX_TEST_CLOUDFLARED_ARGS",
        &args_marker.to_string_lossy(),
    );
    let _forced = TestEnvVarGuard::set("TUNNEL_TRANSPORT_PROTOCOL", "quic");
    let path = std::env::join_paths(std::iter::once(root.clone()).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .unwrap();
    let _path = TestEnvVarGuard::set("PATH", &path.to_string_lossy());

    let mut tunnel = start_cloudflared_tunnel("http://127.0.0.1:43123").unwrap();
    let marker = fs::read_to_string(&args_marker)
        .unwrap()
        .replace("\r\n", "\n");
    assert!(marker.contains("protocol=auto\n"));
    assert!(marker.contains("origin=http://127.0.0.1:43123\n"));
    assert!(marker.contains("home="));
    assert!(marker.contains("userprofile="));
    assert!(marker.contains("config_contents=\n"));
    assert_eq!(
        fs::read_to_string(&user_config).unwrap(),
        "tunnel: user-named-tunnel\nprotocol: quic\ncredentials-file: user.json\n"
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
