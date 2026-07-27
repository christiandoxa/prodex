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
    const init = {
      method: options.method ?? "GET",
      headers,
      redirect: "error",
      signal: options.signal,
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

    const response = await this.fetch(url, init);
    if (options.parse === "stream" && response.ok) {
      if (!response.body) {
        throw new ProdexGatewayError("Prodex gateway returned an empty response stream", {
          status: response.status,
        });
      }
      return response.body;
    }
    const responseBody = await readResponseBody(response, options.parse);
    if (!response.ok) {
      const error = responseBody?.error ?? {};
      throw new ProdexGatewayError(error.message ?? `Prodex gateway request failed with ${response.status}`, {
        status: response.status,
        code: error.code ?? null,
        responseBody,
      });
    }
    return responseBody;
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

async function readResponseBody(response, parse) {
  if (parse === "text") {
    return response.text();
  }
  const contentType = response.headers.get("content-type") ?? "";
  if (parse === "json" || contentType.includes("application/json")) {
    return response.json();
  }
  return response.text();
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
