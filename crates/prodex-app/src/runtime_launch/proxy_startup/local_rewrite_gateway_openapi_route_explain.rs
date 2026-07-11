pub(super) fn runtime_gateway_openapi_route_explain_schemas()
-> serde_json::Map<String, serde_json::Value> {
    serde_json::json!({
        "GatewayRouteDecisionStage": {
            "type": "string",
            "enum": [
                "affinity",
                "model_resolution",
                "endpoint_capability",
                "request_constraints",
                "governance",
                "authentication",
                "quota",
                "circuit_and_backoff",
                "admission",
                "ranking",
                "final_selection"
            ]
        },
        "GatewayRouteDecisionReason": {
            "type": "string",
            "pattern": "^[a-z0-9_]+$",
            "description": "Stable snake_case reason label from the canonical versioned route-decision trace."
        },
        "GatewayRouteExplainRequest": {
            "type": "object",
            "required": ["endpoint", "requested_model"],
            "properties": {
                "endpoint": {
                    "type": "string",
                    "enum": ["responses", "responses/compact", "chat-completions", "messages", "embeddings"]
                },
                "requested_model": {"$ref": "#/components/schemas/GatewayRouteModelIdentifier"},
                "request": {
                    "type": "object",
                    "description": "Optional request shape used only for pure planning. Prompt and tool content are never returned or audited.",
                    "additionalProperties": true
                },
                "required_capabilities": {
                    "type": "array",
                    "maxItems": 16,
                    "items": {"type": "string", "enum": ["tools", "json_schema", "vision", "audio", "web_search", "reasoning", "streaming", "compact", "websocket"]}
                },
                "include_current_state": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include the bounded global model-load snapshot. Available only to unscoped principals; scoped callers receive 403."
                },
                "diagnostic_seed": {"type": "integer", "minimum": 0, "default": 1},
                "hard_affinity_required": {"type": "boolean", "default": false},
                "owner_model": {"$ref": "#/components/schemas/GatewayNullableRouteModelIdentifier"},
                "policy": {
                    "oneOf": [
                        {"$ref": "#/components/schemas/GatewayRouteConstraintPolicy"},
                        {"type": "null"}
                    ]
                }
            },
            "additionalProperties": false
        },
        "GatewayRouteConstraintPolicy": {
            "type": "object",
            "properties": {
                "enabled": {"type": "boolean", "default": false},
                "unknown_context": {"type": "string", "enum": ["allow", "safe_window", "reject"], "default": "allow"},
                "safe_window_tokens": {"type": "integer", "minimum": 1, "default": 128000},
                "oversized_output": {"type": "string", "enum": ["passthrough", "reject", "clamp_with_notice"], "default": "passthrough"}
            },
            "additionalProperties": false
        },
        "GatewayRouteRequestRequirements": {
            "type": "object",
            "required": [
                "endpoint",
                "requested_model",
                "resolved_upstream_model",
                "estimated_input_tokens",
                "explicit_output_tokens",
                "output_limit_field",
                "default_output_reserve_tokens",
                "reasoning_effort",
                "reasoning_reserve_tokens",
                "total_required_tokens",
                "required_features"
            ],
            "properties": {
                "endpoint": {
                    "type": "string",
                    "enum": ["responses", "responses/compact", "chat-completions", "messages", "embeddings"]
                },
                "requested_model": {"$ref": "#/components/schemas/GatewayRouteModelIdentifier"},
                "resolved_upstream_model": {"$ref": "#/components/schemas/GatewayNullableRouteModelIdentifier"},
                "estimated_input_tokens": {"type": "integer", "minimum": 0},
                "explicit_output_tokens": {"type": ["integer", "null"], "minimum": 0},
                "output_limit_field": {
                    "type": ["string", "null"],
                    "enum": ["max_output_tokens", "max_completion_tokens", "max_tokens", null]
                },
                "default_output_reserve_tokens": {"type": ["integer", "null"], "minimum": 0},
                "reasoning_effort": {
                    "type": ["string", "null"],
                    "enum": ["none", "minimal", "low", "medium", "high", "xhigh", "unknown", null]
                },
                "reasoning_reserve_tokens": {"type": ["integer", "null"], "minimum": 0},
                "total_required_tokens": {"type": "integer", "minimum": 0},
                "required_features": {
                    "type": "array",
                    "maxItems": 16,
                    "items": {"type": "string", "enum": ["tools", "json_schema", "vision", "audio", "web_search", "reasoning", "streaming", "compact", "websocket"]}
                }
            },
            "additionalProperties": false
        },
        "GatewayRouteOutputAdjustment": {
            "type": "object",
            "required": ["field", "requested_tokens", "applied_tokens", "reason"],
            "properties": {
                "field": {"type": "string", "enum": ["max_output_tokens", "max_completion_tokens", "max_tokens"]},
                "requested_tokens": {"type": "integer", "minimum": 0},
                "applied_tokens": {"type": "integer", "minimum": 0},
                "reason": {"$ref": "#/components/schemas/GatewayRouteDecisionReason"}
            },
            "additionalProperties": false
        },
        "GatewayRouteCandidateDecision": {
            "type": "object",
            "required": [
                "candidate_id",
                "original_order",
                "provider",
                "model",
                "hard_affinity",
                "class",
                "eligibility",
                "rejection_stage",
                "reason",
                "selected",
                "quota_band",
                "circuit_state",
                "health_band",
                "inflight_count",
                "diagnostics"
            ],
            "properties": {
                "candidate_id": {"type": "string"},
                "original_order": {"type": "integer", "minimum": 0},
                "provider": {"type": ["string", "null"]},
                "model": {"type": ["string", "null"]},
                "hard_affinity": {"type": "boolean"},
                "class": {"type": "string", "enum": ["affinity", "current", "ready", "fallback", "auto_redeem"]},
                "eligibility": {"type": "string", "enum": ["eligible", "rejected", "deferred", "not_evaluated"]},
                "rejection_stage": {
                    "anyOf": [
                        {"$ref": "#/components/schemas/GatewayRouteDecisionStage"},
                        {"type": "null"}
                    ]
                },
                "reason": {
                    "anyOf": [
                        {"$ref": "#/components/schemas/GatewayRouteDecisionReason"},
                        {"type": "null"}
                    ]
                },
                "selected": {"type": "boolean"},
                "quota_band": {"type": ["string", "null"], "enum": ["healthy", "thin", "critical", "exhausted", "unknown", null]},
                "circuit_state": {"type": ["string", "null"], "enum": ["closed", "open", "half_open_wait", "unknown", null]},
                "health_band": {"type": ["string", "null"], "enum": ["healthy", "penalized", "unknown", null]},
                "inflight_count": {"type": ["integer", "null"], "minimum": 0},
                "diagnostics": {"$ref": "#/components/schemas/GatewayRouteDecisionDiagnostics"}
            },
            "additionalProperties": false
        },
        "GatewayRouteDecisionDiagnostics": {
            "type": "object",
            "properties": {
                "estimated_input_tokens": {"type": "integer", "minimum": 0},
                "output_tokens": {"type": "integer", "minimum": 0},
                "reasoning_reserve_tokens": {"type": "integer", "minimum": 0},
                "total_required_tokens": {"type": "integer", "minimum": 0},
                "available_context_tokens": {"type": "integer", "minimum": 0},
                "max_output_tokens": {"type": "integer", "minimum": 0},
                "requested_output_tokens": {"type": "integer", "minimum": 0},
                "applied_output_tokens": {"type": "integer", "minimum": 0}
            },
            "additionalProperties": false
        },
        "GatewayRouteDecisionTrace": {
            "type": "object",
            "required": [
                "schema_version",
                "route",
                "requested_model",
                "resolved_model",
                "affinity",
                "stages",
                "candidates",
                "selected_candidate",
                "terminal_outcome",
                "terminal_reason",
                "commit_state",
                "truncation"
            ],
            "properties": {
                "schema_version": {"type": "integer", "minimum": 1},
                "route": {"type": "string", "enum": ["responses", "responses_compact", "chat_completions", "messages", "embeddings", "websocket", "standard"]},
                "requested_model": {
                    "anyOf": [
                        {"$ref": "#/components/schemas/GatewayRouteModelIdentifier"},
                        {"type": "null"}
                    ]
                },
                "resolved_model": {"$ref": "#/components/schemas/GatewayNullableRouteModelIdentifier"},
                "affinity": {
                    "type": "object",
                    "required": ["kind", "candidate_id", "hard", "outcome"],
                    "properties": {
                        "kind": {"type": "string", "enum": ["none", "strict", "previous_response", "turn_state", "session", "prompt_cache"]},
                        "candidate_id": {"type": ["string", "null"]},
                        "hard": {"type": "boolean"},
                        "outcome": {"type": "string", "enum": ["not_applicable", "retained", "rejected", "exhausted"]}
                    },
                    "additionalProperties": false
                },
                "stages": {
                    "type": "array",
                    "maxItems": 11,
                    "items": {
                        "type": "object",
                        "required": ["stage", "outcome"],
                        "properties": {
                            "stage": {"$ref": "#/components/schemas/GatewayRouteDecisionStage"},
                            "outcome": {"type": "string", "enum": ["passed", "skipped", "rejected", "selected"]}
                        },
                        "additionalProperties": false
                    }
                },
                "candidates": {
                    "type": "array",
                    "maxItems": 32,
                    "items": {"$ref": "#/components/schemas/GatewayRouteCandidateDecision"}
                },
                "selected_candidate": {"type": ["string", "null"]},
                "terminal_outcome": {"type": "string", "enum": ["selected", "no_candidate", "affinity_exhausted", "failed"]},
                "terminal_reason": {
                    "anyOf": [
                        {"$ref": "#/components/schemas/GatewayRouteDecisionReason"},
                        {"type": "null"}
                    ]
                },
                "commit_state": {"type": "string", "enum": ["pre_commit", "committed"]},
                "truncation": {
                    "type": "object",
                    "required": ["truncated", "omitted_stages", "omitted_candidate_records", "truncated_identifiers"],
                    "properties": {
                        "truncated": {"type": "boolean"},
                        "omitted_stages": {"type": "integer", "minimum": 0},
                        "omitted_candidate_records": {"type": "integer", "minimum": 0},
                        "truncated_identifiers": {"type": "integer", "minimum": 0}
                    },
                    "additionalProperties": false
                }
            },
            "additionalProperties": false
        },
        "GatewayRouteExplainResponse": {
            "type": "object",
            "required": [
                "object",
                "schema_version",
                "diagnostic_seed",
                "hard_affinity_required",
                "hard_affinity_applied",
                "owner_model",
                "requested_model",
                "resolved_model",
                "alias_resolution_chain",
                "concrete_candidate_models",
                "request_requirements",
                "candidate_matrix",
                "selected_candidate",
                "policy_adjustments",
                "current_load_included",
                "health_quota_included",
                "warnings",
                "final_no_route_reason",
                "truncated",
                "omitted_candidates",
                "trace"
            ],
            "properties": {
                "object": {"type": "string", "const": "gateway.route_explanation"},
                "schema_version": {"type": "integer", "minimum": 1},
                "diagnostic_seed": {"type": "integer", "minimum": 0},
                "hard_affinity_required": {"type": "boolean"},
                "hard_affinity_applied": {"type": "boolean"},
                "owner_model": {"$ref": "#/components/schemas/GatewayNullableRouteModelIdentifier"},
                "requested_model": {"$ref": "#/components/schemas/GatewayRouteModelIdentifier"},
                "resolved_model": {"$ref": "#/components/schemas/GatewayNullableRouteModelIdentifier"},
                "alias_resolution_chain": {"type": "array", "maxItems": 32, "items": {"$ref": "#/components/schemas/GatewayRouteModelIdentifier"}},
                "concrete_candidate_models": {"type": "array", "maxItems": 32, "items": {"$ref": "#/components/schemas/GatewayRouteModelIdentifier"}},
                "request_requirements": {"$ref": "#/components/schemas/GatewayRouteRequestRequirements"},
                "candidate_matrix": {
                    "type": "array",
                    "maxItems": 32,
                    "items": {"$ref": "#/components/schemas/GatewayRouteCandidateDecision"}
                },
                "selected_candidate": {"type": ["string", "null"]},
                "policy_adjustments": {"type": "array", "maxItems": 1, "items": {"$ref": "#/components/schemas/GatewayRouteOutputAdjustment"}},
                "current_load_included": {"type": "boolean", "description": "Whether the current read-only in-flight/model-load snapshot was included in diagnostic planning."},
                "health_quota_included": {"type": "boolean"},
                "warnings": {"type": "array", "maxItems": 64, "items": {"type": "string"}},
                "final_no_route_reason": {"type": ["string", "null"]},
                "truncated": {"type": "boolean"},
                "omitted_candidates": {"type": "integer", "minimum": 0},
                "trace": {"$ref": "#/components/schemas/GatewayRouteDecisionTrace"}
            },
            "additionalProperties": false
        }
    })
    .as_object()
    .cloned()
    .expect("route explain OpenAPI schemas must be an object")
}
