#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");

const FILES = Object.freeze({
  root: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite.rs",
  pipeline: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline.rs",
  dispatch: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_pipeline_dispatch.rs",
  admission:
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_data_plane.rs",
  request: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_request.rs",
  appManifest: "crates/prodex-app/Cargo.toml",
});

const REMOVED_ENTERPRISE_DEPENDENCIES = Object.freeze([
  "prodex_application",
  "prodex_authz",
  "prodex_control_plane",
  "prodex_gateway_core",
  "prodex_gateway_http",
  "prodex_gateway_server",
  "prodex_storage",
  "prodex_storage_postgres",
  "prodex_storage_postgres_runtime",
  "prodex_storage_redis",
  "prodex_storage_redis_runtime",
  "prodex_storage_sqlite",
  "prodex_storage_sqlite_runtime",
]);

function functionBody(source, name) {
  const start = source.indexOf(`fn ${name}`);
  if (start < 0) return undefined;
  const open = source.indexOf("{", start);
  if (open < 0) return undefined;
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") depth -= 1;
    if (depth === 0) return source.slice(open + 1, index);
  }
  return undefined;
}

function requireText(errors, source, needle, message) {
  if (!source.includes(needle)) errors.push(message);
}

function forbidText(errors, source, needle, message) {
  if (source.includes(needle)) errors.push(message);
}

function requireOrdered(errors, source, needles, message) {
  let previous = -1;
  for (const needle of needles) {
    const index = source.indexOf(needle);
    if (index < 0 || index <= previous) {
      errors.push(message);
      return;
    }
    previous = index;
  }
}

export function validateProductionBoundary(sources) {
  const errors = [];
  const root = functionBody(sources.root, "handle_runtime_local_rewrite_proxy_request");
  if (!root) {
    errors.push(`${FILES.root}: compatibility gateway handler is missing`);
  } else {
    requireOrdered(
      errors,
      root,
      [
        "RuntimeLocalRewriteRequest::tiny(request)",
        "runtime_local_rewrite_request_target_valid(request.url())",
        "request.respond(build_runtime_proxy_json_error_response(",
        "run_runtime_local_rewrite_pipeline(request, target, shared)",
      ],
      `${FILES.root}: request wrapping, target validation, rejection, and pipeline delegation must remain ordered`,
    );
  }

  const pipeline = functionBody(sources.pipeline, "try_run_runtime_local_rewrite_pipeline");
  if (!pipeline) {
    errors.push(`${FILES.pipeline}: compatibility provider pipeline is missing`);
  } else {
    requireOrdered(
      errors,
      pipeline,
      [
        "runtime_local_rewrite_request_state(request, target, shared)",
        "runtime_local_rewrite_bounded_admission(state, shared)",
        "runtime_local_rewrite_capture_body(state, shared)",
        "RuntimeGatewayApplicationAdmission::from_request(&captured.captured, shared)",
        "runtime_local_rewrite_dispatch_websocket(ready, shared)",
        "runtime_local_rewrite_dispatch_compact(ready, shared)",
        "runtime_local_rewrite_dispatch_builtin_models(ready, shared)",
        "runtime_local_rewrite_dispatch_provider(ready, shared)",
      ],
      `${FILES.pipeline}: bounded admission and capture must precede provider dispatch`,
    );
    for (const removed of ["control_plane", "virtual_key", "governance", "billing", "scim"]) {
      forbidText(
        errors,
        pipeline,
        removed,
        `${FILES.pipeline}: removed enterprise ${removed} path must not return to the compatibility pipeline`,
      );
    }
  }

  requireText(
    errors,
    sources.request,
    "runtime_local_rewrite_request_target_valid",
    `${FILES.request}: bounded canonical request-target validation is required`,
  );
  for (const rule of [
    "raw.len() > 8 * 1024",
    "!raw.is_ascii()",
    "raw.contains('#')",
    "path.contains('\\\\')",
    "path.contains(\"//\")",
    "matches!(s, \".\" | \"..\")",
    "matches!(decoded, b'/' | b'\\\\' | b'%' | b'?' | b'#' | b'.')",
  ]) {
    requireText(
      errors,
      sources.request,
      rule,
      `${FILES.request}: canonical request-target safety rule is missing: ${rule}`,
    );
  }

  requireText(
    errors,
    sources.admission,
    "provider_adapter(shared.provider.bridge_kind().provider_id()).capability_status(endpoint)",
    `${FILES.admission}: provider capability admission must remain authoritative`,
  );
  requireText(
    errors,
    sources.dispatch,
    "send_runtime_local_rewrite_upstream_request(",
    `${FILES.dispatch}: provider dispatch must reach the shared upstream transport`,
  );

  for (const dependency of REMOVED_ENTERPRISE_DEPENDENCIES) {
    forbidText(
      errors,
      sources.appManifest,
      dependency,
      `${FILES.appManifest}: removed enterprise dependency ${dependency} must not return`,
    );
  }
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`production-boundary-guard self-test failed: ${message}`);
}

function runSelfTest() {
  const valid = {
    root: `fn handle_runtime_local_rewrite_proxy_request() { RuntimeLocalRewriteRequest::tiny(request); runtime_local_rewrite_request_target_valid(request.url()); request.respond(build_runtime_proxy_json_error_response(400)); run_runtime_local_rewrite_pipeline(request, target, shared); }`,
    pipeline: `fn try_run_runtime_local_rewrite_pipeline() { runtime_local_rewrite_request_state(request, target, shared); runtime_local_rewrite_bounded_admission(state, shared); runtime_local_rewrite_capture_body(state, shared); RuntimeGatewayApplicationAdmission::from_request(&captured.captured, shared); runtime_local_rewrite_dispatch_websocket(ready, shared); runtime_local_rewrite_dispatch_compact(ready, shared); runtime_local_rewrite_dispatch_builtin_models(ready, shared); runtime_local_rewrite_dispatch_provider(ready, shared); }`,
    dispatch: `fn dispatch() { send_runtime_local_rewrite_upstream_request(); }`,
    admission: `fn admission() { provider_adapter(shared.provider.bridge_kind().provider_id()).capability_status(endpoint); }`,
    request: `fn runtime_local_rewrite_request_target_valid() { raw.len() > 8 * 1024; !raw.is_ascii(); raw.contains('#'); path.contains('\\\\'); path.contains("//"); matches!(s, "." | ".."); matches!(decoded, b'/' | b'\\\\' | b'%' | b'?' | b'#' | b'.'); }`,
    appManifest: `[dependencies]\nprodex_provider_core = { workspace = true }`,
  };
  assertSelfTest(validateProductionBoundary(valid).length === 0, "valid core data plane rejected");
  assertSelfTest(
    validateProductionBoundary({ ...valid, appManifest: `${valid.appManifest}\nprodex_storage = { workspace = true }` })
      .some((error) => error.includes("prodex_storage")),
    "removed enterprise dependency accepted",
  );
  assertSelfTest(
    validateProductionBoundary({ ...valid, pipeline: valid.pipeline.replace("runtime_local_rewrite_capture_body(state, shared);", "") })
      .some((error) => error.includes("bounded admission and capture")),
    "provider dispatch without bounded capture accepted",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    process.stdout.write("production boundary guard self-test: ok\n");
    return;
  }
  const entries = await Promise.all(
    Object.entries(FILES).map(async ([key, file]) => [
      key,
      await fs.readFile(path.join(repoRoot, file), "utf8"),
    ]),
  );
  const errors = validateProductionBoundary(Object.fromEntries(entries));
  for (const error of errors) process.stderr.write(`${error}\n`);
  if (errors.length > 0) {
    process.exitCode = 1;
  } else {
    process.stdout.write("production boundary guard: ok\n");
  }
}

main().catch((error) => {
  process.stderr.write(`production-boundary-guard: ${error.stack ?? error.message}\n`);
  process.exitCode = 1;
});
