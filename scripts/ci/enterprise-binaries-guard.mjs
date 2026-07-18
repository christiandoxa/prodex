#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const BINARIES = [
  {
    paths: ["src/bin/prodex-gateway.rs"],
    required: ["Data-plane gateway entrypoint", "DedicatedServerMode::DataPlane"],
  },
  {
    paths: [
      "src/bin/prodex-control-plane.rs",
      "src/bin/prodex_control_plane/mod.rs",
      "src/bin/prodex_control_plane/cli.rs",
      "src/bin/prodex_control_plane/http_plan.rs",
      "src/bin/prodex_control_plane/publication.rs",
    ],
    required: ["Control-plane entrypoint", "DedicatedServerMode::ControlPlane"],
  },
];
const SERVE_COMPOSITION_PATH = "src/enterprise_serve.rs";
const FORBIDDEN_PATTERNS = [
  { name: "business runtime module", pattern: /runtime_launch|runtime_proxy|local_rewrite/u },
  { name: "HTTP framework", pattern: /\b(axum|hyper|tower|tiny_http)\s*::/u },
  { name: "database driver", pattern: /\b(postgres|rusqlite|redis|sqlx)\s*::/u },
  { name: "provider SDK", pattern: /\b(openai|anthropic|gemini|copilot)\s*::/u },
];

function validateBinary(binary) {
  const errors = [];
  const sources = [];
  for (const sourcePath of binary.paths) {
    const filePath = path.join(repoRoot, sourcePath);
    if (!fs.existsSync(filePath)) {
      errors.push(`${sourcePath}: required enterprise binary source is missing`);
      continue;
    }
    sources.push({ path: sourcePath, source: fs.readFileSync(filePath, "utf8") });
  }
  const combinedSource = sources.map(({ source }) => source).join("\n");
  for (const phrase of binary.required) {
    if (!combinedSource.includes(phrase)) {
      errors.push(`${binary.paths[0]}: missing required entrypoint phrase '${phrase}'`);
    }
  }
  for (const { path: sourcePath, source } of sources) {
    source.split(/\r?\n/u).forEach((line, index) => {
      for (const forbidden of FORBIDDEN_PATTERNS) {
        if (forbidden.pattern.test(line)) {
          errors.push(`${sourcePath}:${index + 1}: binary entrypoint must stay thin; found ${forbidden.name} '${line.trim()}'`);
        }
      }
    });
  }
  return errors;
}

function validateServeComposition(source) {
  const errors = [];
  const applicationStart = "prodex_app::start_policy_gateway_application_for_mode(policy_mode)";
  const asyncFront = "serve_with_handler_reloadable(";
  const applicationDispatch = "application.handle(request).await";
  const applicationDrain = "application.shutdown_and_drain(drain_timeout)";
  for (const [needle, message] of [
    [
      applicationStart,
      `${SERVE_COMPOSITION_PATH}: dedicated serve must start the in-process gateway application`,
    ],
    [
      "GatewayServerConfig::production(listen_addr, server_mode)",
      `${SERVE_COMPOSITION_PATH}: public traffic must enter the route-isolated async server`,
    ],
    [
      applicationDispatch,
      `${SERVE_COMPOSITION_PATH}: async server must dispatch the exact handler request in process`,
    ],
    [
      applicationDrain,
      `${SERVE_COMPOSITION_PATH}: in-process application must drain after the async server stops`,
    ],
    [
      "applications.swap(candidate)",
      `${SERVE_COMPOSITION_PATH}: live reload must atomically replace the active application`,
    ],
    [
      "previous.shutdown_and_drain(drain_timeout)",
      `${SERVE_COMPOSITION_PATH}: live reload must drain the replaced application`,
    ],
  ]) {
    if (!source.includes(needle)) errors.push(message);
  }
  const start = source.indexOf(applicationStart);
  const serve = source.indexOf(asyncFront);
  const dispatch = source.indexOf(applicationDispatch);
  const drain = source.indexOf(applicationDrain);
  if (start < 0 || serve < 0 || drain < 0 || !(start < serve && serve < drain)) {
    errors.push(
      `${SERVE_COMPOSITION_PATH}: startup, route-isolated serving, and application drain must remain ordered`,
    );
  }
  if (dispatch < serve || dispatch > drain) {
    errors.push(`${SERVE_COMPOSITION_PATH}: in-process dispatch must remain inside the serve phase`);
  }
  if ((source.match(/start_policy_gateway_application_for_mode\s*\(/gu) ?? []).length !== 2) {
    errors.push(
      `${SERVE_COMPOSITION_PATH}: dedicated serve must own initial and live-reload application lifecycles`,
    );
  }
  for (const forbidden of [
    "start_policy_gateway_backend",
    "backend.listen_addr()",
    '"127.0.0.1:0"',
  ]) {
    if (source.includes(forbidden)) {
      errors.push(`${SERVE_COMPOSITION_PATH}: dedicated serve must not use loopback backend transport`);
    }
  }
  return errors;
}

function runSelfTest() {
  const bad = "fn main() { tiny_http::Server::http(\"127.0.0.1:0\"); }";
  if (!FORBIDDEN_PATTERNS.some((forbidden) => forbidden.pattern.test(bad))) {
    throw new Error("self-test failed: forbidden pattern did not match");
  }
  const validServe = `
    let application = prodex_app::start_policy_gateway_application_for_mode(policy_mode);
    let server_result = serve_with_handler_reloadable(
      GatewayServerConfig::production(listen_addr, server_mode),
      move |request| async move { application.handle(request).await },
    );
    application.shutdown_and_drain(drain_timeout);
    let candidate = prodex_app::start_policy_gateway_application_for_mode(policy_mode);
    let previous = applications.swap(candidate);
    previous.shutdown_and_drain(drain_timeout);
  `;
  if (validateServeComposition(validServe).length !== 0) {
    throw new Error("self-test failed: valid in-process serve composition was rejected");
  }
  const loopbackLegacy = validServe
    .replace("start_policy_gateway_application_for_mode(policy_mode)", 'start_policy_gateway_backend(Some("127.0.0.1:0".to_string()))')
    .replace("serve_with_handler_reloadable(", "serve(")
    .replace("move |request| async move { application.handle(request).await },", "backend.listen_addr(),");
  if (validateServeComposition(loopbackLegacy).length === 0) {
    throw new Error("self-test failed: loopback legacy backend was accepted");
  }
  const bypassedFront = validServe.replace(
    "let server_result = serve_with_handler_reloadable(",
    "let server_result = direct(",
  );
  if (validateServeComposition(bypassedFront).length === 0) {
    throw new Error("self-test failed: route-isolated async front bypass was accepted");
  }
}

function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  const serveSource = fs.readFileSync(path.join(repoRoot, SERVE_COMPOSITION_PATH), "utf8");
  const errors = [
    ...BINARIES.flatMap(validateBinary),
    ...validateServeComposition(serveSource),
  ];
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main();
