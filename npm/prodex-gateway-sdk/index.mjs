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
    return this.request("/v1/responses", {
      method: "POST",
      body,
      signal: options.signal,
    });
  }

  async listKeys(options = {}) {
    return this.request("/v1/prodex/gateway/keys", { signal: options.signal });
  }

  async createKey(body, options = {}) {
    return this.request("/v1/prodex/gateway/keys", {
      method: "POST",
      body,
      signal: options.signal,
    });
  }

  async getKey(name, options = {}) {
    return this.request(`/v1/prodex/gateway/keys/${encodeURIComponent(name)}`, {
      signal: options.signal,
    });
  }

  async updateKey(name, body, options = {}) {
    return this.request(`/v1/prodex/gateway/keys/${encodeURIComponent(name)}`, {
      method: "PATCH",
      body,
      signal: options.signal,
    });
  }

  async deleteKey(name, options = {}) {
    return this.request(`/v1/prodex/gateway/keys/${encodeURIComponent(name)}`, {
      method: "DELETE",
      signal: options.signal,
    });
  }

  async listScimUsers(options = {}) {
    return this.request("/v1/prodex/gateway/scim/v2/Users", { signal: options.signal });
  }

  async createScimUser(body, options = {}) {
    return this.request("/v1/prodex/gateway/scim/v2/Users", {
      method: "POST",
      body,
      signal: options.signal,
    });
  }

  async getScimUser(id, options = {}) {
    return this.request(`/v1/prodex/gateway/scim/v2/Users/${encodeURIComponent(id)}`, {
      signal: options.signal,
    });
  }

  async updateScimUser(id, body, options = {}) {
    return this.request(`/v1/prodex/gateway/scim/v2/Users/${encodeURIComponent(id)}`, {
      method: options.method ?? "PATCH",
      body,
      signal: options.signal,
    });
  }

  async deleteScimUser(id, options = {}) {
    return this.request(`/v1/prodex/gateway/scim/v2/Users/${encodeURIComponent(id)}`, {
      method: "DELETE",
      signal: options.signal,
    });
  }

  async usage(options = {}) {
    return this.request("/v1/prodex/gateway/usage", { signal: options.signal });
  }

  async ledger(options = {}) {
    return this.request("/v1/prodex/gateway/ledger", { signal: options.signal });
  }

  async ledgerCsv(options = {}) {
    return this.request("/v1/prodex/gateway/ledger.csv", {
      accept: "text/csv",
      parse: "text",
      signal: options.signal,
    });
  }

  async billingSummary(options = {}) {
    return this.request("/v1/prodex/gateway/ledger/summary", { signal: options.signal });
  }

  async billingSummaryCsv(options = {}) {
    return this.request("/v1/prodex/gateway/ledger/summary.csv", {
      accept: "text/csv",
      parse: "text",
      signal: options.signal,
    });
  }

  async openapi(options = {}) {
    return this.request("/v1/prodex/gateway/openapi.json", { signal: options.signal });
  }

  async metrics(options = {}) {
    return this.request("/v1/prodex/gateway/metrics", {
      accept: "text/plain",
      parse: "text",
      signal: options.signal,
    });
  }

  async observability(options = {}) {
    return this.request("/v1/prodex/gateway/observability", { signal: options.signal });
  }

  async guardrails(options = {}) {
    return this.request("/v1/prodex/gateway/guardrails", { signal: options.signal });
  }

  async providers(options = {}) {
    return this.request("/v1/prodex/gateway/providers", { signal: options.signal });
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
