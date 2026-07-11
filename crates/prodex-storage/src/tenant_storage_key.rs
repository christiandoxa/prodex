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
}
