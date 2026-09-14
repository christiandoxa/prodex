#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RuntimeGatewayAdminResourceRoute<'a> {
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
pub(super) enum RuntimeGatewayAdminPolicyRoute {
    None,
    Resource { resource_code: u8 },
    AuditExport,
    AuditIntegrity,
    Outbox,
    AuditRetention,
    ExecutionApprovals,
    BreakGlassApprovals,
}

#[cfg(feature = "mojo-core")]
pub(super) fn runtime_gateway_admin_policy_route(
    path: &str,
    admin_prefix: &str,
) -> RuntimeGatewayAdminPolicyRoute {
    mojo_policy_route(path, admin_prefix)
}

#[cfg(feature = "mojo-core")]
fn mojo_policy_route(path: &str, admin_prefix: &str) -> RuntimeGatewayAdminPolicyRoute {
    use prodex_mojo_core::rich::GatewayAdminPolicyRoute as MojoRoute;
    match prodex_mojo_core::rich::plan_gateway_admin_policy_route(path, admin_prefix) {
        Err(_) => RuntimeGatewayAdminPolicyRoute::None,
        Ok(route) => match route {
            MojoRoute::None => RuntimeGatewayAdminPolicyRoute::None,
            MojoRoute::Resource { resource_code } => {
                RuntimeGatewayAdminPolicyRoute::Resource { resource_code }
            }
            MojoRoute::AuditExport => RuntimeGatewayAdminPolicyRoute::AuditExport,
            MojoRoute::AuditIntegrity => RuntimeGatewayAdminPolicyRoute::AuditIntegrity,
            MojoRoute::Outbox => RuntimeGatewayAdminPolicyRoute::Outbox,
            MojoRoute::AuditRetention => RuntimeGatewayAdminPolicyRoute::AuditRetention,
            MojoRoute::ExecutionApprovals => RuntimeGatewayAdminPolicyRoute::ExecutionApprovals,
            MojoRoute::BreakGlassApprovals => RuntimeGatewayAdminPolicyRoute::BreakGlassApprovals,
        },
    }
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn runtime_gateway_admin_policy_route(
    path: &str,
    admin_prefix: &str,
) -> RuntimeGatewayAdminPolicyRoute {
    rust_policy_route(path, admin_prefix)
}

#[cfg(any(test, not(feature = "mojo-core")))]
fn rust_policy_route(path: &str, admin_prefix: &str) -> RuntimeGatewayAdminPolicyRoute {
    let suffix = path.strip_prefix(admin_prefix).unwrap_or_default();
    if suffix == "/audit/exports" {
        RuntimeGatewayAdminPolicyRoute::AuditExport
    } else if suffix == "/governance/audit/integrity" {
        RuntimeGatewayAdminPolicyRoute::AuditIntegrity
    } else if matches!(suffix, "/governance/outbox" | "/governance/outbox/claim") {
        RuntimeGatewayAdminPolicyRoute::Outbox
    } else if suffix == "/audit/retention/holds"
        || suffix.starts_with("/audit/retention/holds/")
        || suffix == "/audit/retention/purge"
    {
        RuntimeGatewayAdminPolicyRoute::AuditRetention
    } else if suffix == "/execution-approvals" || suffix.starts_with("/execution-approvals/") {
        RuntimeGatewayAdminPolicyRoute::ExecutionApprovals
    } else if suffix == "/break-glass-approvals" || suffix.starts_with("/break-glass-approvals/") {
        RuntimeGatewayAdminPolicyRoute::BreakGlassApprovals
    } else {
        [
            "/policies",
            "/classification-rules",
            "/provider-registries",
            "/routing-scores",
        ]
        .into_iter()
        .position(|prefix| suffix == prefix || suffix.starts_with(&format!("{prefix}/")))
        .map_or(RuntimeGatewayAdminPolicyRoute::None, |resource_code| {
            RuntimeGatewayAdminPolicyRoute::Resource {
                resource_code: resource_code as u8,
            }
        })
    }
}

#[cfg(feature = "mojo-core")]
pub(super) fn runtime_gateway_admin_resource_route<'a>(
    method: &str,
    segments: &'a [&'a str],
) -> RuntimeGatewayAdminResourceRoute<'a> {
    use prodex_mojo_core::rich::GatewayAdminResourceRoute as MojoRoute;
    match prodex_mojo_core::rich::plan_gateway_admin_resource_route(method, segments) {
        Err(_) => RuntimeGatewayAdminResourceRoute::NotFound,
        Ok(route) => match route {
            MojoRoute::Create => RuntimeGatewayAdminResourceRoute::Create,
            MojoRoute::List => RuntimeGatewayAdminResourceRoute::List,
            MojoRoute::Validate => RuntimeGatewayAdminResourceRoute::Validate,
            MojoRoute::Status => RuntimeGatewayAdminResourceRoute::Status,
            MojoRoute::Get { revision_id } => RuntimeGatewayAdminResourceRoute::Get { revision_id },
            MojoRoute::Submit { revision_id } => {
                RuntimeGatewayAdminResourceRoute::Submit { revision_id }
            }
            MojoRoute::Vote {
                revision_id,
                approval_id,
            } => RuntimeGatewayAdminResourceRoute::Vote {
                revision_id,
                approval_id,
            },
            MojoRoute::Activate {
                revision_id,
                action,
            } => RuntimeGatewayAdminResourceRoute::Activate {
                revision_id,
                action,
            },
            MojoRoute::NotFound => RuntimeGatewayAdminResourceRoute::NotFound,
            MojoRoute::MethodNotAllowed => RuntimeGatewayAdminResourceRoute::MethodNotAllowed,
        },
    }
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn runtime_gateway_admin_resource_route<'a>(
    method: &str,
    segments: &'a [&'a str],
) -> RuntimeGatewayAdminResourceRoute<'a> {
    rust_route(method, segments)
}

#[cfg(any(test, not(feature = "mojo-core")))]
fn rust_route<'a>(method: &str, segments: &'a [&'a str]) -> RuntimeGatewayAdminResourceRoute<'a> {
    match (method.to_ascii_uppercase().as_str(), segments) {
        ("POST", []) => RuntimeGatewayAdminResourceRoute::Create,
        ("GET", []) => RuntimeGatewayAdminResourceRoute::List,
        ("POST", ["validate"]) => RuntimeGatewayAdminResourceRoute::Validate,
        ("GET", ["status"]) => RuntimeGatewayAdminResourceRoute::Status,
        ("GET", [revision_id]) => RuntimeGatewayAdminResourceRoute::Get { revision_id },
        ("POST", [revision_id, "submit"]) => {
            RuntimeGatewayAdminResourceRoute::Submit { revision_id }
        }
        ("POST", [revision_id, "approvals", approval_id, "votes"]) => {
            RuntimeGatewayAdminResourceRoute::Vote {
                revision_id,
                approval_id,
            }
        }
        ("POST", [revision_id, action @ ("activate" | "rollback" | "revoke")]) => {
            RuntimeGatewayAdminResourceRoute::Activate {
                revision_id,
                action,
            }
        }
        ("GET" | "POST", _) => RuntimeGatewayAdminResourceRoute::NotFound,
        _ => RuntimeGatewayAdminResourceRoute::MethodNotAllowed,
    }
}

#[cfg(all(test, feature = "mojo-core"))]
mod tests {
    use super::*;

    #[test]
    fn mojo_gateway_admin_routes_match_the_non_mojo_oracle() {
        for path in [
            "/admin/audit/exports",
            "/admin/governance/audit/integrity",
            "/admin/governance/outbox/claim",
            "/admin/audit/retention/holds/event-1",
            "/admin/execution-approvals/approval-1",
            "/admin/break-glass-approvals/approval-1",
            "/admin/provider-registries/rev-1",
            "/admin/unrelated",
        ] {
            assert_eq!(
                runtime_gateway_admin_policy_route(path, "/admin"),
                rust_policy_route(path, "/admin")
            );
        }
        let cases: &[(&str, &[&str])] = &[
            ("GET", &[]),
            ("POST", &[]),
            ("POST", &["validate"]),
            ("GET", &["status"]),
            ("GET", &["rev-1"]),
            ("POST", &["rev-1", "submit"]),
            ("POST", &["rev-1", "activate"]),
            ("POST", &["rev-1", "approvals", "approval-1", "votes"]),
            ("POST", &["missing"]),
            ("DELETE", &[]),
        ];
        for (method, segments) in cases {
            assert_eq!(
                runtime_gateway_admin_resource_route(method, segments),
                rust_route(method, segments)
            );
        }
    }
}
