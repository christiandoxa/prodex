# Prodex Gateway SDK

JavaScript client for `prodex gateway`.

```js
import { ProdexGatewayClient } from "@christiandoxa/prodex-gateway-sdk";

const gateway = new ProdexGatewayClient({
  baseUrl: "http://127.0.0.1:4000",
  token: process.env.PRODEX_GATEWAY_TOKEN,
});

const created = await gateway.createKey({ name: "team-a", budget_usd: 10 });
const response = await gateway.createResponse({
  model: "prodex-fast",
  input: "hello",
});
const ledger = await gateway.ledger();
const summary = await gateway.billingSummary();
const summaryCsv = await gateway.billingSummaryCsv();
const metrics = await gateway.metrics();
const observability = await gateway.observability();
const guardrails = await gateway.guardrails();
const providers = await gateway.providers();
```

The client covers the OpenAI-compatible `/v1/responses` gateway path plus Prodex admin endpoints for virtual keys, SCIM users, usage, billing ledger records and summaries, CSV exports, Prometheus metrics, provider contract discovery, observability and guardrail config, and OpenAPI discovery.
