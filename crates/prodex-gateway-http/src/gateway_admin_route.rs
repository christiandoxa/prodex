use crate::{GatewayControlPlaneOperation, GatewayHttpMethod};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayGovernanceResourceFamily {
    Policy,
    ExecutionApproval,
    ClassificationRule,
    ProviderRegistry,
    RoutingScore,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayGovernanceResourceRoute<'a> {
    Collection,
    Validate,
    Status,
    Resource {
        resource_id: &'a str,
    },
    Submit {
        resource_id: &'a str,
    },
    Vote {
        resource_id: &'a str,
        approval_id: &'a str,
    },
    Activate {
        resource_id: &'a str,
    },
    Rollback {
        resource_id: &'a str,
    },
    Unknown(Vec<&'a str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GatewayAdminRoute<'a> {
    Dashboard,
    OpenApi,
    Metrics,
    Providers,
    Observability,
    Guardrails,
    RouteExplain,
    Usage,
    Keys,
    Key {
        key_name: &'a str,
    },
    KeySecret {
        key_name: &'a str,
    },
    KeyUnknown(Vec<&'a str>),
    Ledger,
    LedgerCsv,
    LedgerSummary,
    LedgerSummaryCsv,
    ScimUsers,
    ScimUser {
        user_id: &'a str,
    },
    ScimUnknown(Vec<&'a str>),
    Governance {
        family: GatewayGovernanceResourceFamily,
        resource: GatewayGovernanceResourceRoute<'a>,
    },
    SessionRevoke {
        session_id_hash: &'a str,
    },
    SessionUnknown(Vec<&'a str>),
    GovernanceOutbox,
    GovernanceOutboxClaim,
    GovernanceAuditIntegrity,
    AuditExports,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GatewayAdminHandlerCategory {
    Dashboard,
    OpenApi,
    Metrics,
    StaticJson,
    RouteExplain,
    Keys,
    Ledger,
    Scim,
    Governance,
    Session,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GatewayAdminRouteMetadata {
    pub allowed_methods: &'static [GatewayHttpMethod],
    pub read_only: bool,
    pub requires_resource_id: bool,
    pub handler: GatewayAdminHandlerCategory,
}

const GET: &[GatewayHttpMethod] = &[GatewayHttpMethod::Get];
const POST: &[GatewayHttpMethod] = &[GatewayHttpMethod::Post];
const GET_POST: &[GatewayHttpMethod] = &[GatewayHttpMethod::Get, GatewayHttpMethod::Post];
const KEY_RESOURCE: &[GatewayHttpMethod] = &[
    GatewayHttpMethod::Get,
    GatewayHttpMethod::Patch,
    GatewayHttpMethod::Delete,
];
const SCIM_RESOURCE: &[GatewayHttpMethod] = &[
    GatewayHttpMethod::Get,
    GatewayHttpMethod::Put,
    GatewayHttpMethod::Patch,
    GatewayHttpMethod::Delete,
];

impl GatewayAdminRoute<'_> {
    pub fn metadata(&self) -> GatewayAdminRouteMetadata {
        use GatewayAdminHandlerCategory as Handler;
        match self {
            Self::Dashboard => metadata(GET, true, false, Handler::Dashboard),
            Self::OpenApi => metadata(GET, true, false, Handler::OpenApi),
            Self::Metrics => metadata(GET, true, false, Handler::Metrics),
            Self::Providers | Self::Observability | Self::Guardrails | Self::Usage => {
                metadata(GET, true, false, Handler::StaticJson)
            }
            Self::RouteExplain => metadata(POST, true, false, Handler::RouteExplain),
            Self::Keys => metadata(GET_POST, false, false, Handler::Keys),
            Self::Key { .. } | Self::KeyUnknown(_) => {
                metadata(KEY_RESOURCE, false, true, Handler::Keys)
            }
            Self::KeySecret { .. } => metadata(POST, false, true, Handler::Keys),
            Self::Ledger | Self::LedgerCsv | Self::LedgerSummary | Self::LedgerSummaryCsv => {
                metadata(GET, true, false, Handler::Ledger)
            }
            Self::ScimUsers => metadata(GET_POST, false, false, Handler::Scim),
            Self::ScimUser { .. } | Self::ScimUnknown(_) => {
                metadata(SCIM_RESOURCE, false, true, Handler::Scim)
            }
            Self::Governance { resource, .. } => match resource {
                GatewayGovernanceResourceRoute::Collection => {
                    metadata(GET_POST, false, false, Handler::Governance)
                }
                GatewayGovernanceResourceRoute::Status
                | GatewayGovernanceResourceRoute::Resource { .. } => {
                    metadata(GET, true, true, Handler::Governance)
                }
                GatewayGovernanceResourceRoute::Validate => {
                    metadata(POST, false, false, Handler::Governance)
                }
                GatewayGovernanceResourceRoute::Submit { .. }
                | GatewayGovernanceResourceRoute::Vote { .. }
                | GatewayGovernanceResourceRoute::Activate { .. }
                | GatewayGovernanceResourceRoute::Rollback { .. } => {
                    metadata(POST, false, true, Handler::Governance)
                }
                GatewayGovernanceResourceRoute::Unknown(_) => {
                    metadata(GET_POST, false, false, Handler::Governance)
                }
            },
            Self::GovernanceOutbox | Self::GovernanceAuditIntegrity => {
                metadata(GET, true, false, Handler::Governance)
            }
            Self::GovernanceOutboxClaim | Self::AuditExports => {
                metadata(POST, false, false, Handler::Governance)
            }
            Self::SessionRevoke { .. } | Self::SessionUnknown(_) => {
                metadata(POST, false, true, Handler::Session)
            }
        }
    }

    pub fn operation(&self, method: GatewayHttpMethod) -> Option<GatewayControlPlaneOperation> {
        Some(match self {
            Self::RouteExplain => GatewayControlPlaneOperation::RouteExplain,
            Self::Keys => {
                if method == GatewayHttpMethod::Get {
                    GatewayControlPlaneOperation::VirtualKeyRead
                } else {
                    GatewayControlPlaneOperation::VirtualKeyCreate
                }
            }
            Self::Key { .. } | Self::KeyUnknown(_) => match method {
                GatewayHttpMethod::Get => GatewayControlPlaneOperation::VirtualKeyRead,
                GatewayHttpMethod::Patch => GatewayControlPlaneOperation::VirtualKeyUpdate,
                GatewayHttpMethod::Delete => GatewayControlPlaneOperation::VirtualKeyDelete,
                _ => return None,
            },
            Self::KeySecret { .. } => GatewayControlPlaneOperation::VirtualKeyRotateSecret,
            Self::ScimUsers => {
                if method == GatewayHttpMethod::Get {
                    GatewayControlPlaneOperation::ScimUserRead
                } else {
                    GatewayControlPlaneOperation::ScimUserCreate
                }
            }
            Self::ScimUser { .. } | Self::ScimUnknown(_) => match method {
                GatewayHttpMethod::Get => GatewayControlPlaneOperation::ScimUserRead,
                GatewayHttpMethod::Delete => GatewayControlPlaneOperation::ScimUserDelete,
                _ => GatewayControlPlaneOperation::ScimUserUpdate,
            },
            Self::Ledger | Self::LedgerCsv | Self::LedgerSummary | Self::LedgerSummaryCsv => {
                GatewayControlPlaneOperation::BillingRead
            }
            Self::AuditExports => GatewayControlPlaneOperation::AuditExport,
            Self::Governance { .. }
            | Self::SessionRevoke { .. }
            | Self::GovernanceOutbox
            | Self::GovernanceOutboxClaim
            | Self::GovernanceAuditIntegrity => GatewayControlPlaneOperation::PolicyPublish,
            Self::Dashboard
            | Self::OpenApi
            | Self::Metrics
            | Self::Providers
            | Self::Observability
            | Self::Guardrails
            | Self::Usage => GatewayControlPlaneOperation::GatewayAdminRead,
            Self::SessionUnknown(_) => return None,
        })
    }
}

const fn metadata(
    allowed_methods: &'static [GatewayHttpMethod],
    read_only: bool,
    requires_resource_id: bool,
    handler: GatewayAdminHandlerCategory,
) -> GatewayAdminRouteMetadata {
    GatewayAdminRouteMetadata {
        allowed_methods,
        read_only,
        requires_resource_id,
        handler,
    }
}

pub fn parse_gateway_admin_route<'a>(
    mount_path: &str,
    request_path: &'a str,
) -> Option<GatewayAdminRoute<'a>> {
    let path = request_path
        .trim()
        .split_once(['?', '#'])
        .map_or_else(|| request_path.trim(), |(path, _)| path);
    let mount_path = mount_path.trim_end_matches('/');
    let prefix = format!("{mount_path}/prodex/gateway");
    let suffix = path.strip_prefix(&prefix)?.strip_prefix('/')?;
    let segments = suffix.split('/').collect::<Vec<_>>();
    match segments.as_slice() {
        ["admin"] => Some(GatewayAdminRoute::Dashboard),
        ["openapi.json"] => Some(GatewayAdminRoute::OpenApi),
        ["metrics"] => Some(GatewayAdminRoute::Metrics),
        ["providers"] => Some(GatewayAdminRoute::Providers),
        ["observability"] => Some(GatewayAdminRoute::Observability),
        ["guardrails"] => Some(GatewayAdminRoute::Guardrails),
        ["routes", "explain"] => Some(GatewayAdminRoute::RouteExplain),
        ["usage"] => Some(GatewayAdminRoute::Usage),
        ["keys"] => Some(GatewayAdminRoute::Keys),
        ["keys", key_name, "secret" | "secrets"] if !key_name.is_empty() => {
            Some(GatewayAdminRoute::KeySecret { key_name })
        }
        ["keys", key_name] if !key_name.is_empty() => Some(GatewayAdminRoute::Key { key_name }),
        ["keys", rest @ ..] => Some(GatewayAdminRoute::KeyUnknown(rest.to_vec())),
        ["ledger"] => Some(GatewayAdminRoute::Ledger),
        ["ledger.csv"] => Some(GatewayAdminRoute::LedgerCsv),
        ["ledger", "summary"] => Some(GatewayAdminRoute::LedgerSummary),
        ["ledger", "summary.csv"] => Some(GatewayAdminRoute::LedgerSummaryCsv),
        ["scim", "v2", "Users"] => Some(GatewayAdminRoute::ScimUsers),
        ["scim", "v2", "Users", user_id] if !user_id.is_empty() => {
            Some(GatewayAdminRoute::ScimUser { user_id })
        }
        ["scim", "v2", "Users", rest @ ..] => Some(GatewayAdminRoute::ScimUnknown(rest.to_vec())),
        [
            "policies" | "classification-rules" | "provider-registries" | "routing-scores",
            rest @ ..,
        ] => {
            let family = match segments[0] {
                "policies" => GatewayGovernanceResourceFamily::Policy,
                "classification-rules" => GatewayGovernanceResourceFamily::ClassificationRule,
                "provider-registries" => GatewayGovernanceResourceFamily::ProviderRegistry,
                _ => GatewayGovernanceResourceFamily::RoutingScore,
            };
            Some(GatewayAdminRoute::Governance {
                family,
                resource: governance_resource_route(rest),
            })
        }
        ["execution-approvals", rest @ ..] => Some(GatewayAdminRoute::Governance {
            family: GatewayGovernanceResourceFamily::ExecutionApproval,
            resource: execution_approval_route(rest),
        }),
        ["sessions", session_id_hash, "revoke"] if !session_id_hash.is_empty() => {
            Some(GatewayAdminRoute::SessionRevoke { session_id_hash })
        }
        ["sessions", rest @ ..] => Some(GatewayAdminRoute::SessionUnknown(rest.to_vec())),
        ["governance", "outbox"] => Some(GatewayAdminRoute::GovernanceOutbox),
        ["governance", "outbox", "claim"] => Some(GatewayAdminRoute::GovernanceOutboxClaim),
        ["governance", "audit", "integrity"] => Some(GatewayAdminRoute::GovernanceAuditIntegrity),
        ["audit", "exports"] => Some(GatewayAdminRoute::AuditExports),
        _ => None,
    }
}

fn governance_resource_route<'a>(segments: &[&'a str]) -> GatewayGovernanceResourceRoute<'a> {
    match segments {
        [] => GatewayGovernanceResourceRoute::Collection,
        ["validate"] => GatewayGovernanceResourceRoute::Validate,
        ["status"] => GatewayGovernanceResourceRoute::Status,
        [resource_id] if !resource_id.is_empty() => {
            GatewayGovernanceResourceRoute::Resource { resource_id }
        }
        [resource_id, "submit"] if !resource_id.is_empty() => {
            GatewayGovernanceResourceRoute::Submit { resource_id }
        }
        [resource_id, "approvals", approval_id, "votes"]
            if !resource_id.is_empty() && !approval_id.is_empty() =>
        {
            GatewayGovernanceResourceRoute::Vote {
                resource_id,
                approval_id,
            }
        }
        [resource_id, "activate"] if !resource_id.is_empty() => {
            GatewayGovernanceResourceRoute::Activate { resource_id }
        }
        [resource_id, "rollback"] if !resource_id.is_empty() => {
            GatewayGovernanceResourceRoute::Rollback { resource_id }
        }
        _ => GatewayGovernanceResourceRoute::Unknown(segments.to_vec()),
    }
}

fn execution_approval_route<'a>(segments: &[&'a str]) -> GatewayGovernanceResourceRoute<'a> {
    match segments {
        [] => GatewayGovernanceResourceRoute::Collection,
        [resource_id] if !resource_id.is_empty() => {
            GatewayGovernanceResourceRoute::Resource { resource_id }
        }
        [resource_id, "votes"] if !resource_id.is_empty() => GatewayGovernanceResourceRoute::Vote {
            resource_id,
            approval_id: resource_id,
        },
        _ => GatewayGovernanceResourceRoute::Unknown(segments.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_covers_exact_resource_and_near_miss_routes() {
        assert_eq!(
            parse_gateway_admin_route("/v1", "/v1/prodex/gateway/keys/example-key/secret?x=1"),
            Some(GatewayAdminRoute::KeySecret {
                key_name: "example-key"
            })
        );
        assert_eq!(
            parse_gateway_admin_route("", "/prodex/gateway/scim/v2/Users/user-1"),
            Some(GatewayAdminRoute::ScimUser { user_id: "user-1" })
        );
        assert_eq!(
            parse_gateway_admin_route("", "/prodex/gateway/sessions/current/revoke"),
            Some(GatewayAdminRoute::SessionRevoke {
                session_id_hash: "current"
            })
        );
        assert_eq!(
            parse_gateway_admin_route("", "/prodex/gateway/keys/example-key/extra"),
            Some(GatewayAdminRoute::KeyUnknown(vec!["example-key", "extra"]))
        );
        assert_eq!(
            parse_gateway_admin_route("", "/prodex/gateway/not-supported"),
            None
        );
        assert_eq!(
            parse_gateway_admin_route("", "/prodex/gatewayish/keys"),
            None
        );
    }

    #[test]
    fn metadata_and_operation_are_route_owned() {
        let route = GatewayAdminRoute::Key {
            key_name: "example-key",
        };
        assert_eq!(route.metadata().allowed_methods, KEY_RESOURCE);
        assert!(!route.metadata().read_only);
        assert!(route.metadata().requires_resource_id);
        assert_eq!(
            route.operation(GatewayHttpMethod::Delete),
            Some(GatewayControlPlaneOperation::VirtualKeyDelete)
        );
    }

    #[test]
    fn parser_classifies_every_prodex_admin_endpoint_family() {
        use GatewayAdminHandlerCategory as Handler;
        for (path, handler) in [
            ("/prodex/gateway/admin", Handler::Dashboard),
            ("/prodex/gateway/openapi.json", Handler::OpenApi),
            ("/prodex/gateway/metrics", Handler::Metrics),
            ("/prodex/gateway/providers", Handler::StaticJson),
            ("/prodex/gateway/observability", Handler::StaticJson),
            ("/prodex/gateway/guardrails", Handler::StaticJson),
            ("/prodex/gateway/routes/explain", Handler::RouteExplain),
            ("/prodex/gateway/usage", Handler::StaticJson),
            ("/prodex/gateway/keys", Handler::Keys),
            ("/prodex/gateway/keys/key-1", Handler::Keys),
            ("/prodex/gateway/keys/key-1/secret", Handler::Keys),
            ("/prodex/gateway/ledger", Handler::Ledger),
            ("/prodex/gateway/ledger.csv", Handler::Ledger),
            ("/prodex/gateway/ledger/summary", Handler::Ledger),
            ("/prodex/gateway/ledger/summary.csv", Handler::Ledger),
            ("/prodex/gateway/scim/v2/Users", Handler::Scim),
            ("/prodex/gateway/scim/v2/Users/user-1", Handler::Scim),
            ("/prodex/gateway/policies", Handler::Governance),
            ("/prodex/gateway/policies/rev-1", Handler::Governance),
            ("/prodex/gateway/execution-approvals", Handler::Governance),
            ("/prodex/gateway/classification-rules", Handler::Governance),
            ("/prodex/gateway/provider-registries", Handler::Governance),
            ("/prodex/gateway/routing-scores", Handler::Governance),
            ("/prodex/gateway/sessions/current/revoke", Handler::Session),
            ("/prodex/gateway/governance/outbox", Handler::Governance),
            (
                "/prodex/gateway/governance/outbox/claim",
                Handler::Governance,
            ),
            (
                "/prodex/gateway/governance/audit/integrity",
                Handler::Governance,
            ),
            ("/prodex/gateway/audit/exports", Handler::Governance),
        ] {
            let route = parse_gateway_admin_route("", path).expect(path);
            assert_eq!(route.metadata().handler, handler, "{path}");
        }
    }

    #[test]
    fn governance_resource_ids_are_typed_without_hiding_unknown_suffixes() {
        assert_eq!(
            parse_gateway_admin_route(
                "",
                "/prodex/gateway/policies/rev-1/approvals/approval-1/votes",
            ),
            Some(GatewayAdminRoute::Governance {
                family: GatewayGovernanceResourceFamily::Policy,
                resource: GatewayGovernanceResourceRoute::Vote {
                    resource_id: "rev-1",
                    approval_id: "approval-1",
                },
            })
        );
        assert!(matches!(
            parse_gateway_admin_route("", "/prodex/gateway/policies/rev-1/extra"),
            Some(GatewayAdminRoute::Governance {
                resource: GatewayGovernanceResourceRoute::Unknown(_),
                ..
            })
        ));
        assert!(matches!(
            parse_gateway_admin_route("", "/prodex/gateway/sessions/rev-1/extra"),
            Some(GatewayAdminRoute::SessionUnknown(_))
        ));
        assert_eq!(
            parse_gateway_admin_route("", "/prodex/gateway/keys//secret"),
            Some(GatewayAdminRoute::KeyUnknown(vec!["", "secret"]))
        );
        assert!(matches!(
            parse_gateway_admin_route("", "/prodex/gateway/policies//submit"),
            Some(GatewayAdminRoute::Governance {
                resource: GatewayGovernanceResourceRoute::Unknown(_),
                ..
            })
        ));
        assert!(matches!(
            parse_gateway_admin_route("", "/prodex/gateway/scim/v2/Users/user-1/extra"),
            Some(GatewayAdminRoute::ScimUnknown(_))
        ));
    }
}
