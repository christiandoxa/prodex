use super::{
    RICH_ABI_VERSION, RichStringView, ensure_rich_abi, mojo_mut_pointer_address,
    mojo_pointer_address, status_error, view,
};
use crate::MojoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayAdminResourceRoute<'a> {
    Create,
    List,
    Validate,
    Status,
    Get {
        revision_id: &'a str,
    },
    Submit {
        revision_id: &'a str,
    },
    Vote {
        revision_id: &'a str,
        approval_id: &'a str,
    },
    Activate {
        revision_id: &'a str,
        action: &'a str,
    },
    NotFound,
    MethodNotAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayAdminPolicyRoute {
    None,
    Resource { resource_code: u8 },
    AuditExport,
    AuditIntegrity,
    Outbox,
    AuditRetention,
    ExecutionApprovals,
    BreakGlassApprovals,
}

unsafe extern "C" {
    fn prodex_mojo_gateway_admin_policy_route_v1(
        abi_version: i64,
        path: u64,
        admin_prefix: u64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_gateway_admin_resource_route_v1(
        abi_version: i64,
        method: u64,
        segments: u64,
        segment_count: i64,
        output: u64,
    ) -> i64;
}

pub fn plan_gateway_admin_policy_route(
    path: &str,
    admin_prefix: &str,
) -> Result<GatewayAdminPolicyRoute, MojoError> {
    ensure_rich_abi()?;
    let path = view(path);
    let admin_prefix = view(admin_prefix);
    let mut output = [-1_i64; 2];
    let status = unsafe {
        prodex_mojo_gateway_admin_policy_route_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&path),
            mojo_pointer_address(&admin_prefix),
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(status_error(status, 6, 0, 0, 0));
    }
    match output {
        [0, -1] => Ok(GatewayAdminPolicyRoute::None),
        [1, resource @ 0..=3] => Ok(GatewayAdminPolicyRoute::Resource {
            resource_code: resource as u8,
        }),
        [2, -1] => Ok(GatewayAdminPolicyRoute::AuditExport),
        [3, -1] => Ok(GatewayAdminPolicyRoute::AuditIntegrity),
        [4, -1] => Ok(GatewayAdminPolicyRoute::Outbox),
        [5, -1] => Ok(GatewayAdminPolicyRoute::AuditRetention),
        [6, -1] => Ok(GatewayAdminPolicyRoute::ExecutionApprovals),
        [7, -1] => Ok(GatewayAdminPolicyRoute::BreakGlassApprovals),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn plan_gateway_admin_resource_route<'a>(
    method: &str,
    segments: &'a [&'a str],
) -> Result<GatewayAdminResourceRoute<'a>, MojoError> {
    ensure_rich_abi()?;
    let method = view(method);
    let segment_views = segments
        .iter()
        .map(|segment| view(segment))
        .collect::<Vec<RichStringView>>();
    let mut output = [-1_i64; 4];
    let status = unsafe {
        prodex_mojo_gateway_admin_resource_route_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&method),
            if segment_views.is_empty() {
                0
            } else {
                mojo_pointer_address(segment_views.as_ptr())
            },
            i64::try_from(segment_views.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(status_error(status, 6, 0, 0, 0));
    }
    let segment = |index: i64| {
        usize::try_from(index)
            .ok()
            .and_then(|index| segments.get(index))
            .copied()
            .ok_or(MojoError::InvalidOutput)
    };
    match output[0] {
        0 => Ok(GatewayAdminResourceRoute::Create),
        1 => Ok(GatewayAdminResourceRoute::List),
        2 => Ok(GatewayAdminResourceRoute::Validate),
        3 => Ok(GatewayAdminResourceRoute::Status),
        4 => Ok(GatewayAdminResourceRoute::Get {
            revision_id: segment(output[1])?,
        }),
        5 => Ok(GatewayAdminResourceRoute::Submit {
            revision_id: segment(output[1])?,
        }),
        6 => Ok(GatewayAdminResourceRoute::Vote {
            revision_id: segment(output[1])?,
            approval_id: segment(output[2])?,
        }),
        7 => Ok(GatewayAdminResourceRoute::Activate {
            revision_id: segment(output[1])?,
            action: segment(output[2])?,
        }),
        8 => Ok(GatewayAdminResourceRoute::NotFound),
        9 => Ok(GatewayAdminResourceRoute::MethodNotAllowed),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_admin_resource_route_classifies_complete_route_family() {
        assert_eq!(
            plan_gateway_admin_resource_route("POST", &["rev-1", "activate"]),
            Ok(GatewayAdminResourceRoute::Activate {
                revision_id: "rev-1",
                action: "activate",
            })
        );
        assert_eq!(
            plan_gateway_admin_resource_route(
                "POST",
                &["rev-1", "approvals", "approval-1", "votes"]
            ),
            Ok(GatewayAdminResourceRoute::Vote {
                revision_id: "rev-1",
                approval_id: "approval-1",
            })
        );
        assert_eq!(
            plan_gateway_admin_resource_route("DELETE", &[]),
            Ok(GatewayAdminResourceRoute::MethodNotAllowed)
        );
    }

    #[test]
    fn gateway_admin_policy_route_classifies_special_and_resource_families() {
        assert_eq!(
            plan_gateway_admin_policy_route("/admin/governance/outbox/claim", "/admin"),
            Ok(GatewayAdminPolicyRoute::Outbox)
        );
        assert_eq!(
            plan_gateway_admin_policy_route("/admin/provider-registries/rev-1", "/admin"),
            Ok(GatewayAdminPolicyRoute::Resource { resource_code: 2 })
        );
        assert_eq!(
            plan_gateway_admin_policy_route("/admin/unrelated", "/admin"),
            Ok(GatewayAdminPolicyRoute::None)
        );
    }
}
