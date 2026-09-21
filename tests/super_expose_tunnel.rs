#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::process::Command;

#[cfg(unix)]
#[path = "auto_rotate/support/temp_dir.rs"]
mod temp_dir;
#[cfg(unix)]
use temp_dir::TestDir;

#[cfg(unix)]
const TUNNEL_ID: &str = "tunnel_0123456789abcdef0123456789abcdef";

#[cfg(unix)]
fn fake_tunnel_client(root: &TestDir, version_line: &str) -> std::path::PathBuf {
    let path = root.path.join("tunnel-client");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexit 97\n",
            version_line
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

#[cfg(unix)]
#[test]
fn expose_exec_accepts_latest_and_v0013_tunnel_client_metadata() {
    for version_line in [
        "0.0.14+0f870e50a973fa820d4c409000059e181e8d242b (git sha: 0f870e50a973fa820d4c409000059e181e8d242b)",
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
