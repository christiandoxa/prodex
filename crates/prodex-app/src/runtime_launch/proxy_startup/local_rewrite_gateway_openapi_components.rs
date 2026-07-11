use super::local_rewrite_gateway_openapi_route_explain::runtime_gateway_openapi_route_explain_schemas;
use serde_json::{Map, Value, json};

type SchemaMap = Map<String, Value>;
type NamedSchema = (&'static str, Value);

pub(super) fn runtime_gateway_openapi_components() -> Value {
    let mut schemas = schema_map([
        ("GatewayExactIdentifier", exact_identifier_schema()),
        (
            "GatewayNullableExactIdentifier",
            nullable_schema_ref("GatewayExactIdentifier"),
        ),
        (
            "GatewayRouteModelIdentifier",
            route_model_identifier_schema(),
        ),
        (
            "GatewayNullableRouteModelIdentifier",
            nullable_schema_ref("GatewayRouteModelIdentifier"),
        ),
        ("GatewayError", gateway_error_schema()),
        ("GatewayKeyCreateRequest", gateway_key_create_schema()),
        ("GatewayKeyPatchRequest", gateway_key_patch_schema()),
        (
            "GatewayKeyMutationResponse",
            gateway_key_mutation_response_schema(),
        ),
        ("GatewayHealth", gateway_health_schema()),
        ("GatewayKeyResponse", gateway_key_response_schema()),
        ("GatewayKeyList", gateway_key_list_schema()),
        ("GatewayScimUserList", gateway_scim_user_list_schema()),
        ("GatewayScimUserWrite", gateway_scim_user_write_schema()),
        ("GatewayScimUser", gateway_scim_user_schema()),
        ("GatewayBillingLedger", gateway_billing_ledger_schema()),
        ("GatewayBillingSummary", gateway_billing_summary_schema()),
        (
            "GatewayBillingSummaryBucket",
            gateway_billing_summary_bucket_schema(),
        ),
        (
            "GatewayBillingLedgerEntry",
            gateway_billing_ledger_entry_schema(),
        ),
        ("GatewayKey", gateway_key_schema()),
        ("GatewayUsage", gateway_usage_schema()),
        ("GatewayObservability", gateway_observability_schema()),
        ("GatewayProviders", gateway_providers_schema()),
        (
            "GatewayProviderContract",
            gateway_provider_contract_schema(),
        ),
        ("GatewayGuardrails", gateway_guardrails_schema()),
        ("GatewayKeyDeleted", gateway_key_deleted_schema()),
    ]);
    schemas.extend(runtime_gateway_openapi_route_explain_schemas());

    json!({
        "securitySchemes": {
            "GatewayBearerAuth": {
                "type": "http",
                "scheme": "bearer"
            }
        },
        "responses": {
            "GatewayError": {
                "description": "Gateway JSON error",
                "content": {
                    "application/json": {
                        "schema": schema_ref("GatewayError")
                    }
                }
            }
        },
        "schemas": schemas
    })
}

fn schema_map<const N: usize>(schemas: [NamedSchema; N]) -> SchemaMap {
    schemas
        .into_iter()
        .map(|(name, schema)| (name.to_owned(), schema))
        .collect()
}

fn object_schema(
    required: &[&str],
    properties: SchemaMap,
    additional_properties: Option<bool>,
) -> Value {
    let mut schema = json!({"type": "object", "properties": properties});
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    if let Some(additional_properties) = additional_properties {
        schema["additionalProperties"] = json!(additional_properties);
    }
    schema
}

fn schema_ref(name: &str) -> Value {
    json!({"$ref": format!("#/components/schemas/{name}")})
}

fn nullable_schema_ref(name: &str) -> Value {
    json!({"anyOf": [schema_ref(name), {"type": "null"}]})
}

fn array_schema(items: Value) -> Value {
    json!({"type": "array", "items": items})
}

fn exact_identifier_schema() -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "pattern": "^\\S+$",
        "description": "Exact non-empty identifier; whitespace is not normalized."
    })
}

fn route_model_identifier_schema() -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "maxLength": 256,
        "pattern": "^\\S+$"
    })
}

fn gateway_error_schema() -> Value {
    object_schema(
        &["error"],
        schema_map([(
            "error",
            object_schema(
                &["code", "message"],
                schema_map([
                    ("code", json!({"type": "string"})),
                    ("message", json!({"type": "string"})),
                ]),
                None,
            ),
        )]),
        None,
    )
}

fn gateway_scope_properties() -> SchemaMap {
    schema_map([
        ("tenant_id", schema_ref("GatewayNullableExactIdentifier")),
        ("team_id", schema_ref("GatewayNullableExactIdentifier")),
        ("project_id", schema_ref("GatewayNullableExactIdentifier")),
        ("user_id", schema_ref("GatewayNullableExactIdentifier")),
        ("budget_id", schema_ref("GatewayNullableExactIdentifier")),
    ])
}

fn gateway_key_policy_properties() -> SchemaMap {
    let mut properties = gateway_scope_properties();
    properties.extend(schema_map([
        (
            "allowed_models",
            array_schema(schema_ref("GatewayExactIdentifier")),
        ),
        (
            "budget_microusd",
            json!({"type": ["integer", "null"], "minimum": 0}),
        ),
        (
            "budget_usd",
            json!({"type": ["number", "null"], "minimum": 0}),
        ),
        (
            "request_budget",
            json!({"type": ["integer", "null"], "minimum": 0}),
        ),
        (
            "rpm_limit",
            json!({"type": ["integer", "null"], "minimum": 0}),
        ),
        (
            "tpm_limit",
            json!({"type": ["integer", "null"], "minimum": 0}),
        ),
        ("disabled", json!({"type": "boolean"})),
    ]));
    properties
}

fn gateway_key_create_schema() -> Value {
    let mut properties = gateway_key_policy_properties();
    properties.extend(schema_map([
        ("name", schema_ref("GatewayExactIdentifier")),
        (
            "token",
            json!({
                "type": "string",
                "description": "Optional caller-supplied bearer token. Omit to have Prodex generate one."
            }),
        ),
    ]));
    object_schema(&["name"], properties, Some(false))
}

fn gateway_key_patch_schema() -> Value {
    let mut properties = gateway_key_policy_properties();
    properties.extend(schema_map([
        (
            "token",
            json!({
                "type": "string",
                "description": "Optional replacement bearer token. The stored file only keeps its hash."
            }),
        ),
        (
            "rotate",
            json!({
                "type": "boolean",
                "description": "Generate and return a new bearer token once."
            }),
        ),
    ]));
    object_schema(&[], properties, Some(false))
}

fn gateway_key_mutation_response_schema() -> Value {
    object_schema(
        &["object", "key"],
        schema_map([
            ("object", json!({"type": "string"})),
            ("key", schema_ref("GatewayKey")),
            (
                "token",
                json!({
                    "type": ["string", "null"],
                    "description": "Returned only when Prodex generated or rotated the token for this request."
                }),
            ),
        ]),
        None,
    )
}

fn gateway_health_schema() -> Value {
    object_schema(
        &[
            "object",
            "probe",
            "status",
            "ready",
            "local_overload",
            "draining",
            "policy_version",
            "active_requests",
            "active_request_limit",
        ],
        schema_map([
            (
                "object",
                json!({"type": "string", "const": "gateway.health"}),
            ),
            (
                "probe",
                json!({"type": "string", "enum": ["livez", "readyz", "startupz"]}),
            ),
            (
                "status",
                json!({"type": "string", "enum": ["ok", "overloaded", "draining", "method_not_allowed"]}),
            ),
            ("ready", json!({"type": "boolean"})),
            ("local_overload", json!({"type": "boolean"})),
            ("draining", json!({"type": "boolean"})),
            (
                "policy_version",
                json!({"type": ["integer", "null"], "minimum": 0}),
            ),
            ("active_requests", json!({"type": "integer", "minimum": 0})),
            (
                "active_request_limit",
                json!({"type": "integer", "minimum": 0}),
            ),
        ]),
        None,
    )
}

fn gateway_key_response_schema() -> Value {
    object_schema(
        &["object", "key"],
        schema_map([
            ("object", json!({"type": "string"})),
            ("key", schema_ref("GatewayKey")),
        ]),
        None,
    )
}

fn state_backend_schema() -> Value {
    json!({"type": "string", "enum": ["file", "sqlite", "postgres", "redis"]})
}

fn gateway_key_list_schema() -> Value {
    object_schema(
        &["object", "keys"],
        schema_map([
            ("object", json!({"type": "string"})),
            ("keys", array_schema(schema_ref("GatewayKey"))),
            ("state_backend", state_backend_schema()),
            ("state_path", json!({"type": "string"})),
            ("key_store_path", json!({"type": "string"})),
            ("usage_path", json!({"type": ["string", "null"]})),
            (
                "unknown_persisted_keys",
                array_schema(json!({"type": "string"})),
            ),
        ]),
        None,
    )
}

fn gateway_scim_user_base_properties() -> SchemaMap {
    let mut properties = gateway_scope_properties();
    properties.extend(schema_map([
        ("userName", schema_ref("GatewayExactIdentifier")),
        ("externalId", json!({"type": ["string", "null"]})),
        ("displayName", json!({"type": ["string", "null"]})),
        ("active", json!({"type": "boolean"})),
    ]));
    properties
}

fn gateway_scim_user_list_schema() -> Value {
    object_schema(
        &["schemas", "totalResults", "Resources"],
        schema_map([
            ("schemas", array_schema(json!({"type": "string"}))),
            ("totalResults", json!({"type": "integer"})),
            ("startIndex", json!({"type": "integer"})),
            ("itemsPerPage", json!({"type": "integer"})),
            ("Resources", array_schema(schema_ref("GatewayScimUser"))),
        ]),
        None,
    )
}

fn gateway_scim_user_write_schema() -> Value {
    let mut properties = gateway_scim_user_base_properties();
    properties.extend(schema_map([
        (
            "role",
            json!({"type": ["string", "null"], "enum": ["admin", "viewer", null]}),
        ),
        (
            "allowed_key_prefixes",
            array_schema(schema_ref("GatewayExactIdentifier")),
        ),
    ]));
    object_schema(&["userName"], properties, Some(true))
}

fn gateway_scim_user_schema() -> Value {
    let mut properties = gateway_scim_user_base_properties();
    properties.extend(schema_map([
        ("schemas", array_schema(json!({"type": "string"}))),
        ("id", json!({"type": "string"})),
        (
            "meta",
            json!({"type": "object", "additionalProperties": true}),
        ),
    ]));
    object_schema(
        &["schemas", "id", "userName", "active"],
        properties,
        Some(true),
    )
}

fn gateway_billing_ledger_schema() -> Value {
    object_schema(
        &["object", "records"],
        schema_map([
            ("object", json!({"type": "string"})),
            ("state_backend", state_backend_schema()),
            ("ledger_path", json!({"type": "string"})),
            ("limit", json!({"type": "integer"})),
            (
                "records",
                array_schema(schema_ref("GatewayBillingLedgerEntry")),
            ),
        ]),
        None,
    )
}

fn billing_summary_bucket_array_schema() -> Value {
    array_schema(schema_ref("GatewayBillingSummaryBucket"))
}

fn gateway_billing_summary_schema() -> Value {
    object_schema(
        &["object", "totals", "by_key", "by_model", "by_key_model"],
        schema_map([
            ("object", json!({"type": "string"})),
            ("state_backend", state_backend_schema()),
            ("ledger_path", json!({"type": "string"})),
            ("record_count", json!({"type": "integer"})),
            ("totals", schema_ref("GatewayBillingSummaryBucket")),
            ("by_key", billing_summary_bucket_array_schema()),
            ("by_model", billing_summary_bucket_array_schema()),
            ("by_key_model", billing_summary_bucket_array_schema()),
            ("by_tenant", billing_summary_bucket_array_schema()),
            ("by_team", billing_summary_bucket_array_schema()),
            ("by_project", billing_summary_bucket_array_schema()),
            ("by_user", billing_summary_bucket_array_schema()),
            ("by_budget", billing_summary_bucket_array_schema()),
        ]),
        None,
    )
}

fn gateway_billing_summary_bucket_schema() -> Value {
    object_schema(
        &[
            "requests",
            "successful_requests",
            "failed_requests",
            "unreconciled_requests",
            "input_tokens",
            "output_tokens",
            "response_bytes",
            "estimated_cost_microusd",
            "estimated_cost_usd",
            "final_cost_microusd",
            "final_cost_usd",
        ],
        schema_map([
            ("key_name", json!({"type": ["string", "null"]})),
            ("model", json!({"type": ["string", "null"]})),
            ("tenant_id", json!({"type": ["string", "null"]})),
            ("team_id", json!({"type": ["string", "null"]})),
            ("project_id", json!({"type": ["string", "null"]})),
            ("user_id", json!({"type": ["string", "null"]})),
            ("budget_id", json!({"type": ["string", "null"]})),
            ("requests", json!({"type": "integer"})),
            ("successful_requests", json!({"type": "integer"})),
            ("failed_requests", json!({"type": "integer"})),
            ("unreconciled_requests", json!({"type": "integer"})),
            ("input_tokens", json!({"type": "integer"})),
            ("output_tokens", json!({"type": "integer"})),
            ("response_bytes", json!({"type": "integer"})),
            ("estimated_cost_microusd", json!({"type": "integer"})),
            ("estimated_cost_usd", json!({"type": "number"})),
            ("final_cost_microusd", json!({"type": "integer"})),
            ("final_cost_usd", json!({"type": "number"})),
            (
                "first_created_at_epoch",
                json!({"type": ["integer", "null"]}),
            ),
            (
                "last_created_at_epoch",
                json!({"type": ["integer", "null"]}),
            ),
            (
                "last_reconciled_at_epoch",
                json!({"type": ["integer", "null"]}),
            ),
        ]),
        None,
    )
}

fn gateway_billing_ledger_entry_schema() -> Value {
    object_schema(
        &[
            "object",
            "phase",
            "request",
            "call_id",
            "key_name",
            "model",
            "minute_epoch",
            "input_tokens",
            "created_at_epoch",
        ],
        schema_map([
            ("object", json!({"type": "string"})),
            ("phase", json!({"type": "string", "enum": ["request"]})),
            ("request_id", json!({"type": ["string", "null"]})),
            ("request", json!({"type": "integer"})),
            ("call_id", json!({"type": "string"})),
            ("key_name", json!({"type": "string"})),
            ("tenant_id", json!({"type": ["string", "null"]})),
            ("team_id", json!({"type": ["string", "null"]})),
            ("project_id", json!({"type": ["string", "null"]})),
            ("user_id", json!({"type": ["string", "null"]})),
            ("budget_id", json!({"type": ["string", "null"]})),
            ("model", json!({"type": "string"})),
            ("minute_epoch", json!({"type": "integer"})),
            ("input_tokens", json!({"type": "integer"})),
            (
                "estimated_cost_microusd",
                json!({"type": ["integer", "null"]}),
            ),
            ("estimated_cost_usd", json!({"type": ["number", "null"]})),
            ("created_at_epoch", json!({"type": "integer"})),
            ("response_status", json!({"type": ["integer", "null"]})),
            ("response_bytes", json!({"type": ["integer", "null"]})),
            ("output_tokens", json!({"type": ["integer", "null"]})),
            ("final_cost_microusd", json!({"type": ["integer", "null"]})),
            ("final_cost_usd", json!({"type": ["number", "null"]})),
            ("reconciled_at_epoch", json!({"type": ["integer", "null"]})),
        ]),
        None,
    )
}

fn gateway_key_schema() -> Value {
    let mut properties = gateway_scope_properties();
    properties.extend(schema_map([
        ("name", schema_ref("GatewayExactIdentifier")),
        (
            "source",
            json!({"type": "string", "enum": ["policy", "admin"]}),
        ),
        ("disabled", json!({"type": "boolean"})),
        ("editable", json!({"type": "boolean"})),
        ("created_at_epoch", json!({"type": ["integer", "null"]})),
        ("updated_at_epoch", json!({"type": ["integer", "null"]})),
        (
            "allowed_models",
            array_schema(schema_ref("GatewayExactIdentifier")),
        ),
        ("budget_microusd", json!({"type": ["integer", "null"]})),
        ("budget_usd", json!({"type": ["number", "null"]})),
        ("request_budget", json!({"type": ["integer", "null"]})),
        ("rpm_limit", json!({"type": ["integer", "null"]})),
        ("tpm_limit", json!({"type": ["integer", "null"]})),
        ("usage", schema_ref("GatewayUsage")),
    ]));
    object_schema(
        &["name", "source", "disabled", "editable", "usage"],
        properties,
        None,
    )
}

fn gateway_usage_schema() -> Value {
    object_schema(
        &[
            "minute_epoch",
            "requests_this_minute",
            "tokens_this_minute",
            "requests_total",
            "spend_microusd",
            "spend_usd",
        ],
        schema_map([
            ("minute_epoch", json!({"type": "integer"})),
            ("requests_this_minute", json!({"type": "integer"})),
            ("tokens_this_minute", json!({"type": "integer"})),
            ("requests_total", json!({"type": "integer"})),
            ("spend_microusd", json!({"type": "integer"})),
            ("spend_usd", json!({"type": "number"})),
        ]),
        None,
    )
}

fn gateway_observability_schema() -> Value {
    object_schema(
        &["object", "sinks", "http_bearer_token_configured"],
        schema_map([
            ("object", json!({"type": "string"})),
            ("call_id_header", json!({"type": ["string", "null"]})),
            ("sinks", array_schema(json!({"type": "string"}))),
            ("jsonl_path", json!({"type": ["string", "null"]})),
            ("http_endpoint", json!({"type": ["string", "null"]})),
            ("http_schema", json!({"type": "string"})),
            ("http_bearer_token_configured", json!({"type": "boolean"})),
        ]),
        None,
    )
}

fn gateway_providers_schema() -> Value {
    object_schema(
        &["object", "providers"],
        schema_map([
            ("object", json!({"type": "string"})),
            (
                "providers",
                array_schema(schema_ref("GatewayProviderContract")),
            ),
        ]),
        None,
    )
}

fn gateway_provider_contract_schema() -> Value {
    object_schema(
        &[
            "provider",
            "client_request_format",
            "upstream_request_format",
            "response_format",
            "canonical_client_endpoint",
            "model_list_endpoint",
            "supports_streaming",
            "supports_model_fallback",
            "supported_endpoints",
            "model_count",
            "replay_case_count",
        ],
        schema_map([
            ("provider", json!({"type": "string"})),
            ("client_request_format", json!({"type": "string"})),
            ("upstream_request_format", json!({"type": "string"})),
            ("response_format", json!({"type": "string"})),
            ("canonical_client_endpoint", json!({"type": "string"})),
            ("model_list_endpoint", json!({"type": "string"})),
            ("supports_streaming", json!({"type": "boolean"})),
            ("supports_model_fallback", json!({"type": "boolean"})),
            (
                "supported_endpoints",
                array_schema(json!({"type": "string"})),
            ),
            ("model_count", json!({"type": "integer"})),
            ("replay_case_count", json!({"type": "integer"})),
        ]),
        None,
    )
}

fn gateway_guardrails_schema() -> Value {
    let webhook = object_schema(
        &[
            "configured",
            "phases",
            "bearer_token_configured",
            "fail_closed",
        ],
        schema_map([
            ("configured", json!({"type": "boolean"})),
            ("phases", array_schema(json!({"type": "string"}))),
            ("bearer_token_configured", json!({"type": "boolean"})),
            ("fail_closed", json!({"type": "boolean"})),
        ]),
        None,
    );
    object_schema(
        &[
            "object",
            "blocked_keywords_count",
            "blocked_output_keywords_count",
            "allowed_models",
            "prompt_injection_detection",
            "pii_redaction",
            "webhook",
        ],
        schema_map([
            ("object", json!({"type": "string"})),
            ("blocked_keywords_count", json!({"type": "integer"})),
            ("blocked_output_keywords_count", json!({"type": "integer"})),
            ("allowed_models", array_schema(json!({"type": "string"}))),
            ("prompt_injection_detection", json!({"type": "boolean"})),
            ("pii_redaction", json!({"type": "boolean"})),
            ("webhook", webhook),
        ]),
        None,
    )
}

fn gateway_key_deleted_schema() -> Value {
    object_schema(
        &["object", "name", "deleted"],
        schema_map([
            ("object", json!({"type": "string"})),
            ("name", json!({"type": "string"})),
            ("deleted", json!({"type": "boolean"})),
        ]),
        None,
    )
}
