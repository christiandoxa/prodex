use std::process::Command;

fn bin(name: &str) -> String {
    std::env::var(format!("CARGO_BIN_EXE_{name}")).expect("cargo should expose binary path")
}

#[test]
fn gateway_binary_exposes_dedicated_help_and_version() {
    let help = Command::new(bin("prodex-gateway"))
        .arg("--help")
        .output()
        .expect("run prodex-gateway --help");
    assert!(help.status.success());
    let stdout = String::from_utf8(help.stdout).unwrap();
    assert!(stdout.contains("Data-plane gateway entrypoint"));
    assert!(stdout.contains("composition root"));

    let version = Command::new(bin("prodex-gateway"))
        .arg("--version")
        .output()
        .expect("run prodex-gateway --version");
    assert!(version.status.success());
    assert!(
        String::from_utf8(version.stdout)
            .unwrap()
            .starts_with("prodex-gateway ")
    );
}

#[test]
fn control_plane_binary_exposes_dedicated_help_and_version() {
    let help = Command::new(bin("prodex-control-plane"))
        .arg("--help")
        .output()
        .expect("run prodex-control-plane --help");
    assert!(help.status.success());
    let stdout = String::from_utf8(help.stdout).unwrap();
    assert!(stdout.contains("Control-plane entrypoint"));
    assert!(stdout.contains("composition"));

    let version = Command::new(bin("prodex-control-plane"))
        .arg("--version")
        .output()
        .expect("run prodex-control-plane --version");
    assert!(version.status.success());
    assert!(
        String::from_utf8(version.stdout)
            .unwrap()
            .starts_with("prodex-control-plane ")
    );
}

#[test]
fn enterprise_serve_commands_are_explicitly_gated_until_adapters_are_ready() {
    for name in ["prodex-gateway", "prodex-control-plane"] {
        let output = Command::new(bin(name))
            .arg("serve")
            .output()
            .expect("run gated serve command");
        assert_eq!(output.status.code(), Some(2));
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("not wired yet")
        );
    }
}
