#!/usr/bin/env node
import fs from "node:fs";

const REQUIRED_KINDS = Object.freeze([
  "Deployment",
  "ServiceAccount",
  "Service",
  "ConfigMap",
  "ExternalSecret",
  "PodDisruptionBudget",
  "HorizontalPodAutoscaler",
  "NetworkPolicy",
  "ServiceMonitor",
  "Job",
]);

const COMPOSE_PROJECTED_SECRETS = Object.freeze([
  ["prodex_gateway_token", "PRODEX_GATEWAY_TOKEN", "PRODEX_GATEWAY_TOKEN_FILE"],
  ["openai_api_key", "OPENAI_API_KEY", "OPENAI_API_KEY_FILE"],
  ["prodex_gateway_postgres_url", "PRODEX_GATEWAY_POSTGRES_URL", "PRODEX_GATEWAY_POSTGRES_URL_FILE"],
  ["prodex_gateway_redis_url", "PRODEX_GATEWAY_REDIS_URL", "PRODEX_GATEWAY_REDIS_URL_FILE"],
]);

const FORBIDDEN_COMPOSE_SECRET_ENV = Object.freeze([
  "PRODEX_GATEWAY_TOKEN",
  "PRODEX_GATEWAY_SSO_PROXY_TOKEN",
  "PRODEX_GATEWAY_POSTGRES_URL",
  "PRODEX_GATEWAY_REDIS_URL",
  "OPENAI_API_KEY",
  "GEMINI_API_KEY",
  "ANTHROPIC_API_KEY",
  "DEEPSEEK_API_KEY",
  "GITHUB_COPILOT_API_KEY",
  "POSTGRES_PASSWORD",
]);

const COMPOSE_SECRET_FILE_ENVIRONMENTS = Object.freeze([
  ...COMPOSE_PROJECTED_SECRETS.map(([, , fileEnvironment]) => fileEnvironment),
  "PRODEX_POSTGRES_PASSWORD_FILE",
]);

const REQUIRED_GATEWAY_KUBERNETES_MARKERS = Object.freeze([
  ["runAsNonRoot: true", "non-root pod security context"],
  ["serviceAccountName: prodex-gateway", "explicit gateway service account"],
  ["automountServiceAccountToken: false", "disabled service account token automount"],
  ["seccompProfile:", "explicit seccomp profile"],
  ["type: RuntimeDefault", "RuntimeDefault seccomp profile"],
  ["readOnlyRootFilesystem: true", "read-only container filesystem"],
  ['drop: ["ALL"]', "dropped Linux capabilities"],
  ["allowPrivilegeEscalation: false", "disabled privilege escalation"],
  ["resources:", "resource requests and limits"],
  ["readinessProbe:", "readiness probe"],
  ["livenessProbe:", "liveness probe"],
  ["startupProbe:", "startup probe"],
  ["path: /readyz", "readiness endpoint"],
  ["path: /livez", "liveness endpoint"],
  ["path: /startupz", "startup endpoint"],
  ["topologySpreadConstraints:", "gateway pod topology spreading"],
  ["topologyKey: topology.kubernetes.io/zone", "zone topology spread key"],
  ["topologyKey: kubernetes.io/hostname", "node topology spread key"],
  ["whenUnsatisfiable: DoNotSchedule", "hard node spread scheduling"],
  ["terminationGracePeriodSeconds: 45", "gateway termination grace period"],
  ["preStop:", "gateway pre-stop drain hook"],
  ['command: ["sh", "-c", "sleep 15"]', "gateway pre-stop drain delay"],
  ["@sha256:", "immutable image digest"],
  ['command: ["prodex-gateway", "migrate"]', "explicit gateway migration command"],
  ['"--backend"', "gateway migration backend flag"],
  ['"postgres"', "gateway migration postgres backend"],
  ['"--url-ref"', "gateway migration projected URL reference flag"],
  ['"--secret-provider"', "gateway migration projected secret provider flag"],
  ['"--secret-root"', "gateway migration projected secret root flag"],
  ['"--tls-mode"', "gateway migration TLS mode flag"],
  ['"verify-full"', "gateway migration certificate and hostname verification"],
  ["name: prodex-gateway-migration", "migration job and network policy surface"],
  ["serviceAccountName: prodex-gateway-migration", "explicit migration service account"],
  ["name: prodex-gateway-migration-secrets", "migration-only database secret"],
  ["app.kubernetes.io/name: prodex-gateway-migration", "migration pod network policy label"],
  ["PRODEX_GATEWAY_REPLICA_COUNT", "gateway replica count configuration"],
  ["PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS", "multi-replica accounting gate flag"],
  ["PRODEX_GATEWAY_REDIS_URL", "Redis coordination/cache secret"],
  ["key: prodex/gateway/redis-url", "Redis secret backend reference"],
  ["PRODEX_GATEWAY_METRICS_TOKEN", "dedicated metrics viewer token secret"],
  ["key: prodex/gateway/metrics-token", "metrics viewer token backend reference"],
  ["projected_root = \"/run/secrets/prodex\"", "projected gateway secret root"],
  ["projected_provider = \"kubernetes\"", "projected gateway secret provider"],
  ["production = true", "projected secret production mode"],
  ["require_auth = true", "gateway authentication requirement"],
  ["auth_token_ref = { provider = \"kubernetes\", name = \"PRODEX_GATEWAY_TOKEN\" }", "projected gateway auth token reference"],
  ["provider_api_key_ref = { provider = \"kubernetes\", name = \"OPENAI_API_KEY\" }", "projected provider API key reference"],
  ["postgres_url_ref = { provider = \"kubernetes\", name = \"PRODEX_GATEWAY_POSTGRES_URL\" }", "projected PostgreSQL URL reference"],
  ["redis_url_ref = { provider = \"kubernetes\", name = \"PRODEX_GATEWAY_REDIS_URL\" }", "projected Redis coordination URL reference"],
  ["token_ref = { provider = \"kubernetes\", name = \"PRODEX_GATEWAY_METRICS_TOKEN\" }", "projected metrics token reference"],
  ["mountPath: /run/secrets/prodex", "gateway projected secret mount"],
  ["defaultMode: 0440", "private projected secret file mode"],
  ["name: prodex-gateway-policy", "gateway policy ConfigMap"],
  ["token_ref = { provider = \"kubernetes\", name = \"PRODEX_GATEWAY_METRICS_TOKEN\" }", "metrics viewer admin token policy binding"],
  ['role = "viewer"', "metrics viewer admin token role"],
  ["mountPath: /var/lib/prodex/policy.toml", "gateway policy file mount"],
  ["subPath: policy.toml", "gateway policy ConfigMap subPath"],
  ['backend = "postgres"', "PostgreSQL gateway state backend"],
  ["postgres_url_ref = { provider = \"kubernetes\", name = \"PRODEX_GATEWAY_POSTGRES_URL\" }", "PostgreSQL gateway state URL reference"],
  ['postgres_tls_mode = "verify-full"', "PostgreSQL certificate and hostname verification policy"],
  ["name: prodex-default-deny", "namespace default-deny network policy"],
  ["prodex.dev/network-tier: ingress", "restricted ingress namespace selector"],
  ["prodex.dev/network-tier: monitoring", "monitoring namespace selector for metrics scraping"],
  ["k8s-app: kube-dns", "DNS-only egress target"],
  ["prodex.dev/network-tier: data-store", "data-store egress namespace selector"],
  ["port: 5432", "PostgreSQL egress port"],
  ["port: 6379", "Redis egress port"],
  ["prodex.dev/network-tier: ai-egress", "approved AI egress namespace selector"],
]);

const REQUIRED_CONTROL_PLANE_MARKERS = Object.freeze([
  ["name: prodex-gateway", "gateway deployment/service surface"],
  ["name: prodex-control-plane", "control-plane deployment/service surface"],
  ["name: prodex-control-plane-config", "dedicated control-plane runtime ConfigMap"],
  ["name: prodex-control-plane-policy", "dedicated control-plane policy ConfigMap"],
  ["name: prodex-control-plane-secrets", "control-plane storage/coordination secret"],
  ["serviceAccountName: prodex-control-plane", "explicit control-plane service account"],
  ["app.kubernetes.io/component: control-plane", "control-plane component label"],
  ['command: ["/usr/local/bin/prodex-control-plane"]', "control-plane binary command"],
  ['args: ["serve", "--listen", "0.0.0.0:4100"]', "explicit control-plane listen address"],
  ["service_mode = \"control-plane\"", "typed control-plane runtime policy mode"],
  ["PRODEX_CONTROL_PLANE_ADMIN_TOKEN", "projected control-plane admin token"],
  ["replicas: 1", "live single-replica control-plane deployment"],
  ["terminationGracePeriodSeconds: 45", "control-plane termination grace period"],
  ['command: ["sh", "-c", "sleep 15"]', "control-plane pre-stop drain delay"],
  ["port: 4100", "control-plane service port"],
]);

const REQUIRED_BACKUP_MARKERS = Object.freeze([
  ["## File backend", "file backend backup section"],
  ["## SQLite backend", "SQLite backup section"],
  ["## PostgreSQL backend", "PostgreSQL backup section"],
  ["## Redis backend", "Redis backup section"],
  ["## Audit and evidence", "audit evidence section"],
  ["/readyz", "restore readiness check"],
  ["cross-tenant", "tenant-isolation restore acceptance"],
  ["npm run ci:backup-restore-drill", "automated PostgreSQL restore drill"],
  ["target/backup-restore-drill/evidence.json", "redacted drill evidence path"],
  ["RPO", "recovery-point objective gate"],
  ["RTO", "recovery-time objective gate"],
]);

const REQUIRED_DEPLOYMENT_GUIDE_MARKERS = Object.freeze([
  ["--secret-env deploy/gateway.env", "Compose secret source metadata preflight"],
  ["run --rm prodex-gateway", "one-shot gateway migration invocation"],
  ["migrate --backend postgres", "explicit PostgreSQL migration command"],
  ["--url-ref PRODEX_GATEWAY_POSTGRES_URL", "projected migration URL reference"],
  ["--secret-provider compose --secret-root /run/secrets", "Compose migration secret provider"],
  ["--tls-mode disable", "local Compose migration TLS mode"],
  ["request-serving containers never perform schema migration", "request-path migration prohibition"],
  ["https://docs.docker.com/reference/compose-file/services/#secrets", "official Compose file-secret limitation source"],
]);

function requireIncludes(checks, text, needle, path, description) {
  if (!text.includes(needle)) {
    checks.push(`${path}: missing ${description} (${needle})`);
  }
}

function requireWorkloadHardening(checks, document, path, workload) {
  if (!document) return;
  for (const [needle, description] of [
    ["runAsNonRoot: true", "non-root pod security context"],
    ["type: RuntimeDefault", "RuntimeDefault seccomp profile"],
    ["allowPrivilegeEscalation: false", "disabled privilege escalation"],
    ["readOnlyRootFilesystem: true", "read-only container filesystem"],
    ['drop: ["ALL"]', "dropped Linux capabilities"],
  ]) {
    requireIncludes(checks, document, needle, path, `${workload} ${description}`);
  }
}

function validateComposeSecretFiles(checks, files, envPath) {
  if (!files) return;
  for (const environment of COMPOSE_SECRET_FILE_ENVIRONMENTS) {
    const file = files[environment];
    if (!file) {
      checks.push(`${envPath}: missing ${environment} source metadata`);
      continue;
    }
    if (file.error) {
      checks.push(`${envPath}: ${environment} ${file.error}`);
      continue;
    }
    if (!file.isFile || file.isSymbolicLink) {
      checks.push(`${envPath}: ${environment} must reference a regular non-symlink secret file`);
    }
    if (file.uid !== 10001) {
      checks.push(`${envPath}: ${environment} secret file must be owned by container UID 10001`);
    }
    if ((file.mode & 0o077) !== 0) {
      checks.push(`${envPath}: ${environment} secret file must not grant group or other permissions`);
    }
  }
}

function kubernetesDocumentByKindAndName(kubernetes, kind, name) {
  return kubernetesDocumentsByKindAndName(kubernetes, kind, name)[0];
}

function kubernetesDocumentsByKindAndName(kubernetes, kind, name) {
  return kubernetes
    .split(/^---\s*$/mu)
    .map((document) => document.trim())
    .filter(
      (document) =>
        new RegExp(`^kind:\\s*${kind}\\s*$`, "mu").test(document) &&
        new RegExp(`^\\s*name:\\s*${name}\\s*$`, "mu").test(document),
    );
}

export function validateDeploymentSecurity(inputs) {
  const checks = [];
  const {
    composePath = "compose.yaml",
    composePolicyPath = "deploy/compose-gateway-policy.toml",
    envExamplePath = "deploy/gateway.env.example",
    composeSecretEnvPath = envExamplePath,
    dockerfilePath = "Dockerfile",
    kubernetesPath = "deploy/kubernetes/prodex-gateway.yaml",
    deploymentGuidePath = "docs/deployment.md",
    backupRunbookPath = "docs/backup-restore.md",
    compose,
    composePolicy,
    composeSecretFiles,
    envExample,
    dockerfile,
    kubernetes,
    deploymentGuide,
    backupRunbook,
  } = inputs;

  validateComposeSecretFiles(checks, composeSecretFiles, composeSecretEnvPath);

  for (const [path, text] of [
    [composePath, compose],
    [envExamplePath, envExample],
  ]) {
    if (/\bchange-me\b/u.test(text)) {
      checks.push(`${path}: deployment examples must not contain change-me credentials`);
    }
    if (/postgres:\/\/prodex:prodex@/u.test(text)) {
      checks.push(`${path}: postgres examples must not contain a static prodex password`);
    }
  }

  for (const name of FORBIDDEN_COMPOSE_SECRET_ENV) {
    if (new RegExp(`^\\s+${name}:`, "mu").test(compose)) {
      checks.push(`${composePath}: ${name} must use a Compose secret file, not an environment variable`);
    }
    if (new RegExp(`^${name}=`, "mu").test(envExample)) {
      checks.push(`${envExamplePath}: ${name} must not contain a secret value; configure ${name}_FILE`);
    }
  }
  if (/\benv_file\s*:/u.test(compose)) {
    checks.push(`${composePath}: gateway secrets must not be imported through env_file`);
  }
  requireIncludes(
    checks,
    compose,
    'entrypoint: ["/usr/local/bin/prodex-gateway"]',
    composePath,
    "dedicated gateway entrypoint",
  );
  requireIncludes(
    checks,
    compose,
    'command: ["serve", "--listen", "0.0.0.0:4000"]',
    composePath,
    "dedicated gateway serve command",
  );
  requireIncludes(
    checks,
    compose,
    "127.0.0.1:4000:4000",
    composePath,
    "loopback-only default published gateway port",
  );
  if (/^\s*-\s*["']?4000:4000["']?\s*$/mu.test(compose)) {
    checks.push(`${composePath}: default gateway port must not bind every host interface`);
  }
  requireIncludes(
    checks,
    compose,
    "./deploy/compose-gateway-policy.toml:/var/lib/prodex/policy.toml:ro",
    composePath,
    "tracked gateway policy mount",
  );
  for (const [source, target, fileEnvironment] of COMPOSE_PROJECTED_SECRETS) {
    if (
      !new RegExp(`- source: ${source}\\s+target: ${target}(?:\\s|$)`, "u").test(compose)
    ) {
      checks.push(`${composePath}: ${target} must be projected into the gateway container`);
    }
    requireIncludes(
      checks,
      compose,
      `file: \${${fileEnvironment}:?set ${fileEnvironment}}`,
      composePath,
      `${target} Compose secret file source`,
    );
    requireIncludes(checks, envExample, `${fileEnvironment}=`, envExamplePath, `${target} secret file path`);
  }
  requireIncludes(
    checks,
    compose,
    "POSTGRES_PASSWORD_FILE: /run/secrets/PRODEX_POSTGRES_PASSWORD",
    composePath,
    "Postgres password file environment",
  );
  if (!/- source: prodex_postgres_password\s+target: PRODEX_POSTGRES_PASSWORD/u.test(compose)) {
    checks.push(`${composePath}: Postgres password must be projected into the database container`);
  }
  requireIncludes(
    checks,
    compose,
    "file: ${PRODEX_POSTGRES_PASSWORD_FILE:?set PRODEX_POSTGRES_PASSWORD_FILE}",
    composePath,
    "Postgres password secret file source",
  );
  requireIncludes(
    checks,
    envExample,
    "PRODEX_POSTGRES_PASSWORD_FILE=",
    envExamplePath,
    "Postgres password secret file path",
  );
  for (const [needle, description] of [
    ["version = 1", "runtime policy version"],
    ['projected_root = "/run/secrets"', "Compose projected secret root"],
    ['projected_provider = "compose"', "Compose projected secret provider"],
    ['listen_addr = "0.0.0.0:4000"', "actual Compose gateway listen address"],
    ['expected_host = "127.0.0.1:4000"', "exact Compose gateway authority"],
    ["require_auth = true", "gateway authentication requirement"],
    ['auth_token_ref = { provider = "compose", name = "PRODEX_GATEWAY_TOKEN" }', "gateway token SecretRef"],
    ['provider_api_key_ref = { provider = "compose", name = "OPENAI_API_KEY" }', "OpenAI API key SecretRef"],
    ['backend = "postgres"', "PostgreSQL gateway state backend"],
    ['postgres_url_ref = { provider = "compose", name = "PRODEX_GATEWAY_POSTGRES_URL" }', "PostgreSQL URL SecretRef"],
    ['postgres_tls_mode = "disable"', "local Compose PostgreSQL TLS mode"],
    ['redis_url_ref = { provider = "compose", name = "PRODEX_GATEWAY_REDIS_URL" }', "Redis URL SecretRef"],
  ]) {
    requireIncludes(checks, composePolicy, needle, composePolicyPath, description);
  }
  if (/\b(?:postgres_url_env|redis_url_env|token_env|provider_api_key_env|auth_token_env)\s*=/u.test(composePolicy)) {
    checks.push(`${composePolicyPath}: Compose policy must use SecretRef fields, not secret environment references`);
  }
  if (/serviceAccountName:\s*default\b/u.test(kubernetes)) {
    checks.push(`${kubernetesPath}: workloads must not use the default service account`);
  }
  if (/automountServiceAccountToken:\s*true\b/u.test(kubernetes)) {
    checks.push(`${kubernetesPath}: service account token automount must stay disabled`);
  }
  if (
    /^\s*-\s*name:\s*(?:PRODEX|OPENAI|GEMINI|ANTHROPIC|DEEPSEEK|GITHUB)_[A-Z0-9_]*(?:TOKEN|SECRET|PASSWORD|API_KEY|URL)\s*\n\s*value:\s*["']?[^"'\n]+["']?\s*$/mu.test(
      kubernetes,
    )
  ) {
    checks.push(`${kubernetesPath}: secret-bearing environment variables must use ExternalSecret-backed refs, not literal values`);
  }
  const namespace = kubernetesDocumentByKindAndName(kubernetes, "Namespace", "prodex");
  if (!namespace) {
    checks.push(`${kubernetesPath}: missing prodex Namespace`);
  } else {
    for (const mode of ["enforce", "audit", "warn"]) {
      if (!new RegExp(`^\\s*pod-security\\.kubernetes\\.io/${mode}:\\s*restricted\\s*$`, "mu").test(namespace)) {
        checks.push(`${kubernetesPath}: prodex Namespace must set pod-security ${mode}=restricted`);
      }
      if (!new RegExp(`^\\s*pod-security\\.kubernetes\\.io/${mode}-version:\\s*latest\\s*$`, "mu").test(namespace)) {
        checks.push(`${kubernetesPath}: prodex Namespace must set pod-security ${mode}-version=latest`);
      }
    }
  }
  const defaultDenyPolicy = kubernetesDocumentByKindAndName(kubernetes, "NetworkPolicy", "prodex-default-deny");
  if (!defaultDenyPolicy) {
    checks.push(`${kubernetesPath}: missing namespace default-deny NetworkPolicy`);
  } else {
    if (!/^\s*podSelector:\s*\{\}\s*$/mu.test(defaultDenyPolicy)) {
      checks.push(`${kubernetesPath}: default-deny NetworkPolicy must select all pods`);
    }
    if (
      !/policyTypes:\s*\["Ingress",\s*"Egress"\]/u.test(defaultDenyPolicy) &&
      !/policyTypes:\s*\["Egress",\s*"Ingress"\]/u.test(defaultDenyPolicy)
    ) {
      checks.push(`${kubernetesPath}: default-deny NetworkPolicy must cover ingress and egress`);
    }
    if (/^\s*(ingress|egress):\s*$/mu.test(defaultDenyPolicy)) {
      checks.push(`${kubernetesPath}: default-deny NetworkPolicy must not contain allow rules`);
    }
  }
  const gatewayPolicy = kubernetesDocumentByKindAndName(kubernetes, "NetworkPolicy", "prodex-gateway");
  if (!gatewayPolicy) {
    checks.push(`${kubernetesPath}: missing gateway NetworkPolicy`);
  } else if (!/prodex\.dev\/network-tier:\s*monitoring[\s\S]*?port:\s*4000\b/u.test(gatewayPolicy)) {
    checks.push(`${kubernetesPath}: gateway NetworkPolicy must allow monitoring metrics ingress on port 4000`);
  }
  if (gatewayPolicy && /cidr:\s*(?:0\.0\.0\.0\/0|::\/0)\b/u.test(gatewayPolicy)) {
    checks.push(`${kubernetesPath}: gateway NetworkPolicy must route provider traffic through approved AI egress`);
  }
  const gatewaySecrets = kubernetesDocumentByKindAndName(kubernetes, "ExternalSecret", "prodex-gateway-secrets");
  if (!gatewaySecrets) {
    checks.push(`${kubernetesPath}: missing gateway ExternalSecret`);
  } else if (
    !/secretKey:\s*PRODEX_GATEWAY_METRICS_TOKEN/u.test(gatewaySecrets) ||
    !/key:\s*prodex\/gateway\/metrics-token/u.test(gatewaySecrets)
  ) {
    checks.push(`${kubernetesPath}: gateway ExternalSecret must include a dedicated metrics token`);
  }
  const serviceMonitor = kubernetesDocumentByKindAndName(kubernetes, "ServiceMonitor", "prodex-gateway");
  if (!serviceMonitor) {
    checks.push(`${kubernetesPath}: missing gateway ServiceMonitor`);
  } else {
    if (
      !/authorization:\s*\n\s*type:\s*Bearer\s*\n\s*credentials:\s*\n\s*name:\s*prodex-gateway-secrets\s*\n\s*key:\s*PRODEX_GATEWAY_METRICS_TOKEN/u.test(
        serviceMonitor,
      )
    ) {
      checks.push(`${kubernetesPath}: ServiceMonitor must use the dedicated metrics token`);
    }
    if (/key:\s*PRODEX_GATEWAY_TOKEN\b/u.test(serviceMonitor)) {
      checks.push(`${kubernetesPath}: ServiceMonitor must not use the gateway root token`);
    }
  }
  const gatewayPolicyConfig = kubernetesDocumentByKindAndName(kubernetes, "ConfigMap", "prodex-gateway-policy");
  if (!gatewayPolicyConfig) {
    checks.push(`${kubernetesPath}: missing gateway policy ConfigMap`);
  } else {
    if (!/(?:^|\n)\s*version\s*=\s*1\s*$/mu.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must declare runtime policy version 1`);
    }
    if (!/\[\[gateway\.admin_tokens\]\]/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must define admin token bindings`);
    }
    if (!/(?:^|\n)\s*listen_addr\s*=\s*"0\.0\.0\.0:4000"\s*$/mu.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must match the Deployment listen address`);
    }
    if (!/(?:^|\n)\s*expected_host\s*=\s*"prodex-gateway\.prodex\.svc\.cluster\.local:4000"\s*$/mu.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must declare the exact in-cluster authority`);
    }
    if (!/(?:^|\n)\s*token_ref\s*=\s*\{\s*provider\s*=\s*"kubernetes",\s*name\s*=\s*"PRODEX_GATEWAY_METRICS_TOKEN"\s*\}/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must bind the metrics token projected reference`);
    }
    if (!/role\s*=\s*"viewer"/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must bind metrics token as viewer`);
    }
    if (/(?:^|\n)\s*token_ref\s*=\s*\{\s*provider\s*=\s*"kubernetes",\s*name\s*=\s*"PRODEX_GATEWAY_TOKEN"\s*\}/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must not bind the root token as metrics viewer`);
    }
    if (!/\[gateway\.state\]/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must configure shared gateway state`);
    }
    if (!/backend\s*=\s*"postgres"/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must use the PostgreSQL state backend`);
    }
    if (!/postgres_url_ref\s*=\s*\{\s*provider\s*=\s*"kubernetes",\s*name\s*=\s*"PRODEX_GATEWAY_POSTGRES_URL"\s*\}/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must read PostgreSQL state URL from a projected secret reference`);
    }
    if (!/redis_url_ref\s*=\s*\{\s*provider\s*=\s*"kubernetes",\s*name\s*=\s*"PRODEX_GATEWAY_REDIS_URL"\s*\}/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must read Redis coordination URL from a projected secret reference`);
    }
    if (/\b(?:postgres_url_env|redis_url_env|token_env)\s*=/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must not use legacy secret env references`);
    }
  }
  const controlPlanePolicyConfig = kubernetesDocumentByKindAndName(
    kubernetes,
    "ConfigMap",
    "prodex-control-plane-policy",
  );
  if (!controlPlanePolicyConfig) {
    checks.push(`${kubernetesPath}: missing control-plane policy ConfigMap`);
  } else {
    for (const [pattern, description] of [
      [/(?:^|\n)\s*version\s*=\s*1\s*$/mu, "runtime policy version 1"],
      [/(?:^|\n)\s*service_mode\s*=\s*"control-plane"\s*$/mu, "typed service_mode=control-plane"],
      [/(?:^|\n)\s*production\s*=\s*true\s*$/mu, "production projected-secret mode"],
      [/projected_root\s*=\s*"\/run\/secrets\/prodex"/u, "projected secret root"],
      [/projected_provider\s*=\s*"kubernetes"/u, "projected Kubernetes secret provider"],
      [/(?:^|\n)\s*listen_addr\s*=\s*"0\.0\.0\.0:4100"\s*$/mu, "actual control-plane listen address"],
      [/(?:^|\n)\s*expected_host\s*=\s*"prodex-control-plane\.prodex\.svc\.cluster\.local:4100"\s*$/mu, "exact in-cluster control-plane authority"],
      [/\[gateway\.state\]/u, "shared state section"],
      [/(?:^|\n)\s*backend\s*=\s*"postgres"\s*$/mu, "PostgreSQL shared state backend"],
      [/postgres_url_ref\s*=\s*\{\s*provider\s*=\s*"kubernetes",\s*name\s*=\s*"PRODEX_GATEWAY_POSTGRES_URL"\s*\}/u, "projected PostgreSQL URL"],
      [/redis_url_ref\s*=\s*\{\s*provider\s*=\s*"kubernetes",\s*name\s*=\s*"PRODEX_GATEWAY_REDIS_URL"\s*\}/u, "projected Redis URL"],
      [/\[\[gateway\.admin_tokens\]\]/u, "projected admin token binding"],
      [/token_ref\s*=\s*\{\s*provider\s*=\s*"kubernetes",\s*name\s*=\s*"PRODEX_CONTROL_PLANE_ADMIN_TOKEN"\s*\}/u, "projected control-plane admin token"],
      [/(?:^|\n)\s*role\s*=\s*"admin"\s*$/mu, "explicit admin role"],
    ]) {
      if (!pattern.test(controlPlanePolicyConfig)) {
        checks.push(`${kubernetesPath}: control-plane policy must configure ${description}`);
      }
    }
    if (
      /(?:^|\n)\s*(?:provider|base_url|require_auth|auth_token_ref|provider_api_key_ref)\s*=/mu.test(
        controlPlanePolicyConfig,
      ) ||
      /\[\[gateway\.(?:virtual_keys|route_aliases)\]\]|\[gateway\.(?:adaptive_routing|request_constraints|sso|observability|guardrails)\]/u.test(
        controlPlanePolicyConfig,
      )
    ) {
      checks.push(`${kubernetesPath}: control-plane policy must not configure data-plane, SSO, or outbound capabilities`);
    }
    if (/\b(?:postgres_url_env|redis_url_env|token_env|http_bearer_token_env)\s*=/u.test(controlPlanePolicyConfig)) {
      checks.push(`${kubernetesPath}: control-plane policy must use projected SecretRef fields, not secret environment names`);
    }
  }
  const controlPlaneSecrets = kubernetesDocumentByKindAndName(
    kubernetes,
    "ExternalSecret",
    "prodex-control-plane-secrets",
  );
  if (!controlPlaneSecrets) {
    checks.push(`${kubernetesPath}: missing control-plane ExternalSecret`);
  } else {
    const secretKeys = [...controlPlaneSecrets.matchAll(/^\s*-\s*secretKey:\s*(\S+)\s*$/gmu)].map(
      (match) => match[1],
    );
    const expectedSecretKeys = [
      "PRODEX_CONTROL_PLANE_ADMIN_TOKEN",
      "PRODEX_GATEWAY_POSTGRES_URL",
      "PRODEX_GATEWAY_REDIS_URL",
    ];
    if (
      secretKeys.length !== expectedSecretKeys.length ||
      expectedSecretKeys.some((key) => !secretKeys.includes(key)) ||
      secretKeys.some((key) => !expectedSecretKeys.includes(key))
    ) {
      checks.push(
        `${kubernetesPath}: control-plane ExternalSecret must contain only admin, PostgreSQL, and Redis projected keys`,
      );
    }
  }
  const gatewayDeployment = kubernetesDocumentByKindAndName(kubernetes, "Deployment", "prodex-gateway");
  if (!gatewayDeployment) {
    checks.push(`${kubernetesPath}: missing gateway Deployment`);
  } else {
    requireWorkloadHardening(checks, gatewayDeployment, kubernetesPath, "gateway Deployment");
    if (!/^\s*serviceAccountName:\s*prodex-gateway\s*$/mu.test(gatewayDeployment)) {
      checks.push(`${kubernetesPath}: gateway Deployment must use prodex-gateway service account`);
    }
    if (!/^\s*command:\s*\["\/usr\/local\/bin\/prodex-gateway"\]\s*$/mu.test(gatewayDeployment)) {
      checks.push(`${kubernetesPath}: gateway Deployment must use the dedicated prodex-gateway entrypoint`);
    }
    if (!/^\s*args:\s*\["serve",\s*"--listen",\s*"0\.0\.0\.0:4000"\]\s*$/mu.test(gatewayDeployment)) {
      checks.push(`${kubernetesPath}: gateway Deployment must run the dedicated serve command`);
    }
    if (/envFrom:[\s\S]*?secretRef:\s*\n\s*name:\s*prodex-gateway-secrets/u.test(gatewayDeployment)) {
      checks.push(`${kubernetesPath}: gateway Secret must not be consumed through envFrom`);
    }
    if (!/envFrom:\s*\n\s*-\s*configMapRef:\s*\n\s*name:\s*prodex-gateway-config/u.test(gatewayDeployment)) {
      checks.push(`${kubernetesPath}: gateway workload must keep prodex-gateway-config envFrom`);
    }
    if (!/name:\s*PRODEX_CONFIG_PUBLICATION_REPLICA_ID\s*\n\s*valueFrom:\s*\n\s*fieldRef:\s*\n\s*fieldPath:\s*metadata\.name/u.test(gatewayDeployment)) {
      checks.push(`${kubernetesPath}: gateway config-publication replica ID must come from the pod name`);
    }
    if (/secretRef:\s*\n\s*name:\s*prodex-control-plane-secrets/u.test(gatewayDeployment)) {
      checks.push(`${kubernetesPath}: gateway workload must not mount prodex-control-plane-secrets`);
    }
    if (!/mountPath:\s*\/var\/lib\/prodex\/policy\.toml[\s\S]*?subPath:\s*policy\.toml/u.test(gatewayDeployment)) {
      checks.push(`${kubernetesPath}: gateway Deployment must mount policy.toml from ConfigMap`);
    }
    if (!/configMap:\s*\n\s*name:\s*prodex-gateway-policy/u.test(gatewayDeployment)) {
      checks.push(`${kubernetesPath}: gateway Deployment must source policy.toml from prodex-gateway-policy`);
    }
    const gatewayVolumeMounts =
      gatewayDeployment.split(/\n\s+volumeMounts:\s*\n/u)[1]?.split(/\n\s+volumes:\s*\n/u)[0] ?? "";
    const gatewaySecretMount = gatewayVolumeMounts
      .split(/(?=^\s+-\s+name:\s+)/mu)
      .find((item) => /name:\s+prodex-gateway-secrets\b/u.test(item));
    if (!gatewaySecretMount || !/mountPath:\s*\/run\/secrets\/prodex/u.test(gatewaySecretMount)) {
      checks.push(`${kubernetesPath}: gateway Deployment must mount the Secret at /run/secrets/prodex`);
    } else {
      if (/subPath:/u.test(gatewaySecretMount)) {
        checks.push(`${kubernetesPath}: gateway Secret volume mount must not use subPath`);
      }
      if (!/readOnly:\s*true/u.test(gatewaySecretMount)) {
        checks.push(`${kubernetesPath}: gateway Secret volume mount must be read-only`);
      }
    }
    const gatewayVolumes = gatewayDeployment.split(/\n\s+volumes:\s*\n/u)[1] ?? "";
    const gatewaySecretVolume = gatewayVolumes
      .split(/(?=^\s+-\s+name:\s+)/mu)
      .find((item) => /name:\s+prodex-gateway-secrets\b/u.test(item));
    if (!gatewaySecretVolume || !/projected:\s*\n/u.test(gatewaySecretVolume)) {
      checks.push(`${kubernetesPath}: gateway Secret must use a projected volume`);
    } else {
      if (!/defaultMode:\s*0440\b/u.test(gatewaySecretVolume)) {
        checks.push(`${kubernetesPath}: gateway projected Secret volume must use private defaultMode 0440`);
      }
      if (!/secret:\s*\n\s*name:\s*prodex-gateway-secrets/u.test(gatewaySecretVolume)) {
        checks.push(`${kubernetesPath}: gateway projected Secret volume must reference prodex-gateway-secrets`);
      }
    }
  }
  const controlPlaneDeployment = kubernetesDocumentByKindAndName(kubernetes, "Deployment", "prodex-control-plane");
  if (!controlPlaneDeployment) {
    checks.push(`${kubernetesPath}: missing control-plane Deployment`);
  } else {
    requireWorkloadHardening(checks, controlPlaneDeployment, kubernetesPath, "control-plane Deployment");
    if (!/^\s*serviceAccountName:\s*prodex-control-plane\s*$/mu.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane Deployment must use prodex-control-plane service account`);
    }
    if (!/^\s*replicas:\s*1\s*$/mu.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane Deployment must run exactly one replica`);
    }
    if (!/^\s*args:\s*\["serve",\s*"--listen",\s*"0\.0\.0\.0:4100"\]\s*$/mu.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane Deployment must listen explicitly on 0.0.0.0:4100`);
    }
    if (!/terminationGracePeriodSeconds:\s*45\b/u.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane Deployment must declare a 45-second termination grace period`);
    }
    if (!/preStop:[\s\S]*?command:\s*\["sh",\s*"-c",\s*"sleep 15"\]/u.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane Deployment must declare the pre-stop drain delay`);
    }
    if (/envFrom:[\s\S]*?secretRef:/u.test(controlPlaneDeployment) || /secretKeyRef:/u.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane secrets must not be consumed through env or envFrom`);
    }
    if (!/envFrom:\s*\n\s*-\s*configMapRef:\s*\n\s*name:\s*prodex-control-plane-config/u.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane Deployment must use its dedicated runtime ConfigMap`);
    }
    if (!/name:\s*PRODEX_CONFIG_PUBLICATION_REPLICA_ID\s*\n\s*valueFrom:\s*\n\s*fieldRef:\s*\n\s*fieldPath:\s*metadata\.name/u.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane config-publication replica ID must come from the pod name`);
    }
    if (!/mountPath:\s*\/var\/lib\/prodex\/policy\.toml[\s\S]*?subPath:\s*policy\.toml/u.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane Deployment must mount its policy.toml ConfigMap`);
    }
    if (!/configMap:\s*\n\s*name:\s*prodex-control-plane-policy/u.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane Deployment must source prodex-control-plane-policy`);
    }
    const controlPlaneVolumeMounts =
      controlPlaneDeployment.split(/\n\s+volumeMounts:\s*\n/u)[1]?.split(/\n\s+volumes:\s*\n/u)[0] ?? "";
    const controlPlaneSecretMount = controlPlaneVolumeMounts
      .split(/(?=^\s+-\s+name:\s+)/mu)
      .find((item) => /name:\s+prodex-control-plane-secrets\b/u.test(item));
    if (!controlPlaneSecretMount || !/mountPath:\s*\/run\/secrets\/prodex/u.test(controlPlaneSecretMount)) {
      checks.push(`${kubernetesPath}: control-plane Deployment must mount projected secrets at /run/secrets/prodex`);
    } else {
      if (/subPath:/u.test(controlPlaneSecretMount)) {
        checks.push(`${kubernetesPath}: control-plane projected Secret mount must not use subPath`);
      }
      if (!/readOnly:\s*true/u.test(controlPlaneSecretMount)) {
        checks.push(`${kubernetesPath}: control-plane projected Secret mount must be read-only`);
      }
    }
    const controlPlaneVolumes = controlPlaneDeployment.split(/\n\s+volumes:\s*\n/u)[1] ?? "";
    const controlPlaneSecretVolume = controlPlaneVolumes
      .split(/(?=^\s+-\s+name:\s+)/mu)
      .find((item) => /name:\s+prodex-control-plane-secrets\b/u.test(item));
    if (!controlPlaneSecretVolume || !/projected:\s*\n/u.test(controlPlaneSecretVolume)) {
      checks.push(`${kubernetesPath}: control-plane Secret must use a projected volume`);
    } else {
      if (!/defaultMode:\s*0440\b/u.test(controlPlaneSecretVolume)) {
        checks.push(`${kubernetesPath}: control-plane projected Secret volume must use private defaultMode 0440`);
      }
      if (!/secret:\s*\n\s*name:\s*prodex-control-plane-secrets/u.test(controlPlaneSecretVolume)) {
        checks.push(`${kubernetesPath}: control-plane projected Secret volume must reference prodex-control-plane-secrets`);
      }
      for (const key of [
        "PRODEX_CONTROL_PLANE_ADMIN_TOKEN",
        "PRODEX_GATEWAY_POSTGRES_URL",
        "PRODEX_GATEWAY_REDIS_URL",
      ]) {
        if (!new RegExp(`key:\\s*${key}\\b`, "u").test(controlPlaneSecretVolume)) {
          checks.push(`${kubernetesPath}: control-plane projected Secret volume must include ${key}`);
        }
      }
    }
    if (/prodex-gateway-secrets|prodex-gateway-policy|OPENAI_API_KEY|PRODEX_GATEWAY_TOKEN\b/u.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane workload must not mount gateway or provider capabilities`);
    }
  }
  const migrationJob = kubernetesDocumentByKindAndName(kubernetes, "Job", "prodex-gateway-migration");
  if (!migrationJob) {
    checks.push(`${kubernetesPath}: missing migration Job`);
  } else {
    requireWorkloadHardening(checks, migrationJob, kubernetesPath, "migration Job");
    if (!/^\s*serviceAccountName:\s*prodex-gateway-migration\s*$/mu.test(migrationJob)) {
      checks.push(`${kubernetesPath}: migration Job must use prodex-gateway-migration service account`);
    }
    if (/^\s*serviceAccountName:\s*prodex-gateway\s*$/mu.test(migrationJob)) {
      checks.push(`${kubernetesPath}: migration Job must not use prodex-gateway service account`);
    }
    if (/envFrom:[\s\S]*?secretRef:\s*\n\s*name:\s*prodex-gateway-migration-secrets/u.test(migrationJob)) {
      checks.push(`${kubernetesPath}: migration database URL must not be consumed through envFrom`);
    }
    if (!/mountPath:\s*\/run\/secrets\/prodex[\s\S]*?readOnly:\s*true/u.test(migrationJob)) {
      checks.push(`${kubernetesPath}: migration Job must mount projected secrets read-only`);
    }
    if (!/name:\s*prodex-gateway-migration-secrets[\s\S]*?projected:\s*\n[\s\S]*?defaultMode:\s*0440/u.test(migrationJob)) {
      checks.push(`${kubernetesPath}: migration Job must use a private projected Secret volume`);
    }
  }
  const controlPlanePolicies = kubernetesDocumentsByKindAndName(
    kubernetes,
    "NetworkPolicy",
    "prodex-control-plane",
  );
  if (controlPlanePolicies.length === 0) {
    checks.push(`${kubernetesPath}: missing control-plane NetworkPolicy`);
  } else if (
    controlPlanePolicies.some(
      (controlPlanePolicy) =>
        /cidr:\s*0\.0\.0\.0\/0/u.test(controlPlanePolicy) || /port:\s*443\b/u.test(controlPlanePolicy),
    )
  ) {
    checks.push(`${kubernetesPath}: control-plane NetworkPolicy must not allow public provider egress`);
  }

  requireIncludes(checks, compose, "read_only: true", composePath, "read-only root filesystem");
  requireIncludes(checks, compose, "cap_drop:", composePath, "capability drop list");
  requireIncludes(checks, compose, "- ALL", composePath, "drop all Linux capabilities");
  requireIncludes(
    checks,
    compose,
    "no-new-privileges:true",
    composePath,
    "no-new-privileges security option",
  );
  requireIncludes(
    checks,
    compose,
    "/tmp:rw,noexec,nosuid,size=64m",
    composePath,
    "bounded noexec tmpfs",
  );
  requireIncludes(
    checks,
    compose,
    "http://127.0.0.1:4000/readyz",
    composePath,
    "public readiness healthcheck",
  );
  requireIncludes(checks, dockerfile, "USER prodex", dockerfilePath, "non-root runtime user");
  requireIncludes(
    checks,
    dockerfile,
    "/workspace/target/release/prodex-gateway",
    dockerfilePath,
    "prodex-gateway binary copy",
  );
  requireIncludes(
    checks,
    dockerfile,
    "/workspace/target/release/prodex-control-plane",
    dockerfilePath,
    "prodex-control-plane binary copy",
  );
  requireIncludes(
    checks,
    dockerfile,
    'ENTRYPOINT ["/usr/local/bin/prodex-gateway"]',
    dockerfilePath,
    "dedicated gateway default entrypoint",
  );
  requireIncludes(
    checks,
    dockerfile,
    'CMD ["serve", "--listen", "0.0.0.0:4000"]',
    dockerfilePath,
    "dedicated gateway default serve command",
  );

  if (/healthcheck:[\s\S]*Authorization:\s*Bearer/u.test(compose)) {
    checks.push(`${composePath}: healthcheck must not place bearer tokens in process arguments`);
  }
  if (/@sha256:([0-9a-f])\1{63}\b/iu.test(kubernetes)) {
    checks.push(`${kubernetesPath}: image digest must not use a repeated-character placeholder`);
  } else if (/@sha256:([0-9a-f]{16})\1{3}\b/iu.test(kubernetes)) {
    checks.push(`${kubernetesPath}: image digest must not use a repeated-pattern placeholder`);
  }
  if (/@sha256:(?![0-9a-f]{64}\b)[^\s"']+/iu.test(kubernetes)) {
    checks.push(`${kubernetesPath}: image digest must be a 64-character sha256 hex value`);
  }

  for (const kind of REQUIRED_KINDS) {
    requireIncludes(checks, kubernetes, `kind: ${kind}`, kubernetesPath, `${kind} artifact`);
  }
  for (const [needle, description] of REQUIRED_GATEWAY_KUBERNETES_MARKERS) {
    requireIncludes(checks, kubernetes, needle, kubernetesPath, description);
  }
  if (/egress:\s+- to:\s+- namespaceSelector:\s*\{\}/u.test(kubernetes)) {
    checks.push(`${kubernetesPath}: NetworkPolicy egress must not allow all namespaces without ports`);
  }
  if (/ingress:\s+- from:\s+- namespaceSelector:\s*\{\}/u.test(kubernetes)) {
    checks.push(`${kubernetesPath}: NetworkPolicy ingress must not allow every namespace`);
  }
  for (const [needle, description] of REQUIRED_CONTROL_PLANE_MARKERS) {
    requireIncludes(checks, kubernetes, needle, kubernetesPath, description);
  }
  for (const [needle, description] of REQUIRED_DEPLOYMENT_GUIDE_MARKERS) {
    requireIncludes(checks, deploymentGuide, needle, deploymentGuidePath, description);
  }
  for (const [needle, description] of REQUIRED_BACKUP_MARKERS) {
    requireIncludes(checks, backupRunbook, needle, backupRunbookPath, description);
  }

  return checks;
}

function validFixture() {
  return {
    compose: `
entrypoint: ["/usr/local/bin/prodex-gateway"]
command: ["serve", "--listen", "0.0.0.0:4000"]
ports:
  - "127.0.0.1:4000:4000"
read_only: true
cap_drop:
  - ALL
security_opt:
  - no-new-privileges:true
tmpfs:
  - /tmp:rw,noexec,nosuid,size=64m
healthcheck:
  test: ["CMD", "curl", "http://127.0.0.1:4000/readyz"]
environment:
  PRODEX_HOME: /var/lib/prodex
  POSTGRES_PASSWORD_FILE: /run/secrets/PRODEX_POSTGRES_PASSWORD
secrets:
  - source: prodex_gateway_token
    target: PRODEX_GATEWAY_TOKEN
  - source: openai_api_key
    target: OPENAI_API_KEY
  - source: prodex_gateway_postgres_url
    target: PRODEX_GATEWAY_POSTGRES_URL
  - source: prodex_gateway_redis_url
    target: PRODEX_GATEWAY_REDIS_URL
  - source: prodex_postgres_password
    target: PRODEX_POSTGRES_PASSWORD
volumes:
  - ./deploy/compose-gateway-policy.toml:/var/lib/prodex/policy.toml:ro
prodex_gateway_token:
  file: \${PRODEX_GATEWAY_TOKEN_FILE:?set PRODEX_GATEWAY_TOKEN_FILE}
openai_api_key:
  file: \${OPENAI_API_KEY_FILE:?set OPENAI_API_KEY_FILE}
prodex_gateway_postgres_url:
  file: \${PRODEX_GATEWAY_POSTGRES_URL_FILE:?set PRODEX_GATEWAY_POSTGRES_URL_FILE}
prodex_gateway_redis_url:
  file: \${PRODEX_GATEWAY_REDIS_URL_FILE:?set PRODEX_GATEWAY_REDIS_URL_FILE}
prodex_postgres_password:
  file: \${PRODEX_POSTGRES_PASSWORD_FILE:?set PRODEX_POSTGRES_PASSWORD_FILE}
`,
    composePolicy: `
version = 1

[secrets]
projected_root = "/run/secrets"
projected_provider = "compose"

[gateway]
listen_addr = "0.0.0.0:4000"
expected_host = "127.0.0.1:4000"
require_auth = true
auth_token_ref = { provider = "compose", name = "PRODEX_GATEWAY_TOKEN" }
provider_api_key_ref = { provider = "compose", name = "OPENAI_API_KEY" }

[gateway.state]
backend = "postgres"
postgres_url_ref = { provider = "compose", name = "PRODEX_GATEWAY_POSTGRES_URL" }
postgres_tls_mode = "disable"
redis_url_ref = { provider = "compose", name = "PRODEX_GATEWAY_REDIS_URL" }
`,
    composeSecretFiles: Object.fromEntries(
      COMPOSE_SECRET_FILE_ENVIRONMENTS.map((environment) => [
        environment,
        { isFile: true, isSymbolicLink: false, mode: 0o600, uid: 10001 },
      ]),
    ),
    envExample: COMPOSE_SECRET_FILE_ENVIRONMENTS.map(
      (environment) => `${environment}=./deploy/secrets/${environment}`,
    ).join("\n"),
    deploymentGuide: REQUIRED_DEPLOYMENT_GUIDE_MARKERS.map(([needle]) => needle).join("\n"),
    dockerfile: `
COPY --from=builder /workspace/target/release/prodex-gateway /usr/local/bin/prodex-gateway
COPY --from=builder /workspace/target/release/prodex-control-plane /usr/local/bin/prodex-control-plane
USER prodex
ENTRYPOINT ["/usr/local/bin/prodex-gateway"]
CMD ["serve", "--listen", "0.0.0.0:4000"]
`,
    kubernetes: `
kind: Namespace
metadata:
  name: prodex
  labels:
    pod-security.kubernetes.io/enforce: restricted
    pod-security.kubernetes.io/enforce-version: latest
    pod-security.kubernetes.io/audit: restricted
    pod-security.kubernetes.io/audit-version: latest
    pod-security.kubernetes.io/warn: restricted
    pod-security.kubernetes.io/warn-version: latest
---
kind: Deployment
metadata:
  name: prodex-gateway
spec:
  template:
    spec:
      serviceAccountName: prodex-gateway
      securityContext:
        runAsNonRoot: true
        seccompProfile:
          type: RuntimeDefault
      containers:
        - name: prodex-gateway
          command: ["/usr/local/bin/prodex-gateway"]
          args: ["serve", "--listen", "0.0.0.0:4000"]
          securityContext:
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            capabilities:
              drop: ["ALL"]
      envFrom:
        - configMapRef:
            name: prodex-gateway-config
      env:
        - name: PRODEX_CONFIG_PUBLICATION_REPLICA_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
      volumeMounts:
        - name: prodex-gateway-secrets
          mountPath: /run/secrets/prodex
          readOnly: true
        - name: gateway-policy
          mountPath: /var/lib/prodex/policy.toml
          subPath: policy.toml
          readOnly: true
      volumes:
        - name: prodex-gateway-secrets
          projected:
            defaultMode: 0440
            sources:
              - secret:
                  name: prodex-gateway-secrets
        - name: gateway-policy
          configMap:
            name: prodex-gateway-policy
---
kind: Deployment
metadata:
  name: prodex-control-plane
spec:
  replicas: 1
  template:
    spec:
      serviceAccountName: prodex-control-plane
      terminationGracePeriodSeconds: 45
      securityContext:
        runAsNonRoot: true
        seccompProfile:
          type: RuntimeDefault
      containers:
        - name: prodex-control-plane
          command: ["/usr/local/bin/prodex-control-plane"]
          args: ["serve", "--listen", "0.0.0.0:4100"]
          envFrom:
            - configMapRef:
                name: prodex-control-plane-config
          env:
            - name: PRODEX_CONFIG_PUBLICATION_REPLICA_ID
              valueFrom:
                fieldRef:
                  fieldPath: metadata.name
          lifecycle:
            preStop:
              exec:
                command: ["sh", "-c", "sleep 15"]
          securityContext:
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            capabilities:
              drop: ["ALL"]
          volumeMounts:
            - name: prodex-control-plane-secrets
              mountPath: /run/secrets/prodex
              readOnly: true
            - name: control-plane-policy
              mountPath: /var/lib/prodex/policy.toml
              subPath: policy.toml
              readOnly: true
      volumes:
        - name: prodex-control-plane-secrets
          projected:
            defaultMode: 0440
            sources:
              - secret:
                  name: prodex-control-plane-secrets
                  items:
                    - key: PRODEX_CONTROL_PLANE_ADMIN_TOKEN
                      path: PRODEX_CONTROL_PLANE_ADMIN_TOKEN
                    - key: PRODEX_GATEWAY_POSTGRES_URL
                      path: PRODEX_GATEWAY_POSTGRES_URL
                    - key: PRODEX_GATEWAY_REDIS_URL
                      path: PRODEX_GATEWAY_REDIS_URL
        - name: control-plane-policy
          configMap:
            name: prodex-control-plane-policy
---
kind: ConfigMap
metadata:
  name: prodex-gateway-policy
data:
  policy.toml: |
    version = 1

    [secrets]
    production = true
    projected_root = "/run/secrets/prodex"
    projected_provider = "kubernetes"

    [gateway]
    listen_addr = "0.0.0.0:4000"
    expected_host = "prodex-gateway.prodex.svc.cluster.local:4000"
    require_auth = true
    auth_token_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_TOKEN" }
    provider_api_key_ref = { provider = "kubernetes", name = "OPENAI_API_KEY" }

    [gateway.state]
    backend = "postgres"
    postgres_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_POSTGRES_URL" }
    postgres_tls_mode = "verify-full"
    redis_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_REDIS_URL" }

    [[gateway.admin_tokens]]
    name = "prometheus"
    token_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_METRICS_TOKEN" }
    role = "viewer"
---
kind: ConfigMap
metadata:
  name: prodex-control-plane-config
---
kind: ConfigMap
metadata:
  name: prodex-control-plane-policy
data:
  policy.toml: |
    version = 1
    service_mode = "control-plane"

    [secrets]
    production = true
    projected_root = "/run/secrets/prodex"
    projected_provider = "kubernetes"

    [gateway]
    listen_addr = "0.0.0.0:4100"
    expected_host = "prodex-control-plane.prodex.svc.cluster.local:4100"

    [gateway.state]
    backend = "postgres"
    postgres_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_POSTGRES_URL" }
    redis_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_REDIS_URL" }

    [[gateway.admin_tokens]]
    name = "operations"
    token_ref = { provider = "kubernetes", name = "PRODEX_CONTROL_PLANE_ADMIN_TOKEN" }
    role = "admin"
---
kind: ExternalSecret
metadata:
  name: prodex-gateway-secrets
spec:
  data:
    - secretKey: PRODEX_GATEWAY_METRICS_TOKEN
      remoteRef:
        key: prodex/gateway/metrics-token
---
kind: ExternalSecret
metadata:
  name: prodex-control-plane-secrets
spec:
  data:
    - secretKey: PRODEX_CONTROL_PLANE_ADMIN_TOKEN
    - secretKey: PRODEX_GATEWAY_POSTGRES_URL
    - secretKey: PRODEX_GATEWAY_REDIS_URL
---
kind: ServiceMonitor
metadata:
  name: prodex-gateway
spec:
  endpoints:
    - port: http
      path: /v1/prodex/gateway/metrics
      authorization:
        type: Bearer
        credentials:
          name: prodex-gateway-secrets
          key: PRODEX_GATEWAY_METRICS_TOKEN
---
kind: NetworkPolicy
metadata:
  name: prodex-default-deny
spec:
  podSelector: {}
  policyTypes: ["Ingress", "Egress"]
---
kind: NetworkPolicy
metadata:
  name: prodex-gateway
spec:
  ingress:
    - from:
        - namespaceSelector:
            matchLabels:
              prodex.dev/network-tier: ingress
      ports:
        - protocol: TCP
          port: 4000
    - from:
        - namespaceSelector:
            matchLabels:
              prodex.dev/network-tier: monitoring
      ports:
        - protocol: TCP
          port: 4000
---
kind: NetworkPolicy
metadata:
  name: prodex-control-plane
spec:
  egress:
    - to:
        - namespaceSelector:
            matchLabels:
              prodex.dev/network-tier: data-store
      ports:
        - protocol: TCP
          port: 5432
        - protocol: TCP
          port: 6379
---
kind: Job
metadata:
  name: prodex-gateway-migration
spec:
  template:
    spec:
      serviceAccountName: prodex-gateway-migration
      securityContext:
        runAsNonRoot: true
        seccompProfile:
          type: RuntimeDefault
      containers:
        - name: migration
          args: ["--backend", "postgres", "--url-ref", "PRODEX_GATEWAY_POSTGRES_URL", "--secret-provider", "kubernetes", "--secret-root", "/run/secrets/prodex", "--tls-mode", "verify-full"]
          securityContext:
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            capabilities:
              drop: ["ALL"]
          volumeMounts:
            - name: prodex-gateway-migration-secrets
              mountPath: /run/secrets/prodex
              readOnly: true
      volumes:
        - name: prodex-gateway-migration-secrets
          projected:
            defaultMode: 0440
            sources:
              - secret:
                  name: prodex-gateway-migration-secrets
---
kind: ServiceAccount
kind: Service
kind: ConfigMap
kind: ExternalSecret
kind: PodDisruptionBudget
kind: HorizontalPodAutoscaler
kind: ServiceMonitor
kind: Job
name: prodex-gateway
name: prodex-control-plane
name: prodex-control-plane-secrets
serviceAccountName: prodex-gateway
serviceAccountName: prodex-gateway-migration
serviceAccountName: prodex-control-plane
automountServiceAccountToken: false
seccompProfile:
  type: RuntimeDefault
runAsNonRoot: true
readOnlyRootFilesystem: true
drop: ["ALL"]
allowPrivilegeEscalation: false
resources:
readinessProbe:
livenessProbe:
startupProbe:
path: /readyz
path: /livez
path: /startupz
topologySpreadConstraints:
topologyKey: topology.kubernetes.io/zone
topologyKey: kubernetes.io/hostname
whenUnsatisfiable: DoNotSchedule
terminationGracePeriodSeconds: 45
preStop:
command: ["sh", "-c", "sleep 15"]
image: repo/prodex@sha256:b148ccaa87b9601fe367313b3a734129372b12c473a2bf32bdf177bcb7a4289c
command: ["prodex-gateway", "migrate"]
"--backend"
"postgres"
"--url-ref"
"--secret-provider"
"--secret-root"
"--tls-mode"
"verify-full"
name: prodex-gateway-migration
name: prodex-gateway-migration-secrets
app.kubernetes.io/name: prodex-gateway-migration
PRODEX_GATEWAY_REPLICA_COUNT
PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS
PRODEX_GATEWAY_REDIS_URL
key: prodex/gateway/redis-url
prodex.dev/network-tier: ingress
prodex.dev/network-tier: monitoring
name: prodex-default-deny
k8s-app: kube-dns
prodex.dev/network-tier: data-store
port: 5432
port: 6379
prodex.dev/network-tier: ai-egress
app.kubernetes.io/component: control-plane
port: 4100
`,
    backupRunbook: `
## File backend
## SQLite backend
## PostgreSQL backend
## Redis backend
## Audit and evidence
/readyz
cross-tenant
npm run ci:backup-restore-drill
target/backup-restore-drill/evidence.json
RPO
RTO
`,
  };
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

export function runSelfTest() {
  const valid = validFixture();
  assertSelfTest(validateDeploymentSecurity(valid).length === 0, "valid fixture rejected");
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      deploymentGuide: valid.deploymentGuide.replace("run --rm prodex-gateway", "up prodex-gateway"),
    }).some((error) => error.includes("one-shot gateway migration invocation")),
    "deployment guide without one-shot migration accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("kind: Namespace", "kind: NamespaceMissing"),
    }).some((error) => error.includes("missing prodex Namespace")),
    "missing prodex Namespace accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("pod-security.kubernetes.io/enforce: restricted", ""),
    }).some((error) => error.includes("pod-security enforce=restricted")),
    "missing namespace pod-security enforce accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("pod-security.kubernetes.io/audit-version: latest", ""),
    }).some((error) => error.includes("pod-security audit-version=latest")),
    "missing namespace pod-security audit version accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("pod-security.kubernetes.io/warn: restricted", "pod-security.kubernetes.io/warn: baseline"),
    }).some((error) => error.includes("pod-security warn=restricted")),
    "weakened namespace pod-security warn accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("name: prodex-default-deny", "name: prodex-default-deny-missing"),
    }).some((error) => error.includes("missing namespace default-deny")),
    "missing namespace default-deny accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("podSelector: {}", "podSelector:\n  matchLabels:\n    app: prodex"),
    }).some((error) => error.includes("default-deny NetworkPolicy must select all pods")),
    "default-deny non-global pod selector accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace('policyTypes: ["Ingress", "Egress"]', 'policyTypes: ["Ingress"]'),
    }).some((error) => error.includes("default-deny NetworkPolicy must cover ingress and egress")),
    "default-deny without egress accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        'policyTypes: ["Ingress", "Egress"]',
        'policyTypes: ["Ingress", "Egress"]\ningress:\n  - from: []',
      ),
    }).some((error) => error.includes("default-deny NetworkPolicy must not contain allow rules")),
    "default-deny allow rule accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("prodex.dev/network-tier: monitoring", "prodex.dev/network-tier: metrics-missing"),
    }).some((error) => error.includes("monitoring metrics ingress")),
    "missing gateway monitoring metrics ingress accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "prodex.dev/network-tier: monitoring\n      ports:\n        - protocol: TCP\n          port: 4000",
        "prodex.dev/network-tier: monitoring\n      ports:\n        - protocol: TCP\n          port: 4100",
      ),
    }).some((error) => error.includes("monitoring metrics ingress")),
    "gateway monitoring ingress without metrics port accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("secretKey: PRODEX_GATEWAY_METRICS_TOKEN", "secretKey: PRODEX_GATEWAY_TOKEN"),
    }).some((error) => error.includes("dedicated metrics token")),
    "missing dedicated metrics token secret accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "      authorization:\n        type: Bearer\n        credentials:\n          name: prodex-gateway-secrets\n          key: PRODEX_GATEWAY_METRICS_TOKEN\n",
        "",
      ),
    }).some((error) => error.includes("ServiceMonitor must use the dedicated metrics token")),
    "unauthenticated ServiceMonitor accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("key: PRODEX_GATEWAY_METRICS_TOKEN", "key: PRODEX_GATEWAY_TOKEN"),
    }).some((error) => error.includes("ServiceMonitor must not use the gateway root token")),
    "ServiceMonitor gateway root token accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "kind: ConfigMap\nmetadata:\n  name: prodex-gateway-policy",
        "kind: ConfigMap\nmetadata:\n  name: prodex-gateway-policy-missing",
      ),
    }).some((error) => error.includes("missing gateway policy ConfigMap")),
    "missing gateway policy ConfigMap accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        'token_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_METRICS_TOKEN" }',
        'token_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_TOKEN" }',
      ),
    }).some((error) => error.includes("gateway policy must bind the metrics token projected reference")),
    "gateway policy root token binding accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace('role = "viewer"', 'role = "admin"'),
    }).some((error) => error.includes("gateway policy must bind metrics token as viewer")),
    "gateway policy admin metrics token accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("[gateway.state]", "[gateway.state_missing]"),
    }).some((error) => error.includes("gateway policy must configure shared gateway state")),
    "missing gateway shared state policy accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace('backend = "postgres"', 'backend = "file"'),
    }).some((error) => error.includes("gateway policy must use the PostgreSQL state backend")),
    "non-PostgreSQL gateway state backend accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        'postgres_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_POSTGRES_URL" }',
        'postgres_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_WRONG_POSTGRES_URL" }',
      ),
    }).some((error) => error.includes("projected secret reference")),
    "wrong PostgreSQL state projected reference accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        'redis_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_REDIS_URL" }',
        'redis_url_ref = { provider = "kubernetes", name = "PRODEX_GATEWAY_WRONG_REDIS_URL" }',
      ),
    }).some((error) => error.includes("Redis coordination URL")),
    "wrong Redis coordination projected reference accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("mountPath: /var/lib/prodex/policy.toml", "mountPath: /var/lib/prodex/policy-missing.toml"),
    }).some((error) => error.includes("gateway Deployment must mount policy.toml")),
    "missing gateway policy mount accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      compose: valid.compose.replace('entrypoint: ["/usr/local/bin/prodex-gateway"]', ""),
    }).some((error) => error.includes("dedicated gateway entrypoint")),
    "legacy Compose gateway entrypoint accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      compose: valid.compose.replace("target: OPENAI_API_KEY", "target: OPENAI_API_KEY_MISSING"),
    }).some((error) => error.includes("OPENAI_API_KEY must be projected")),
    "missing Compose provider secret projection accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      composePolicy: valid.composePolicy.replace(
        'auth_token_ref = { provider = "compose", name = "PRODEX_GATEWAY_TOKEN" }',
        'auth_token_env = "PRODEX_GATEWAY_TOKEN"',
      ),
    }).some((error) => error.includes("Compose policy must use SecretRef fields")),
    "Compose policy secret environment reference accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      composeSecretFiles: {
        ...valid.composeSecretFiles,
        OPENAI_API_KEY_FILE: {
          ...valid.composeSecretFiles.OPENAI_API_KEY_FILE,
          mode: 0o644,
        },
      },
    }).some((error) => error.includes("must not grant group or other permissions")),
    "public Compose secret source accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      composeSecretFiles: {
        ...valid.composeSecretFiles,
        PRODEX_GATEWAY_TOKEN_FILE: {
          ...valid.composeSecretFiles.PRODEX_GATEWAY_TOKEN_FILE,
          uid: 1000,
        },
      },
    }).some((error) => error.includes("must be owned by container UID 10001")),
    "wrong Compose secret source owner accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      envExample: `${valid.envExample}\nOPENAI_API_KEY=inline-secret\n`,
    }).some((error) => error.includes("OPENAI_API_KEY must not contain a secret value")),
    "secret value field in Compose environment example accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace('command: ["/usr/local/bin/prodex-gateway"]', ""),
    }).some((error) => error.includes("dedicated prodex-gateway entrypoint")),
    "legacy Kubernetes data-plane entrypoint accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        'args: ["serve", "--listen", "0.0.0.0:4000"]',
        'args: ["gateway", "--listen", "0.0.0.0:4000"]',
      ),
    }).some((error) => error.includes("dedicated serve command")),
    "legacy Kubernetes gateway arguments accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      compose: `${valid.compose}\nPRODEX_GATEWAY_TOKEN: \${PRODEX_GATEWAY_TOKEN:-dev}\n`,
    }).some((error) => error.includes("PRODEX_GATEWAY_TOKEN must use a Compose secret file")),
    "gateway token environment variable accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      compose: valid.compose.replace(
        "POSTGRES_PASSWORD_FILE: /run/secrets/PRODEX_POSTGRES_PASSWORD",
        "POSTGRES_PASSWORD: ${PRODEX_POSTGRES_PASSWORD}",
      ),
    }).some((error) => error.includes("POSTGRES_PASSWORD must use a Compose secret file")),
    "Postgres password environment variable accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({ ...valid, envExample: `${valid.envExample}\nPASSWORD=change-me\n` }).some(
      (error) => error.includes("change-me"),
    ),
    "change-me credential accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("serviceAccountName: prodex-gateway", ""),
    }).some((error) => error.includes("gateway Deployment must use")),
    "missing gateway service account binding accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "        - configMapRef:\n            name: prodex-gateway-config",
        "        - configMapRef:\n            name: prodex-gateway-config\n        - secretRef:\n            name: prodex-gateway-secrets",
      ),
    }).some((error) => error.includes("Secret must not be consumed through envFrom")),
    "gateway Secret envFrom accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "      env:\n        - name: PRODEX_CONFIG_PUBLICATION_REPLICA_ID\n          valueFrom:\n            fieldRef:\n              fieldPath: metadata.name\n",
        "",
      ),
    }).some((error) => error.includes("gateway config-publication replica ID")),
    "gateway config-publication replica identity accepted without pod binding",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "        - name: prodex-gateway-secrets\n          mountPath: /run/secrets/prodex\n          readOnly: true\n",
        "",
      ),
    }).some((error) => error.includes("must mount the Secret at /run/secrets/prodex")),
    "missing gateway projected Secret mount accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "          mountPath: /run/secrets/prodex\n          readOnly: true",
        "          mountPath: /run/secrets/prodex\n          subPath: PRODEX_GATEWAY_TOKEN\n          readOnly: true",
      ),
    }).some((error) => error.includes("Secret volume mount must not use subPath")),
    "gateway projected Secret subPath accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("defaultMode: 0440", "defaultMode: 0644"),
    }).some((error) => error.includes("private defaultMode 0440")),
    "world-readable projected Secret mode accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({ ...valid, kubernetes: `${valid.kubernetes}\nserviceAccountName: default\n` }).some(
      (error) => error.includes("default service account"),
    ),
    "default service account accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: `${valid.kubernetes}\nimage: repo/prodex@sha256:0000000000000000000000000000000000000000000000000000000000000000\n`,
    }).some((error) => error.includes("repeated-character placeholder")),
    "all-zero image digest accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: `${valid.kubernetes}\nimage: repo/prodex@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n`,
    }).some((error) => error.includes("repeated-character placeholder")),
    "repeated-character image digest accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: `${valid.kubernetes}\nimage: repo/prodex@sha256:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef\n`,
    }).some((error) => error.includes("repeated-pattern placeholder")),
    "repeated-pattern image digest accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: `${valid.kubernetes}\nimage: repo/prodex@sha256:abc\n`,
    }).some((error) => error.includes("64-character sha256 hex")),
    "short image digest accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: `${valid.kubernetes}\nimage: repo/prodex@sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg\n`,
    }).some((error) => error.includes("64-character sha256 hex")),
    "non-hex image digest accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: `${valid.kubernetes}\nautomountServiceAccountToken: true\n`,
    }).some((error) => error.includes("token automount")),
    "enabled service account token automount accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: `${valid.kubernetes}\nenv:\n  - name: PRODEX_GATEWAY_TOKEN\n    value: static-token\n`,
    }).some((error) => error.includes("secret-bearing environment variables")),
    "literal Kubernetes secret env accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("seccompProfile:\n          type: RuntimeDefault\n", ""),
    }).some((error) => error.includes("RuntimeDefault seccomp profile")),
    "missing seccomp profile accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("readOnlyRootFilesystem: true", "readOnlyRootFilesystem: false"),
    }).some((error) => error.includes("gateway Deployment read-only container filesystem")),
    "unhardened gateway workload accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("type: RuntimeDefault", "type: Unconfined"),
    }).some((error) => error.includes("RuntimeDefault seccomp profile")),
    "weakened seccomp profile accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("topologySpreadConstraints:", ""),
    }).some((error) => error.includes("gateway pod topology spreading")),
    "missing gateway topology spread constraints accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: `${valid.kubernetes}\ningress:\n  - from:\n      - namespaceSelector: {}\n`,
    }).some((error) => error.includes("ingress must not allow every namespace")),
    "unrestricted namespace ingress accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("terminationGracePeriodSeconds: 45", ""),
    }).some((error) => error.includes("termination grace period")),
    "missing gateway termination grace period accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("serviceAccountName: prodex-gateway-migration", ""),
    }).some((error) => error.includes("migration Job must use")),
    "missing migration service account binding accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("serviceAccountName: prodex-gateway-migration", "serviceAccountName: prodex-gateway"),
    }).some((error) => error.includes("migration Job must not use")),
    "migration gateway service account binding accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("app.kubernetes.io/name: prodex-gateway-migration", ""),
    }).some((error) => error.includes("migration pod network policy label")),
    "missing migration network policy label accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replaceAll("name: prodex-gateway-migration-secrets", ""),
    }).some((error) => error.includes("migration-only database secret")),
    "missing migration-only database secret accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "        - name: migration\n          args:",
        "        - name: migration\n          envFrom:\n            - secretRef:\n                name: prodex-gateway-migration-secrets\n          args:",
      ),
    }).some((error) => error.includes("migration database URL must not be consumed through envFrom")),
    "migration database URL envFrom accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replaceAll("name: prodex-control-plane-secrets", ""),
    }).some((error) => error.includes("control-plane storage/coordination secret")),
    "missing control-plane storage secret accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "            - name: prodex-control-plane-secrets\n              mountPath: /run/secrets/prodex\n              readOnly: true\n",
        "",
      ),
    }).some((error) => error.includes("must mount projected secrets")),
    "missing control-plane workload secret binding accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "                  name: prodex-control-plane-secrets\n                  items:",
        "                  name: prodex-gateway-secrets\n                  items:",
      ),
    }).some((error) => error.includes("must not mount gateway or provider capabilities")),
    "control-plane gateway secret binding accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace("  replicas: 1\n  template:", "  replicas: 0\n  template:"),
    }).some((error) => error.includes("exactly one replica")),
    "scaled-to-zero control-plane accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace('service_mode = "control-plane"', 'service_mode = "gateway"'),
    }).some((error) => error.includes("typed service_mode=control-plane")),
    "gateway-mode control-plane policy accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        '    service_mode = "control-plane"\n\n    [secrets]',
        '    service_mode = "control-plane"\n\n    [gateway]\n    provider_api_key_ref = { provider = "kubernetes", name = "OPENAI_API_KEY" }\n\n    [secrets]',
      ),
    }).some((error) => error.includes("must not configure data-plane, SSO, or outbound capabilities")),
    "control-plane provider capability accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        '    service_mode = "control-plane"\n\n    [secrets]',
        '    service_mode = "control-plane"\n\n    [gateway.sso]\n    oidc_issuer = "https://idp.example.com"\n    oidc_audience = "prodex"\n\n    [secrets]',
      ),
    }).some((error) => error.includes("must not configure data-plane, SSO, or outbound capabilities")),
    "control-plane SSO capability accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replaceAll('    backend = "postgres"', '    backend = "file"'),
    }).some((error) => error.includes("PostgreSQL shared state backend")),
    "control-plane local state accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace('    role = "admin"', '    role = "viewer"'),
    }).some((error) => error.includes("explicit admin role")),
    "control-plane viewer-only token accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "        - name: prodex-control-plane\n          command:",
        "        - name: prodex-control-plane\n          envFrom:\n            - secretRef:\n                name: prodex-control-plane-secrets\n          command:",
      ),
    }).some((error) => error.includes("must not be consumed through env or envFrom")),
    "control-plane secret envFrom accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "          env:\n            - name: PRODEX_CONFIG_PUBLICATION_REPLICA_ID\n              valueFrom:\n                fieldRef:\n                  fieldPath: metadata.name\n",
        "",
      ),
    }).some((error) => error.includes("control-plane config-publication replica ID")),
    "control-plane config-publication replica identity accepted without pod binding",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "    - secretKey: PRODEX_GATEWAY_REDIS_URL\n---\nkind: ServiceMonitor",
        "    - secretKey: PRODEX_GATEWAY_REDIS_URL\n    - secretKey: OPENAI_API_KEY\n---\nkind: ServiceMonitor",
      ),
    }).some((error) => error.includes("must contain only admin, PostgreSQL, and Redis")),
    "control-plane provider secret accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        'args: ["serve", "--listen", "0.0.0.0:4100"]',
        'args: ["serve"]',
      ),
    }).some((error) => error.includes("listen explicitly on 0.0.0.0:4100")),
    "implicit loopback control-plane listen accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: `${valid.kubernetes}\nkind: NetworkPolicy\nmetadata:\n  name: prodex-control-plane\nspec:\n  egress:\n    - to:\n        - ipBlock:\n            cidr: 0.0.0.0/0\n      ports:\n        - protocol: TCP\n          port: 443\n`,
    }).some((error) => error.includes("control-plane NetworkPolicy must not allow public provider egress")),
    "control-plane public provider egress accepted",
  );
}

function readComposeSecretFiles(envPath) {
  const values = {};
  for (const rawLine of fs.readFileSync(envPath, "utf8").split(/\r?\n/u)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const separator = line.indexOf("=");
    if (separator < 1) continue;
    const name = line.slice(0, separator).trim();
    let value = line.slice(separator + 1).trim();
    if (
      value.length >= 2 &&
      ((value.startsWith('"') && value.endsWith('"')) ||
        (value.startsWith("'") && value.endsWith("'")))
    ) {
      value = value.slice(1, -1);
    }
    values[name] = value;
  }

  return Object.fromEntries(
    COMPOSE_SECRET_FILE_ENVIRONMENTS.map((environment) => {
      const filePath = values[environment];
      if (!filePath) return [environment, { error: "path is missing" }];
      try {
        const stat = fs.lstatSync(filePath);
        return [
          environment,
          {
            isFile: stat.isFile(),
            isSymbolicLink: stat.isSymbolicLink(),
            mode: stat.mode & 0o777,
            uid: stat.uid,
          },
        ];
      } catch {
        return [environment, { error: "path cannot be inspected" }];
      }
    }),
  );
}

function readInputs(secretEnvPath) {
  return {
    compose: fs.readFileSync("compose.yaml", "utf8"),
    composePolicy: fs.readFileSync("deploy/compose-gateway-policy.toml", "utf8"),
    composeSecretFiles: secretEnvPath ? readComposeSecretFiles(secretEnvPath) : undefined,
    composeSecretEnvPath: secretEnvPath,
    envExample: fs.readFileSync("deploy/gateway.env.example", "utf8"),
    dockerfile: fs.readFileSync("Dockerfile", "utf8"),
    kubernetes: fs.readFileSync("deploy/kubernetes/prodex-gateway.yaml", "utf8"),
    deploymentGuide: fs.readFileSync("docs/deployment.md", "utf8"),
    backupRunbook: fs.readFileSync("docs/backup-restore.md", "utf8"),
  };
}

function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  const secretEnvIndex = process.argv.indexOf("--secret-env");
  const secretEnvPath = secretEnvIndex >= 0 ? process.argv[secretEnvIndex + 1] : undefined;
  if (secretEnvIndex >= 0 && !secretEnvPath) {
    process.stderr.write("--secret-env requires a Compose environment file path\n");
    process.exitCode = 2;
    return;
  }
  const checks = validateDeploymentSecurity(readInputs(secretEnvPath));
  if (checks.length > 0) {
    for (const check of checks) process.stderr.write(`${check}\n`);
    process.exitCode = 1;
  }
}

main();
