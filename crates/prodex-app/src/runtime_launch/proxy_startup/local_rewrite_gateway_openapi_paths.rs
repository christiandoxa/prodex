mod billing;

use billing::insert_runtime_gateway_billing_openapi_paths;

pub(super) fn runtime_gateway_openapi_paths_for_mount(
    mount_path: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let responses_path = format!("{mount_path}/responses");
    let keys_path = format!("{mount_path}/prodex/gateway/keys");
    let key_path = format!("{keys_path}/{{name}}");
    let scim_users_path = format!("{mount_path}/prodex/gateway/scim/v2/Users");
    let scim_user_path = format!("{scim_users_path}/{{id}}");
    let usage_path = format!("{mount_path}/prodex/gateway/usage");
    let metrics_path = format!("{mount_path}/prodex/gateway/metrics");
    let providers_path = format!("{mount_path}/prodex/gateway/providers");
    let observability_path = format!("{mount_path}/prodex/gateway/observability");
    let guardrails_path = format!("{mount_path}/prodex/gateway/guardrails");
    let openapi_path = format!("{mount_path}/prodex/gateway/openapi.json");
    let admin_path = format!("{mount_path}/prodex/gateway/admin");

    let idempotency_key_header = serde_json::json!({
        "name": "Idempotency-Key",
        "in": "header",
        "required": false,
        "description": "Optional printable ASCII idempotency key for admin mutations. Reusing the same method/path/key returns 409.",
        "schema": {"type": "string", "maxLength": 200}
    });
    let if_match_header = serde_json::json!({
        "name": "If-Match",
        "in": "header",
        "required": false,
        "description": "Optional optimistic-concurrency guard. Use the current ETag from GET, or * to bypass the guard.",
        "schema": {"type": "string"}
    });

    let mut paths = serde_json::Map::new();
    for (path, operation_id, summary) in [
        (
            "/livez",
            "getGatewayLivez",
            "Check that the local gateway process is alive",
        ),
        (
            "/readyz",
            "getGatewayReadyz",
            "Check that the gateway is ready to admit local traffic",
        ),
        (
            "/startupz",
            "getGatewayStartupz",
            "Check that the gateway startup path completed",
        ),
    ] {
        paths.insert(
            path.to_string(),
            serde_json::json!({
                "get": {
                    "operationId": operation_id,
                    "summary": summary,
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "Gateway operational health snapshot",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/GatewayHealth"}
                                }
                            }
                        },
                        "503": {
                            "description": "Gateway is alive but not locally ready",
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/GatewayHealth"}
                                }
                            }
                        },
                        "405": {
                            "description": "Health probe method is not allowed",
                            "headers": {
                                "Allow": {
                                    "schema": {"type": "string"}
                                }
                            },
                            "content": {
                                "application/json": {
                                    "schema": {"$ref": "#/components/schemas/GatewayHealth"}
                                }
                            }
                        }
                    }
                },
                "head": {
                    "operationId": operation_id.replace("get", "head"),
                    "summary": summary,
                    "security": [],
                    "responses": {
                        "200": {"description": "Gateway operational health snapshot without a body"},
                        "503": {"description": "Gateway is alive but not locally ready"}
                    }
                }
            }),
        );
    }
    paths.insert(
        responses_path,
        serde_json::json!({
            "post": {
                "operationId": "createGatewayResponse",
                "summary": "Create an OpenAI-compatible response through the Prodex gateway",
                "security": [{"GatewayBearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {"type": "object", "additionalProperties": true}
                        }
                    }
                },
                "responses": {
                    "200": {
                        "description": "OpenAI-compatible response payload or stream"
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "429": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        keys_path,
        serde_json::json!({
            "get": {
                "operationId": "listGatewayVirtualKeys",
                "summary": "List gateway virtual keys and usage counters",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway virtual key list",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyList"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "post": {
                "operationId": "createGatewayVirtualKey",
                "parameters": [idempotency_key_header.clone()],
                "summary": "Create an admin-managed gateway virtual key",
                "security": [{"GatewayBearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/GatewayKeyCreateRequest"}
                        }
                    }
                },
                "responses": {
                    "201": {
                        "description": "Created virtual key. token is returned once when generated by Prodex.",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyMutationResponse"}
                            }
                        }
                    },
                    "400": {"$ref": "#/components/responses/GatewayError"},
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "409": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        key_path,
        serde_json::json!({
            "parameters": [
                {
                    "name": "name",
                    "in": "path",
                    "required": true,
                    "schema": {"type": "string"}
                }
            ],
            "get": {
                "operationId": "getGatewayVirtualKey",
                "summary": "Get one gateway virtual key",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway virtual key",
                        "headers": {
                            "ETag": {
                                "description": "Opaque virtual-key version token for If-Match concurrency guards.",
                                "schema": {"type": "string"}
                            }
                        },
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyResponse"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "patch": {
                "operationId": "updateGatewayVirtualKey",
                "parameters": [idempotency_key_header.clone(), if_match_header.clone()],
                "summary": "Update, disable, or rotate an admin-managed gateway virtual key",
                "security": [{"GatewayBearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/GatewayKeyPatchRequest"}
                        }
                    }
                },
                "responses": {
                    "200": {
                        "description": "Updated virtual key. token is returned once when rotated.",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyMutationResponse"}
                            }
                        }
                    },
                    "400": {"$ref": "#/components/responses/GatewayError"},
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"},
                    "412": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "delete": {
                "operationId": "deleteGatewayVirtualKey",
                "parameters": [idempotency_key_header.clone(), if_match_header.clone()],
                "summary": "Delete an admin-managed gateway virtual key",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Deleted virtual key marker",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyDeleted"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"},
                    "412": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        scim_users_path,
        serde_json::json!({
            "get": {
                "operationId": "listGatewayScimUsers",
                "summary": "List gateway SCIM-provisioned admin users",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "SCIM ListResponse of users",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayScimUserList"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "post": {
                "operationId": "createGatewayScimUser",
                "parameters": [idempotency_key_header.clone()],
                "summary": "Provision a gateway admin user for trusted-proxy SSO",
                "security": [{"GatewayBearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {"$ref": "#/components/schemas/GatewayScimUserWrite"}
                        }
                    }
                },
                "responses": {
                    "201": {
                        "description": "Created SCIM user",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayScimUser"}
                            }
                        }
                    },
                    "400": {"$ref": "#/components/responses/GatewayError"},
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "409": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        scim_user_path,
        serde_json::json!({
            "parameters": [
                {
                    "name": "id",
                    "in": "path",
                    "required": true,
                    "schema": {"type": "string"}
                }
            ],
            "get": {
                "operationId": "getGatewayScimUser",
                "summary": "Get one gateway SCIM user",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "SCIM user",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayScimUser"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "patch": {
                "operationId": "patchGatewayScimUser",
                "parameters": [idempotency_key_header.clone()],
                "summary": "Patch one gateway SCIM user",
                "security": [{"GatewayBearerAuth": []}],
                "requestBody": {
                    "required": true,
                    "content": {
                        "application/json": {
                            "schema": {"type": "object", "additionalProperties": true}
                        }
                    }
                },
                "responses": {
                    "200": {
                        "description": "Updated SCIM user",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayScimUser"}
                            }
                        }
                    },
                    "400": {"$ref": "#/components/responses/GatewayError"},
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"},
                    "409": {"$ref": "#/components/responses/GatewayError"}
                }
            },
            "delete": {
                "operationId": "deleteGatewayScimUser",
                "parameters": [idempotency_key_header.clone()],
                "summary": "Delete one gateway SCIM user",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {"description": "Deleted SCIM user marker"},
                    "401": {"$ref": "#/components/responses/GatewayError"},
                    "403": {"$ref": "#/components/responses/GatewayError"},
                    "404": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        usage_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayVirtualKeyUsage",
                "summary": "List gateway virtual keys with persisted usage counters",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway virtual key usage",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayKeyList"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    insert_runtime_gateway_billing_openapi_paths(&mut paths, mount_path);
    paths.insert(
        metrics_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayPrometheusMetrics",
                "summary": "Get gateway virtual-key usage metrics in Prometheus text format",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Prometheus text metrics",
                        "content": {
                            "text/plain": {
                                "schema": {"type": "string"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        providers_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayProviders",
                "summary": "Get gateway provider adapter contract matrix",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway provider adapter contracts",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayProviders"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        observability_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayObservability",
                "summary": "Get active gateway observability configuration",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway observability configuration",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayObservability"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        guardrails_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayGuardrails",
                "summary": "Get active gateway guardrail configuration",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway guardrail configuration",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayGuardrails"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        openapi_path,
        serde_json::json!({
            "get": {
                "operationId": "getProdexGatewayOpenApi",
                "summary": "Get the Prodex gateway OpenAPI document",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "OpenAPI document",
                        "content": {
                            "application/json": {
                                "schema": {"type": "object", "additionalProperties": true}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        admin_path,
        serde_json::json!({
            "get": {
                "operationId": "getProdexGatewayAdminDashboard",
                "summary": "Get the authenticated built-in Prodex gateway admin dashboard shell",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Authenticated HTML dashboard shell for gateway administration.",
                        "content": {
                            "text/html": {
                                "schema": {"type": "string"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );

    paths
}
