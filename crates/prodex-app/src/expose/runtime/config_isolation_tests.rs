use super::CloudflaredConfigIsolation;
use crate::test_temp_root;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cloudflared_config_isolation_does_not_touch_user_config() {
    let root = test_temp_root().join(format!(
        "prodex-cloudflared-config-fixture-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let user_config = root.join("home/.cloudflared/config.yaml");
    fs::create_dir_all(user_config.parent().unwrap()).unwrap();
    fs::write(
        &user_config,
        "tunnel: named-tunnel\ncredentials-file: secret.json\n",
    )
    .unwrap();

    let config = CloudflaredConfigIsolation::create().unwrap();
    let private_config = config.path.clone();
    assert!(!private_config.starts_with(user_config.parent().unwrap()));
    assert!(private_config.is_file());
    assert_eq!(
        fs::read_to_string(&user_config).unwrap(),
        "tunnel: named-tunnel\ncredentials-file: secret.json\n"
    );

    drop(config);
    assert!(!private_config.exists());
    let _ = fs::remove_dir_all(root);
}
