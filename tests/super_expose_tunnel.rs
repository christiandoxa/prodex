#[cfg(unix)]
use std::fs;
#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::net::TcpListener;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::process::Command;
#[cfg(target_os = "linux")]
use std::process::Stdio;
#[cfg(target_os = "linux")]
use std::sync::mpsc;
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(unix)]
#[path = "auto_rotate/support/temp_dir.rs"]
mod temp_dir;
#[cfg(unix)]
use temp_dir::TestDir;

#[cfg(unix)]
const TUNNEL_ID: &str = "tunnel_0123456789abcdef0123456789abcdef";
#[cfg(unix)]
const LATEST_VERSION_LINE: &str = "0.0.14+0f870e50a973fa820d4c409000059e181e8d242b (git sha: 0f870e50a973fa820d4c409000059e181e8d242b)";

#[cfg(unix)]
fn fake_tunnel_client(root: &TestDir, version_line: &str) -> std::path::PathBuf {
    let path = root.path.join("tunnel-client");
    fs::write(
        &path,
        format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '{}'
  exit 0
fi
exit 97
"#,
            version_line
        ),
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[cfg(target_os = "linux")]
fn long_lived_fake_tunnel_client(
    root: &TestDir,
    version_line: &str,
    health_base_url: &str,
) -> std::path::PathBuf {
    let path = root.path.join("tunnel-client");
    fs::write(
        &path,
        format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '{}'
  exit 0
fi
health_url_file=
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--health.url-file" ]; then
    shift
    health_url_file="$1"
    break
  fi
  shift
done
if [ -z "$health_url_file" ]; then
  exit 98
fi
printf '%s\n' '{}' > "$health_url_file"
exec sleep 30
"#,
            version_line, health_base_url
        ),
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[cfg(unix)]
fn probe_expose(version_line: &str) -> std::process::Output {
    let root = TestDir::new();
    let tunnel_client = fake_tunnel_client(&root, version_line);
    Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args([
            "s",
            "expose",
            "exec",
            "--openai-tunnel-id",
            TUNNEL_ID,
            "--listen",
            "127.0.0.1:0",
            "--no-presidio",
        ])
        .env("PRODEX_HOME", root.path.join("prodex-home"))
        .env("PRODEX_TUNNEL_CLIENT_BIN", tunnel_client)
        .env_remove("CONTROL_PLANE_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .output()
        .expect("prodex expose exec should run")
}

#[cfg(target_os = "linux")]
fn start_health_server() -> (String, mpsc::Sender<()>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let (stop_tx, stop_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut request = [0_u8; 1024];
                    let _ = stream.read(&mut request);
                    let body = b"ready\n";
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .unwrap();
                    stream.write_all(body).unwrap();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("fake tunnel health server failed: {error}"),
            }
        }
    });
    (format!("http://{address}"), stop_tx, handle)
}

#[cfg(unix)]
#[test]
fn expose_exec_accepts_latest_and_v0013_tunnel_client_metadata() {
    for version_line in [
        LATEST_VERSION_LINE,
        "0.0.13+4b5267f823be0b046bb883aacb51603cfde3a0ea (git sha: 4b5267f823be0b046bb883aacb51603cfde3a0ea)",
    ] {
        let output = probe_expose(version_line);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(
            stderr.contains("requires CONTROL_PLANE_API_KEY in noninteractive mode"),
            "supported tunnel-client metadata was rejected: {stderr}"
        );
        assert!(
            !stderr.contains("did not report a supported official version"),
            "supported tunnel-client metadata was rejected: {stderr}"
        );
    }
}

#[cfg(unix)]
#[test]
fn expose_exec_rejects_unvetted_tunnel_client_metadata() {
    let output = probe_expose(
        "0.0.13+0000000000000000000000000000000000000000 (git sha: 0000000000000000000000000000000000000000)",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("did not report a supported official version"),
        "unexpected stderr: {stderr}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn expose_exec_keeps_ready_tunnel_client_alive_after_startup_handoff() {
    let root = TestDir::new();
    let (health_base_url, stop_health, health_thread) = start_health_server();
    let tunnel_client = long_lived_fake_tunnel_client(&root, LATEST_VERSION_LINE, &health_base_url);
    let stderr_path = root.path.join("stderr.log");
    let stdout_path = root.path.join("stdout.log");
    let stderr_file = File::create(&stderr_path).unwrap();
    let stdout_file = File::create(&stdout_path).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .args([
            "s",
            "expose",
            "exec",
            "--openai-tunnel-id",
            TUNNEL_ID,
            "--listen",
            "127.0.0.1:0",
            "--no-presidio",
        ])
        .env("PRODEX_HOME", root.path.join("prodex-home"))
        .env("PRODEX_TUNNEL_CLIENT_BIN", tunnel_client)
        .env("CONTROL_PLANE_API_KEY", "test-runtime-key")
        .env_remove("OPENAI_API_KEY")
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .expect("prodex expose exec should start");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stderr = String::new();
    while Instant::now() < deadline {
        stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
        if stderr.contains("OpenAI Secure MCP Tunnel ready:") {
            break;
        }
        if let Some(status) = child.try_wait().unwrap() {
            let _ = stop_health.send(());
            let _ = health_thread.join();
            panic!("prodex exited before tunnel readiness ({status}): {stderr}");
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        stderr.contains("OpenAI Secure MCP Tunnel ready:"),
        "tunnel never became ready: {stderr}"
    );

    thread::sleep(Duration::from_millis(400));
    let premature_status = child.try_wait().unwrap();
    let final_stderr = fs::read_to_string(&stderr_path).unwrap_or_default();

    if premature_status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = stop_health.send(());
    health_thread.join().unwrap();

    assert!(
        premature_status.is_none(),
        "tunnel-client died during startup handoff: status={premature_status:?}, stderr={final_stderr}"
    );
    assert!(
        !final_stderr.contains("OpenAI tunnel-client exited unexpectedly"),
        "unexpected tunnel lifecycle failure: {final_stderr}"
    );
}
