pub(super) fn runtime_gateway_openapi_components() -> serde_json::Value {
    serde_json::json!({
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
                        "schema": {"$ref": "#/components/schemas/GatewayError"}
                    }
                }
            }
        },
            "schemas": {
                "GatewayError": {
                    "type": "object",
                    "required": ["error"],
                    "properties": {
                        "error": {
                            "type": "object",
                            "required": ["code", "message"],
                            "properties": {
                                "code": {"type": "string"},
                                "message": {"type": "string"}
                            }
                        }
                    }
                },
                "GatewayKeyCreateRequest": {
                    "type": "object",
                    "required": ["name"],
                    "properties": {
                        "name": {"type": "string"},
                        "token": {
                            "type": "string",
                            "description": "Optional caller-supplied bearer token. Omit to have Prodex generate one."
                        },
                        "tenant_id": {"type": ["string", "null"]},
                        "team_id": {"type": ["string", "null"]},
                        "project_id": {"type": ["string", "null"]},
                        "user_id": {"type": ["string", "null"]},
                        "budget_id": {"type": ["string", "null"]},
                        "allowed_models": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "budget_microusd": {"type": ["integer", "null"], "minimum": 0},
                        "budget_usd": {"type": ["number", "null"], "minimum": 0},
                        "request_budget": {"type": ["integer", "null"], "minimum": 0},
                        "rpm_limit": {"type": ["integer", "null"], "minimum": 0},
                        "tpm_limit": {"type": ["integer", "null"], "minimum": 0},
                        "disabled": {"type": "boolean"}
                    },
                    "additionalProperties": false
                },
                "GatewayKeyPatchRequest": {
                    "type": "object",
                    "properties": {
                        "token": {
                            "type": "string",
                            "description": "Optional replacement bearer token. The stored file only keeps its hash."
                        },
                        "rotate": {
                            "type": "boolean",
                            "description": "Generate and return a new bearer token once."
                        },
                        "tenant_id": {"type": ["string", "null"]},
                        "team_id": {"type": ["string", "null"]},
                        "project_id": {"type": ["string", "null"]},
                        "user_id": {"type": ["string", "null"]},
                        "budget_id": {"type": ["string", "null"]},
                        "allowed_models": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "budget_microusd": {"type": ["integer", "null"], "minimum": 0},
                        "budget_usd": {"type": ["number", "null"], "minimum": 0},
                        "request_budget": {"type": ["integer", "null"], "minimum": 0},
                        "rpm_limit": {"type": ["integer", "null"], "minimum": 0},
                        "tpm_limit": {"type": ["integer", "null"], "minimum": 0},
                        "disabled": {"type": "boolean"}
                    },
                    "additionalProperties": false
                },
                "GatewayKeyMutationResponse": {
                    "type": "object",
                    "required": ["object", "key"],
                    "properties": {
                        "object": {"type": "string"},
                        "key": {"$ref": "#/components/schemas/GatewayKey"},
                        "token": {
                            "type": ["string", "null"],
                            "description": "Returned only when Prodex generated or rotated the token for this request."
                        }
                    }
                },
                "GatewayKeyResponse": {
                    "type": "object",
                    "required": ["object", "key"],
                    "properties": {
                        "object": {"type": "string"},
                        "key": {"$ref": "#/components/schemas/GatewayKey"}
                    }
                },
                "GatewayKeyList": {
                    "type": "object",
                    "required": ["object", "keys"],
                    "properties": {
                        "object": {"type": "string"},
                        "keys": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayKey"}
                        },
                        "state_backend": {"type": "string", "enum": ["file", "sqlite", "postgres", "redis"]},
                        "state_path": {"type": "string"},
                        "key_store_path": {"type": "string"},
                        "usage_path": {"type": ["string", "null"]},
                        "unknown_persisted_keys": {
                            "type": "array",
                            "items": {"type": "string"}
                        }
                    }
                },
                "GatewayScimUserList": {
                    "type": "object",
                    "required": ["schemas", "totalResults", "Resources"],
                    "properties": {
                        "schemas": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "totalResults": {"type": "integer"},
                        "startIndex": {"type": "integer"},
                        "itemsPerPage": {"type": "integer"},
                        "Resources": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayScimUser"}
                        }
                    }
                },
                "GatewayScimUserWrite": {
                    "type": "object",
                    "required": ["userName"],
                    "properties": {
                        "userName": {"type": "string"},
                        "externalId": {"type": ["string", "null"]},
                        "displayName": {"type": ["string", "null"]},
                        "active": {"type": "boolean"},
                        "tenant_id": {"type": ["string", "null"]},
                        "team_id": {"type": ["string", "null"]},
                        "project_id": {"type": ["string", "null"]},
                        "user_id": {"type": ["string", "null"]},
                        "budget_id": {"type": ["string", "null"]},
                        "role": {"type": ["string", "null"], "enum": ["admin", "viewer", null]},
                        "allowed_key_prefixes": {
                            "type": "array",
                            "items": {"type": "string"}
                        }
                    },
                    "additionalProperties": true
                },
                "GatewayScimUser": {
                    "type": "object",
                    "required": ["schemas", "id", "userName", "active"],
                    "properties": {
                        "schemas": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "id": {"type": "string"},
                        "userName": {"type": "string"},
                        "externalId": {"type": ["string", "null"]},
                        "displayName": {"type": ["string", "null"]},
                        "active": {"type": "boolean"},
                        "tenant_id": {"type": ["string", "null"]},
                        "team_id": {"type": ["string", "null"]},
                        "project_id": {"type": ["string", "null"]},
                        "user_id": {"type": ["string", "null"]},
                        "budget_id": {"type": ["string", "null"]},
                        "meta": {"type": "object", "additionalProperties": true}
                    },
                    "additionalProperties": true
                },
                "GatewayBillingLedger": {
                    "type": "object",
                    "required": ["object", "records"],
                    "properties": {
                        "object": {"type": "string"},
                        "state_backend": {"type": "string", "enum": ["file", "sqlite", "postgres", "redis"]},
                        "ledger_path": {"type": "string"},
                        "limit": {"type": "integer"},
                        "records": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingLedgerEntry"}
                        }
                    }
                },
                "GatewayBillingSummary": {
                    "type": "object",
                    "required": ["object", "totals", "by_key", "by_model", "by_key_model"],
                    "properties": {
                        "object": {"type": "string"},
                        "state_backend": {"type": "string", "enum": ["file", "sqlite", "postgres", "redis"]},
                        "ledger_path": {"type": "string"},
                        "record_count": {"type": "integer"},
                        "totals": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"},
                        "by_key": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"}
                        },
                        "by_model": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"}
                        },
                        "by_key_model": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"}
                        },
                        "by_team": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"}
                        },
                        "by_project": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"}
                        },
                        "by_user": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"}
                        },
                        "by_budget": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayBillingSummaryBucket"}
                        }
                    }
                },
                "GatewayBillingSummaryBucket": {
                    "type": "object",
                    "required": [
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
                        "final_cost_usd"
                    ],
                    "properties": {
                        "key_name": {"type": ["string", "null"]},
                        "model": {"type": ["string", "null"]},
                        "team_id": {"type": ["string", "null"]},
                        "project_id": {"type": ["string", "null"]},
                        "user_id": {"type": ["string", "null"]},
                        "budget_id": {"type": ["string", "null"]},
                        "requests": {"type": "integer"},
                        "successful_requests": {"type": "integer"},
                        "failed_requests": {"type": "integer"},
                        "unreconciled_requests": {"type": "integer"},
                        "input_tokens": {"type": "integer"},
                        "output_tokens": {"type": "integer"},
                        "response_bytes": {"type": "integer"},
                        "estimated_cost_microusd": {"type": "integer"},
                        "estimated_cost_usd": {"type": "number"},
                        "final_cost_microusd": {"type": "integer"},
                        "final_cost_usd": {"type": "number"},
                        "first_created_at_epoch": {"type": ["integer", "null"]},
                        "last_created_at_epoch": {"type": ["integer", "null"]},
                        "last_reconciled_at_epoch": {"type": ["integer", "null"]}
                    }
                },
                "GatewayBillingLedgerEntry": {
                    "type": "object",
                    "required": [
                        "object",
                        "phase",
                        "request",
                        "call_id",
                        "key_name",
                        "model",
                        "minute_epoch",
                        "input_tokens",
                        "created_at_epoch"
                    ],
                    "properties": {
                        "object": {"type": "string"},
                        "phase": {"type": "string", "enum": ["request"]},
                        "request": {"type": "integer"},
                        "call_id": {"type": "string"},
                        "key_name": {"type": "string"},
                        "model": {"type": "string"},
                        "minute_epoch": {"type": "integer"},
                        "input_tokens": {"type": "integer"},
                        "estimated_cost_microusd": {"type": ["integer", "null"]},
                        "estimated_cost_usd": {"type": ["number", "null"]},
                        "created_at_epoch": {"type": "integer"},
                        "response_status": {"type": ["integer", "null"]},
                        "response_bytes": {"type": ["integer", "null"]},
                        "output_tokens": {"type": ["integer", "null"]},
                        "final_cost_microusd": {"type": ["integer", "null"]},
                        "final_cost_usd": {"type": ["number", "null"]},
                        "reconciled_at_epoch": {"type": ["integer", "null"]}
                    }
                },
                "GatewayKey": {
                    "type": "object",
                    "required": ["name", "source", "disabled", "editable", "usage"],
                    "properties": {
                        "name": {"type": "string"},
                        "tenant_id": {"type": ["string", "null"]},
                        "team_id": {"type": ["string", "null"]},
                        "project_id": {"type": ["string", "null"]},
                        "user_id": {"type": ["string", "null"]},
                        "budget_id": {"type": ["string", "null"]},
                        "source": {"type": "string", "enum": ["policy", "admin"]},
                        "disabled": {"type": "boolean"},
                        "editable": {"type": "boolean"},
                        "created_at_epoch": {"type": ["integer", "null"]},
                        "updated_at_epoch": {"type": ["integer", "null"]},
                        "allowed_models": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "budget_microusd": {"type": ["integer", "null"]},
                        "budget_usd": {"type": ["number", "null"]},
                        "request_budget": {"type": ["integer", "null"]},
                        "rpm_limit": {"type": ["integer", "null"]},
                        "tpm_limit": {"type": ["integer", "null"]},
                        "usage": {"$ref": "#/components/schemas/GatewayUsage"}
                    }
                },
                "GatewayUsage": {
                    "type": "object",
                    "required": [
                        "minute_epoch",
                        "requests_this_minute",
                        "tokens_this_minute",
                        "requests_total",
                        "spend_microusd",
                        "spend_usd"
                    ],
                    "properties": {
                        "minute_epoch": {"type": "integer"},
                        "requests_this_minute": {"type": "integer"},
                        "tokens_this_minute": {"type": "integer"},
                        "requests_total": {"type": "integer"},
                        "spend_microusd": {"type": "integer"},
                        "spend_usd": {"type": "number"}
                    }
                },
                "GatewayObservability": {
                    "type": "object",
                    "required": ["object", "sinks", "http_bearer_token_configured"],
                    "properties": {
                        "object": {"type": "string"},
                        "call_id_header": {"type": ["string", "null"]},
                        "sinks": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "jsonl_path": {"type": ["string", "null"]},
                        "http_endpoint": {"type": ["string", "null"]},
                        "http_schema": {"type": "string"},
                        "http_bearer_token_configured": {"type": "boolean"}
                    }
                },
                "GatewayProviders": {
                    "type": "object",
                    "required": ["object", "providers"],
                    "properties": {
                        "object": {"type": "string"},
                        "providers": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/GatewayProviderContract"}
                        }
                    }
                },
                "GatewayProviderContract": {
                    "type": "object",
                    "required": [
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
                        "replay_case_count"
                    ],
                    "properties": {
                        "provider": {"type": "string"},
                        "client_request_format": {"type": "string"},
                        "upstream_request_format": {"type": "string"},
                        "response_format": {"type": "string"},
                        "canonical_client_endpoint": {"type": "string"},
                        "model_list_endpoint": {"type": "string"},
                        "supports_streaming": {"type": "boolean"},
                        "supports_model_fallback": {"type": "boolean"},
                        "supported_endpoints": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "model_count": {"type": "integer"},
                        "replay_case_count": {"type": "integer"}
                    }
                },
                "GatewayGuardrails": {
                    "type": "object",
                    "required": [
                        "object",
                        "blocked_keywords_count",
                        "blocked_output_keywords_count",
                        "allowed_models",
                        "prompt_injection_detection",
                        "pii_redaction",
                        "webhook"
                    ],
                    "properties": {
                        "object": {"type": "string"},
                        "blocked_keywords_count": {"type": "integer"},
                        "blocked_output_keywords_count": {"type": "integer"},
                        "allowed_models": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "prompt_injection_detection": {"type": "boolean"},
                        "pii_redaction": {"type": "boolean"},
                        "webhook": {
                            "type": "object",
                            "required": ["configured", "phases", "bearer_token_configured", "fail_closed"],
                            "properties": {
                                "configured": {"type": "boolean"},
                                "phases": {
                                    "type": "array",
                                    "items": {"type": "string"}
                                },
                                "bearer_token_configured": {"type": "boolean"},
                                "fail_closed": {"type": "boolean"}
                            }
                        }
                    }
                },
                "GatewayKeyDeleted": {
                    "type": "object",
                    "required": ["object", "name", "deleted"],
                    "properties": {
                        "object": {"type": "string"},
                        "name": {"type": "string"},
                        "deleted": {"type": "boolean"}
                    }
                }
            }
    })
}
