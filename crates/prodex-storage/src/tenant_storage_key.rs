use prodex_domain::{TenantId, VirtualKeyId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BudgetStorageScope([u8; 32]);

impl BudgetStorageScope {
    pub fn from_digest(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    fn encoded(self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = String::with_capacity(64);
        for byte in self.0 {
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
        encoded
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TenantStorageKey {
    pub tenant_id: TenantId,
    pub virtual_key_id: Option<VirtualKeyId>,
    pub budget_scope: Option<BudgetStorageScope>,
}

impl TenantStorageKey {
    pub fn tenant(tenant_id: TenantId) -> Self {
        Self {
            tenant_id,
            virtual_key_id: None,
            budget_scope: None,
        }
    }

    pub fn virtual_key(tenant_id: TenantId, virtual_key_id: VirtualKeyId) -> Self {
        Self {
            tenant_id,
            virtual_key_id: Some(virtual_key_id),
            budget_scope: None,
        }
    }

    pub fn budget_group(
        tenant_id: TenantId,
        virtual_key_id: VirtualKeyId,
        budget_scope: BudgetStorageScope,
    ) -> Self {
        Self {
            tenant_id,
            virtual_key_id: Some(virtual_key_id),
            budget_scope: Some(budget_scope),
        }
    }

    pub fn storage_scope(self) -> String {
        if let Some(scope) = self.budget_scope {
            return format!("budget_group:{}", scope.encoded());
        }
        self.virtual_key_id
            .map(|virtual_key_id| format!("virtual_key:{virtual_key_id}"))
            .unwrap_or_else(|| "tenant-default".to_string())
    }

    pub fn from_storage_scope(
        tenant_id: TenantId,
        virtual_key_id: Option<VirtualKeyId>,
        storage_scope: &str,
    ) -> Option<Self> {
        if storage_scope == "tenant-default" {
            return virtual_key_id.is_none().then(|| Self::tenant(tenant_id));
        }
        if let Some(value) = storage_scope.strip_prefix("virtual_key:") {
            let parsed = value.parse::<VirtualKeyId>().ok()?;
            return (virtual_key_id == Some(parsed)).then(|| Self::virtual_key(tenant_id, parsed));
        }
        let encoded = storage_scope.strip_prefix("budget_group:")?;
        if encoded.len() != 64 {
            return None;
        }
        let mut digest = [0_u8; 32];
        for (index, chunk) in encoded.as_bytes().chunks_exact(2).enumerate() {
            digest[index] = hex_nibble(chunk[0])?.checked_mul(16)? + hex_nibble(chunk[1])?;
        }
        Some(Self::budget_group(
            tenant_id,
            virtual_key_id?,
            BudgetStorageScope::from_digest(digest),
        ))
    }
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_scope_round_trips_all_supported_keys() {
        let tenant_id = TenantId::new();
        let virtual_key_id = VirtualKeyId::new();
        let keys = [
            TenantStorageKey::tenant(tenant_id),
            TenantStorageKey::virtual_key(tenant_id, virtual_key_id),
            TenantStorageKey::budget_group(
                tenant_id,
                virtual_key_id,
                BudgetStorageScope::from_digest([0xab; 32]),
            ),
        ];
        for key in keys {
            assert_eq!(
                TenantStorageKey::from_storage_scope(
                    tenant_id,
                    key.virtual_key_id,
                    &key.storage_scope(),
                ),
                Some(key)
            );
        }
        assert!(TenantStorageKey::from_storage_scope(tenant_id, None, "budget_group:00").is_none());
    }
}
