export interface ProdexGatewayClientOptions {
  baseUrl?: string;
  token?: string;
  fetch?: typeof fetch;
}

export interface ProdexGatewayRequestOptions {
  signal?: AbortSignal;
  headers?: HeadersInit;
}

export interface GatewayKeyMutationOptions extends ProdexGatewayRequestOptions {}

export interface GatewayKeyCreateRequest {
  name: string;
  token?: string;
  tenant_id?: string | null;
  team_id?: string | null;
  project_id?: string | null;
  user_id?: string | null;
  budget_id?: string | null;
  allowed_models?: string[];
  budget_microusd?: number | null;
  budget_usd?: number | null;
  request_budget?: number | null;
  rpm_limit?: number | null;
  tpm_limit?: number | null;
  disabled?: boolean;
}

export interface GatewayKeyPatchRequest extends Partial<Omit<GatewayKeyCreateRequest, "name">> {
  rotate?: boolean;
}

export interface GatewayUsage {
  minute_epoch: number;
  requests_this_minute: number;
  tokens_this_minute: number;
  requests_total: number;
  spend_microusd: number;
  spend_usd: number;
}

export interface GatewayKey {
  name: string;
  tenant_id?: string | null;
  team_id?: string | null;
  project_id?: string | null;
  user_id?: string | null;
  budget_id?: string | null;
  source: "policy" | "admin" | string;
  disabled: boolean;
  editable: boolean;
  created_at_epoch: number | null;
  updated_at_epoch: number | null;
  allowed_models: string[];
  budget_microusd: number | null;
  budget_usd: number | null;
  request_budget: number | null;
  rpm_limit: number | null;
  tpm_limit: number | null;
  usage: GatewayUsage;
}

export interface GatewayKeyList {
  object: string;
  keys: GatewayKey[];
  state_backend?: "file" | "sqlite" | string;
  state_path?: string;
  key_store_path?: string;
  usage_path?: string | null;
  unknown_persisted_keys?: string[];
}

export interface GatewayBillingLedgerEntry {
  object: string;
  phase: "request" | string;
  request: number;
  call_id: string;
  key_name: string;
  model: string;
  minute_epoch: number;
  input_tokens: number;
  estimated_cost_microusd: number | null;
  estimated_cost_usd: number | null;
  created_at_epoch: number;
  response_status?: number | null;
  response_bytes?: number | null;
  output_tokens?: number | null;
  final_cost_microusd?: number | null;
  final_cost_usd?: number | null;
  reconciled_at_epoch?: number | null;
}

export interface GatewayBillingLedger {
  object: string;
  state_backend?: "file" | "sqlite" | string;
  ledger_path?: string;
  limit?: number;
  records: GatewayBillingLedgerEntry[];
}

export interface GatewayBillingSummaryBucket {
  key_name?: string | null;
  model?: string | null;
  team_id?: string | null;
  project_id?: string | null;
  user_id?: string | null;
  budget_id?: string | null;
  requests: number;
  successful_requests: number;
  failed_requests: number;
  unreconciled_requests: number;
  input_tokens: number;
  output_tokens: number;
  response_bytes: number;
  estimated_cost_microusd: number;
  estimated_cost_usd: number;
  final_cost_microusd: number;
  final_cost_usd: number;
  first_created_at_epoch?: number | null;
  last_created_at_epoch?: number | null;
  last_reconciled_at_epoch?: number | null;
}

export interface GatewayBillingSummary {
  object: string;
  state_backend?: "file" | "sqlite" | string;
  ledger_path?: string;
  record_count: number;
  totals: GatewayBillingSummaryBucket;
  by_key: GatewayBillingSummaryBucket[];
  by_model: GatewayBillingSummaryBucket[];
  by_key_model: GatewayBillingSummaryBucket[];
  by_team?: GatewayBillingSummaryBucket[];
  by_project?: GatewayBillingSummaryBucket[];
  by_user?: GatewayBillingSummaryBucket[];
  by_budget?: GatewayBillingSummaryBucket[];
}

export interface GatewayObservability {
  object: string;
  call_id_header?: string | null;
  sinks: string[];
  jsonl_path?: string | null;
  http_endpoint?: string | null;
  http_schema: string;
  http_bearer_token_configured: boolean;
}

export interface GatewayGuardrails {
  object: string;
  blocked_keywords_count: number;
  blocked_output_keywords_count: number;
  allowed_models: string[];
  prompt_injection_detection: boolean;
  pii_redaction: boolean;
  webhook: {
    configured: boolean;
    phases: string[];
    bearer_token_configured: boolean;
    fail_closed: boolean;
  };
}

export interface GatewayProviderContract {
  provider: string;
  client_request_format: string;
  upstream_request_format: string;
  response_format: string;
  canonical_client_endpoint: string;
  model_list_endpoint: string;
  supports_streaming: boolean;
  supports_model_fallback: boolean;
  supported_endpoints: string[];
  model_count: number;
  replay_case_count: number;
}

export interface GatewayProviders {
  object: string;
  providers: GatewayProviderContract[];
}

export interface GatewayKeyResponse {
  object: string;
  key: GatewayKey;
  token?: string | null;
}

export interface GatewayKeyDeleted {
  object: string;
  name: string;
  deleted: boolean;
}

export interface GatewayScimProdexUserExtension {
  tenant_id?: string | null;
  team_id?: string | null;
  project_id?: string | null;
  user_id?: string | null;
  budget_id?: string | null;
  role?: "admin" | "viewer" | string | null;
  allowed_key_prefixes?: string[];
}

export interface GatewayScimUserWrite {
  userName: string;
  externalId?: string | null;
  displayName?: string | null;
  active?: boolean;
  tenant_id?: string | null;
  team_id?: string | null;
  project_id?: string | null;
  user_id?: string | null;
  budget_id?: string | null;
  role?: "admin" | "viewer" | string | null;
  allowed_key_prefixes?: string[];
  "urn:prodex:params:scim:schemas:gateway:2.0:User"?: GatewayScimProdexUserExtension;
}

export interface GatewayScimPatchOperation {
  op?: "add" | "replace" | "remove" | string;
  path?: string;
  value?: unknown;
}

export interface GatewayScimPatchRequest {
  schemas?: string[];
  Operations?: GatewayScimPatchOperation[];
  operations?: GatewayScimPatchOperation[];
  [key: string]: unknown;
}

export interface GatewayScimUser {
  schemas: string[];
  id: string;
  userName: string;
  externalId?: string | null;
  displayName?: string | null;
  active: boolean;
  tenant_id?: string | null;
  team_id?: string | null;
  project_id?: string | null;
  user_id?: string | null;
  budget_id?: string | null;
  meta?: Record<string, unknown>;
  "urn:prodex:params:scim:schemas:gateway:2.0:User"?: GatewayScimProdexUserExtension;
}

export interface GatewayScimUserList {
  schemas: string[];
  totalResults: number;
  startIndex?: number;
  itemsPerPage?: number;
  Resources: GatewayScimUser[];
}

export interface GatewayScimUserDeleted {
  object: string;
  id: string;
  deleted: boolean;
}

export class ProdexGatewayError extends Error {
  status: number;
  code: string | null;
  responseBody: unknown;
}

export class ProdexGatewayClient {
  constructor(options?: ProdexGatewayClientOptions);
  createResponse<T = unknown>(body: unknown, options?: ProdexGatewayRequestOptions): Promise<T>;
  listKeys(options?: ProdexGatewayRequestOptions): Promise<GatewayKeyList>;
  createKey(body: GatewayKeyCreateRequest, options?: GatewayKeyMutationOptions): Promise<GatewayKeyResponse>;
  getKey(name: string, options?: ProdexGatewayRequestOptions): Promise<GatewayKeyResponse>;
  updateKey(name: string, body: GatewayKeyPatchRequest, options?: GatewayKeyMutationOptions): Promise<GatewayKeyResponse>;
  deleteKey(name: string, options?: ProdexGatewayRequestOptions): Promise<GatewayKeyDeleted>;
  listScimUsers(options?: ProdexGatewayRequestOptions): Promise<GatewayScimUserList>;
  createScimUser(body: GatewayScimUserWrite, options?: ProdexGatewayRequestOptions): Promise<GatewayScimUser>;
  getScimUser(id: string, options?: ProdexGatewayRequestOptions): Promise<GatewayScimUser>;
  updateScimUser(
    id: string,
    body: GatewayScimPatchRequest | Partial<GatewayScimUserWrite>,
    options?: ProdexGatewayRequestOptions & { method?: "PATCH" | "PUT" | string },
  ): Promise<GatewayScimUser>;
  deleteScimUser(id: string, options?: ProdexGatewayRequestOptions): Promise<GatewayScimUserDeleted>;
  usage(options?: ProdexGatewayRequestOptions): Promise<GatewayKeyList>;
  ledger(options?: ProdexGatewayRequestOptions): Promise<GatewayBillingLedger>;
  ledgerCsv(options?: ProdexGatewayRequestOptions): Promise<string>;
  billingSummary(options?: ProdexGatewayRequestOptions): Promise<GatewayBillingSummary>;
  billingSummaryCsv(options?: ProdexGatewayRequestOptions): Promise<string>;
  openapi<T = unknown>(options?: ProdexGatewayRequestOptions): Promise<T>;
  metrics(options?: ProdexGatewayRequestOptions): Promise<string>;
  observability(options?: ProdexGatewayRequestOptions): Promise<GatewayObservability>;
  guardrails(options?: ProdexGatewayRequestOptions): Promise<GatewayGuardrails>;
  providers(options?: ProdexGatewayRequestOptions): Promise<GatewayProviders>;
  request<T = unknown>(
    path: string,
    options?: ProdexGatewayRequestOptions & {
      method?: string;
      body?: unknown;
      accept?: string;
      parse?: "json" | "text";
    },
  ): Promise<T>;
}
