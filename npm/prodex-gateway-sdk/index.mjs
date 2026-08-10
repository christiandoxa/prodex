const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_MAX_RESPONSE_BYTES = 8 * 1024 * 1024;

export class ProdexGatewayError extends Error {
  constructor(message, options = {}) {
    super(message);
    this.name = "ProdexGatewayError";
    this.status = options.status ?? 0;
    this.code = options.code ?? null;
    this.responseBody = options.responseBody ?? null;
  }
}

export class ProdexGatewayClient {
  constructor(options = {}) {
    this.baseUrl = normalizeBaseUrl(options.baseUrl ?? "http://127.0.0.1:4000");
    this.token = options.token ?? null;
    this.fetch = options.fetch ?? globalThis.fetch;
    this.timeoutMs = positiveInteger(options.timeoutMs ?? DEFAULT_TIMEOUT_MS, "timeoutMs");
    this.maxResponseBytes = positiveInteger(
      options.maxResponseBytes ?? DEFAULT_MAX_RESPONSE_BYTES,
      "maxResponseBytes",
    );
    if (typeof this.fetch !== "function") {
      throw new TypeError("ProdexGatewayClient requires fetch; pass options.fetch on this runtime");
    }
  }

  async createResponse(body, options = {}) {
    const stream = options.stream ?? body?.stream === true;
    return this.request("/v1/responses", {
      ...options,
      method: "POST",
      body,
      accept: stream ? "text/event-stream" : undefined,
      parse: stream ? "stream" : undefined,
    });
  }

  async listKeys(options = {}) {
    return this.request("/v1/prodex/gateway/keys", options);
  }

  async createKey(body, options = {}) {
    return this.request("/v1/prodex/gateway/keys", {
      ...mutationOptions(options),
      method: "POST",
      body,
    });
  }

  async getKey(name, options = {}) {
    return this.request(`/v1/prodex/gateway/keys/${encodeURIComponent(name)}`, options);
  }

  async updateKey(name, body, options = {}) {
    return this.request(`/v1/prodex/gateway/keys/${encodeURIComponent(name)}`, {
      ...mutationOptions(options),
      method: "PATCH",
      body,
    });
  }

  async deleteKey(name, options = {}) {
    return this.request(`/v1/prodex/gateway/keys/${encodeURIComponent(name)}`, {
      ...mutationOptions(options),
      method: "DELETE",
    });
  }

  async listScimUsers(options = {}) {
    return this.request("/v1/prodex/gateway/scim/v2/Users", options);
  }

  async createScimUser(body, options = {}) {
    return this.request("/v1/prodex/gateway/scim/v2/Users", {
      ...mutationOptions(options),
      method: "POST",
      body,
    });
  }

  async getScimUser(id, options = {}) {
    return this.request(`/v1/prodex/gateway/scim/v2/Users/${encodeURIComponent(id)}`, options);
  }

  async updateScimUser(id, body, options = {}) {
    return this.request(`/v1/prodex/gateway/scim/v2/Users/${encodeURIComponent(id)}`, {
      ...mutationOptions(options),
      method: options.method ?? "PATCH",
      body,
    });
  }

  async deleteScimUser(id, options = {}) {
    return this.request(`/v1/prodex/gateway/scim/v2/Users/${encodeURIComponent(id)}`, {
      ...mutationOptions(options),
      method: "DELETE",
    });
  }

  async usage(options = {}) {
    return this.request("/v1/prodex/gateway/usage", options);
  }

  async ledger(options = {}) {
    return this.request("/v1/prodex/gateway/ledger", options);
  }

  async ledgerCsv(options = {}) {
    return this.request("/v1/prodex/gateway/ledger.csv", {
      ...options,
      accept: "text/csv",
      parse: "text",
    });
  }

  async billingSummary(options = {}) {
    return this.request("/v1/prodex/gateway/ledger/summary", options);
  }

  async billingSummaryCsv(options = {}) {
    return this.request("/v1/prodex/gateway/ledger/summary.csv", {
      ...options,
      accept: "text/csv",
      parse: "text",
    });
  }

  async openapi(options = {}) {
    return this.request("/v1/prodex/gateway/openapi.json", options);
  }

  async metrics(options = {}) {
    return this.request("/v1/prodex/gateway/metrics", {
      ...options,
      accept: "text/plain",
      parse: "text",
    });
  }

  async observability(options = {}) {
    return this.request("/v1/prodex/gateway/observability", options);
  }

  async guardrails(options = {}) {
    return this.request("/v1/prodex/gateway/guardrails", options);
  }

  async providers(options = {}) {
    return this.request("/v1/prodex/gateway/providers", options);
  }

  async request(path, options = {}) {
    const url = new URL(path, this.baseUrl);
    if (url.origin !== this.baseUrl.origin) {
      throw new TypeError("ProdexGatewayClient requests must stay on the configured origin");
    }
    const headers = new Headers(options.headers ?? {});
    if (this.token && !headers.has("authorization")) {
      headers.set("authorization", `Bearer ${this.token}`);
    }
    if (options.accept && !headers.has("accept")) {
      headers.set("accept", options.accept);
    }
    const timeoutMs = positiveInteger(options.timeoutMs ?? this.timeoutMs, "timeoutMs");
    const maxResponseBytes = positiveInteger(
      options.maxResponseBytes ?? this.maxResponseBytes,
      "maxResponseBytes",
    );
    const deadline = options.signal ? null : new AbortController();
    const timeout = deadline
      ? setTimeout(() => deadline.abort(), timeoutMs)
      : null;
    const init = {
      method: options.method ?? "GET",
      headers,
      redirect: "error",
      signal: options.signal ?? deadline.signal,
    };
    if (options.body !== undefined) {
      if (!headers.has("content-type")) {
        headers.set("content-type", "application/json");
      }
      init.body =
        typeof options.body === "string" || options.body instanceof Uint8Array
          ? options.body
          : JSON.stringify(options.body);
    }

    try {
      const response = await this.fetch(url, init);
      if (options.parse === "stream" && response.ok) {
        if (!response.body) {
          throw new ProdexGatewayError("Prodex gateway returned an empty response stream", {
            status: response.status,
          });
        }
        if (timeout !== null) clearTimeout(timeout);
        return response.body;
      }
      const responseBody = await readResponseBody(response, options.parse, maxResponseBytes);
      if (!response.ok) {
        const error = responseBody?.error ?? {};
        throw new ProdexGatewayError(error.message ?? `Prodex gateway request failed with ${response.status}`, {
          status: response.status,
          code: error.code ?? null,
          responseBody,
        });
      }
      return responseBody;
    } catch (error) {
      if (deadline?.signal.aborted) {
        throw new ProdexGatewayError(`Prodex gateway request timed out after ${timeoutMs}ms`, {
          code: "request_timeout",
        });
      }
      throw error;
    } finally {
      if (timeout !== null) clearTimeout(timeout);
    }
  }
}

function mutationOptions(options) {
  const headers = new Headers(options.headers ?? {});
  const idempotencyKey = headers.get("idempotency-key")?.trim() || options.idempotencyKey?.trim();
  if (!idempotencyKey) {
    throw new TypeError(
      "Prodex gateway mutations require options.idempotencyKey or an Idempotency-Key header",
    );
  }
  headers.set("idempotency-key", idempotencyKey);
  return { ...options, headers };
}

async function readResponseBody(response, parse, maxBytes) {
  const text = await readResponseText(response, maxBytes);
  const contentType = response.headers.get("content-type") ?? "";
  if (parse === "json" || contentType.includes("application/json")) {
    return JSON.parse(text);
  }
  return text;
}

async function readResponseText(response, maxBytes) {
  const contentLength = Number(response.headers.get("content-length"));
  if (Number.isFinite(contentLength) && contentLength > maxBytes) {
    throw responseTooLarge(response.status, maxBytes);
  }
  if (!response.body) return "";
  const reader = response.body.getReader();
  const chunks = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > maxBytes) {
        void reader.cancel().catch(() => {});
        throw responseTooLarge(response.status, maxBytes);
      }
      chunks.push(value);
    }
  } finally {
    reader.releaseLock();
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return new TextDecoder().decode(bytes);
}

function responseTooLarge(status, maxBytes) {
  return new ProdexGatewayError(`Prodex gateway response exceeded ${maxBytes} bytes`, {
    status,
    code: "response_too_large",
  });
}

function positiveInteger(value, name) {
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new TypeError(`ProdexGatewayClient ${name} must be a positive integer`);
  }
  return value;
}

function normalizeBaseUrl(value) {
  const url = new URL(value);
  if (
    !["http:", "https:"].includes(url.protocol) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  ) {
    throw new TypeError("ProdexGatewayClient baseUrl must be an HTTP(S) origin without credentials");
  }
  url.pathname = url.pathname.replace(/\/+$/, "/");
  return url;
}
