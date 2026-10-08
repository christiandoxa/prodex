#![cfg(feature = "mojo-runtime")]

use prodex_mojo_core::runtime_broker_continuity::{
    BrokerRegistryArtifactStatus::{Malformed, NotLegacy, TooLarge, ValidLegacy},
    BrokerRegistryErrorSource::{Backup, None, Primary},
    BrokerRegistryStoreAction::{ReadCurrent, RemoveLegacyArtifacts, RemoveLegacyBackup},
    BrokerRegistryStorePlan, registry_instance_matches, registry_store_plan,
};

#[test]
fn registry_store_policy_preserves_recovery_precedence() {
    let cases = [
        (true, NotLegacy, ValidLegacy, true, RemoveLegacyBackup, None),
        (true, NotLegacy, ValidLegacy, false, ReadCurrent, None),
        (
            true,
            ValidLegacy,
            ValidLegacy,
            true,
            RemoveLegacyArtifacts,
            None,
        ),
        (
            false,
            NotLegacy,
            ValidLegacy,
            false,
            RemoveLegacyArtifacts,
            None,
        ),
        (true, TooLarge, TooLarge, false, ReadCurrent, Primary),
        (true, Malformed, TooLarge, false, ReadCurrent, Backup),
    ];
    for (primary_exists, primary, backup, current, action, error_source) in cases {
        assert_eq!(
            registry_store_plan(primary_exists, primary, backup, current).unwrap(),
            BrokerRegistryStorePlan {
                action,
                error_source,
            }
        );
    }
    assert!(registry_instance_matches("instance-α", "instance-α").unwrap());
    assert!(!registry_instance_matches("Instance", "instance").unwrap());
}
