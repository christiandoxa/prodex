//! Billing ledger OpenAPI path definitions for the local gateway admin surface.

pub(super) fn insert_runtime_gateway_billing_openapi_paths(
    paths: &mut serde_json::Map<String, serde_json::Value>,
    mount_path: &str,
) {
    let ledger_path = format!("{mount_path}/prodex/gateway/ledger");
    let ledger_csv_path = format!("{ledger_path}.csv");
    let ledger_summary_path = format!("{ledger_path}/summary");
    let ledger_summary_csv_path = format!("{ledger_summary_path}.csv");

    paths.insert(
        ledger_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayBillingLedger",
                "summary": "Get recent gateway virtual-key billing ledger records",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway billing ledger records",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayBillingLedger"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        ledger_csv_path,
        serde_json::json!({
            "get": {
                "operationId": "exportGatewayBillingLedgerCsv",
                "summary": "Export recent gateway virtual-key billing ledger records as CSV",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway billing ledger CSV export",
                        "content": {
                            "text/csv": {
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
        ledger_summary_path,
        serde_json::json!({
            "get": {
                "operationId": "getGatewayBillingSummary",
                "summary": "Get aggregated gateway virtual-key billing totals",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway billing summary grouped by key and model",
                        "content": {
                            "application/json": {
                                "schema": {"$ref": "#/components/schemas/GatewayBillingSummary"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
    paths.insert(
        ledger_summary_csv_path,
        serde_json::json!({
            "get": {
                "operationId": "exportGatewayBillingSummaryCsv",
                "summary": "Export aggregated gateway virtual-key billing totals as CSV",
                "security": [{"GatewayBearerAuth": []}],
                "responses": {
                    "200": {
                        "description": "Gateway billing summary CSV export",
                        "content": {
                            "text/csv": {
                                "schema": {"type": "string"}
                            }
                        }
                    },
                    "401": {"$ref": "#/components/responses/GatewayError"}
                }
            }
        }),
    );
}
