import assert from "node:assert/strict";
import test from "node:test";
import { ProdexGatewayClient, ProdexGatewayError } from "../index.mjs";

test("createResponse returns the live body for streaming requests", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      return new Response("data: {\"type\":\"response.completed\"}\n\n", {
        status: 200,
        headers: { "content-type": "text/event-stream" },
      });
    },
  });

  const stream = await client.createResponse(
    { model: "prodex-fast", input: "hello", stream: true },
    { stream: true },
  );

  assert.equal(calls[0].init.headers.get("accept"), "text/event-stream");
  assert.match(await new Response(stream).text(), /response\.completed/);
});

test("createKey sends bearer JSON request", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    baseUrl: "http://127.0.0.1:4000",
    token: "admin-token",
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      return jsonResponse({ object: "gateway.key", key: { name: "team-a" }, token: "pk-test" }, 201);
    },
  });

  const result = await client.createKey({ name: "team-a", budget_usd: 1.5 });

  assert.equal(result.token, "pk-test");
  assert.equal(calls[0].url, "http://127.0.0.1:4000/v1/prodex/gateway/keys");
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.headers.get("authorization"), "Bearer admin-token");
  assert.equal(calls[0].init.headers.get("content-type"), "application/json");
  assert.equal(calls[0].init.redirect, "error");
  assert.deepEqual(JSON.parse(calls[0].init.body), { name: "team-a", budget_usd: 1.5 });
});

test("request rejects cross-origin targets before attaching credentials", async () => {
  let calls = 0;
  const client = new ProdexGatewayClient({
    baseUrl: "https://gateway.example.com",
    token: "admin-token",
    fetch: async () => {
      calls += 1;
      return jsonResponse({});
    },
  });

  await assert.rejects(
    () => client.request("https://attacker.example/collect"),
    /must stay on the configured origin/,
  );
  await assert.rejects(
    () => client.request("//attacker.example/collect"),
    /must stay on the configured origin/,
  );
  assert.equal(calls, 0);
});

test("constructor rejects unsafe base URLs", () => {
  for (const baseUrl of [
    "file:///tmp/prodex",
    "https://user:secret@gateway.example.com",
    "https://gateway.example.com?token=secret",
  ]) {
    assert.throws(() => new ProdexGatewayClient({ baseUrl }), /must be an HTTP\(S\) origin/);
  }
});

test("metrics requests text format", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      return new Response("prodex_gateway_virtual_key_requests_total 1\n", {
        status: 200,
        headers: { "content-type": "text/plain" },
      });
    },
  });

  const body = await client.metrics();

  assert.match(body, /prodex_gateway_virtual_key_requests_total/);
  assert.equal(calls[0].url, "http://127.0.0.1:4000/v1/prodex/gateway/metrics");
  assert.equal(calls[0].init.headers.get("accept"), "text/plain");
});

test("observability and guardrails helpers read config surfaces", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      if (String(url).endsWith("/observability")) {
        return jsonResponse({
          object: "gateway.observability",
          sinks: ["runtime-log"],
          http_schema: "generic",
          http_bearer_token_configured: false,
        });
      }
      return jsonResponse({
        object: "gateway.guardrails",
        blocked_keywords_count: 1,
        blocked_output_keywords_count: 0,
        allowed_models: ["prodex-fast"],
        prompt_injection_detection: true,
        pii_redaction: true,
        webhook: {
          configured: true,
          phases: ["pre", "post"],
          bearer_token_configured: true,
          fail_closed: false,
        },
      });
    },
  });

  const observability = await client.observability();
  const guardrails = await client.guardrails();

  assert.deepEqual(observability.sinks, ["runtime-log"]);
  assert.equal(guardrails.webhook.configured, true);
  assert.equal(guardrails.pii_redaction, true);
  assert.equal(calls[0].url, "http://127.0.0.1:4000/v1/prodex/gateway/observability");
  assert.equal(calls[1].url, "http://127.0.0.1:4000/v1/prodex/gateway/guardrails");
});

test("providers helper reads adapter contract matrix", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      return jsonResponse({
        object: "gateway.providers",
        providers: [
          {
            provider: "openai",
            client_request_format: "openai-responses",
            upstream_request_format: "openai-responses",
            response_format: "openai-responses",
            canonical_client_endpoint: "/v1/responses",
            model_list_endpoint: "/v1/models",
            supports_streaming: true,
            supports_model_fallback: false,
            supported_endpoints: ["responses", "models"],
            model_count: 4,
            replay_case_count: 1,
          },
        ],
      });
    },
  });

  const providers = await client.providers();

  assert.equal(providers.providers[0].provider, "openai");
  assert.equal(providers.providers[0].client_request_format, "openai-responses");
  assert.equal(providers.providers[0].replay_case_count, 1);
  assert.equal(calls[0].url, "http://127.0.0.1:4000/v1/prodex/gateway/providers");
});

test("SCIM user helpers send bearer JSON requests", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    token: "admin-token",
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      return jsonResponse({
        schemas: ["urn:ietf:params:scim:schemas:core:2.0:User"],
        id: "user-1",
        userName: "alice@example.com",
        active: true,
      }, init.method === "POST" ? 201 : 200);
    },
  });

  const created = await client.createScimUser({
    userName: "alice@example.com",
    active: true,
    "urn:prodex:params:scim:schemas:gateway:2.0:User": {
      role: "admin",
      team_id: "team-a",
      budget_id: "budget-a",
      allowed_key_prefixes: ["team-a-"],
    },
  });
  const updated = await client.updateScimUser("user-1", {
    Operations: [{ op: "replace", path: "active", value: false }],
  });

  assert.equal(created.id, "user-1");
  assert.equal(updated.userName, "alice@example.com");
  assert.equal(calls[0].init.body.includes('"team_id":"team-a"'), true);
  assert.equal(calls[0].init.body.includes('"budget_id":"budget-a"'), true);
  assert.equal(calls[0].url, "http://127.0.0.1:4000/v1/prodex/gateway/scim/v2/Users");
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.headers.get("authorization"), "Bearer admin-token");
  assert.equal(JSON.parse(calls[0].init.body).userName, "alice@example.com");
  assert.equal(calls[1].url, "http://127.0.0.1:4000/v1/prodex/gateway/scim/v2/Users/user-1");
  assert.equal(calls[1].init.method, "PATCH");
  assert.deepEqual(JSON.parse(calls[1].init.body).Operations[0], {
    op: "replace",
    path: "active",
    value: false,
  });
});

test("SCIM list and delete helpers target Users endpoints", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    token: "admin-token",
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      if (init.method === "DELETE") {
        return jsonResponse({ object: "gateway.scim_user.deleted", id: "user-1", deleted: true });
      }
      return jsonResponse({ schemas: [], totalResults: 0, Resources: [] });
    },
  });

  const listed = await client.listScimUsers();
  const deleted = await client.deleteScimUser("user-1");

  assert.equal(listed.totalResults, 0);
  assert.equal(deleted.deleted, true);
  assert.equal(calls[0].url, "http://127.0.0.1:4000/v1/prodex/gateway/scim/v2/Users");
  assert.equal(calls[0].init.method, "GET");
  assert.equal(calls[1].url, "http://127.0.0.1:4000/v1/prodex/gateway/scim/v2/Users/user-1");
  assert.equal(calls[1].init.method, "DELETE");
});

test("ledger reads billing records", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    token: "admin-token",
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      return jsonResponse({
        object: "gateway.billing_ledger",
        records: [{ call_id: "prodex-1", key_name: "team-a", response_status: 200, output_tokens: 11 }],
      });
    },
  });

  const ledger = await client.ledger();

  assert.equal(ledger.records[0].call_id, "prodex-1");
  assert.equal(ledger.records[0].response_status, 200);
  assert.equal(ledger.records[0].output_tokens, 11);
  assert.equal(calls[0].url, "http://127.0.0.1:4000/v1/prodex/gateway/ledger");
  assert.equal(calls[0].init.headers.get("authorization"), "Bearer admin-token");
});

test("ledgerCsv exports billing records as text", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    token: "admin-token",
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      return textResponse("call_id,key_name\nprodex-1,team-a\n", 200, "text/csv");
    },
  });

  const csv = await client.ledgerCsv();

  assert.match(csv, /call_id,key_name/);
  assert.equal(calls[0].url, "http://127.0.0.1:4000/v1/prodex/gateway/ledger.csv");
  assert.equal(calls[0].init.headers.get("accept"), "text/csv");
});

test("billingSummary reads aggregated billing totals", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    token: "admin-token",
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      return jsonResponse({
        object: "gateway.billing_summary",
        record_count: 1,
        totals: { requests: 1, final_cost_usd: 0.001 },
        by_key: [{ key_name: "team-a", requests: 1 }],
        by_model: [{ model: "gpt-5.4", requests: 1 }],
        by_key_model: [{ key_name: "team-a", model: "gpt-5.4", requests: 1 }],
      });
    },
  });

  const summary = await client.billingSummary();

  assert.equal(summary.totals.requests, 1);
  assert.equal(summary.by_key_model[0].model, "gpt-5.4");
  assert.equal(calls[0].url, "http://127.0.0.1:4000/v1/prodex/gateway/ledger/summary");
  assert.equal(calls[0].init.headers.get("authorization"), "Bearer admin-token");
});

test("billingSummaryCsv exports aggregated billing totals as text", async () => {
  const calls = [];
  const client = new ProdexGatewayClient({
    token: "admin-token",
    fetch: async (url, init) => {
      calls.push({ url: String(url), init });
      return textResponse("group,key_name,requests\ntotals,,1\n", 200, "text/csv");
    },
  });

  const csv = await client.billingSummaryCsv();

  assert.match(csv, /group,key_name,requests/);
  assert.equal(calls[0].url, "http://127.0.0.1:4000/v1/prodex/gateway/ledger/summary.csv");
  assert.equal(calls[0].init.headers.get("accept"), "text/csv");
});

test("gateway JSON errors throw ProdexGatewayError", async () => {
  const client = new ProdexGatewayClient({
    fetch: async () =>
      jsonResponse(
        { error: { code: "invalid_admin_token", message: "missing or invalid gateway admin bearer token" } },
        401,
      ),
  });

  await assert.rejects(
    () => client.listKeys(),
    (error) => {
      assert.ok(error instanceof ProdexGatewayError);
      assert.equal(error.status, 401);
      assert.equal(error.code, "invalid_admin_token");
      assert.match(error.message, /missing or invalid/);
      return true;
    },
  );
});

function jsonResponse(body, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function textResponse(body, status = 200, contentType = "text/plain") {
  return new Response(body, {
    status,
    headers: { "content-type": contentType },
  });
}
