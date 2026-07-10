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
  ['"--url-env"', "gateway migration URL env flag"],
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
  ["name: prodex-gateway-policy", "gateway policy ConfigMap"],
  ['token_env = "PRODEX_GATEWAY_METRICS_TOKEN"', "metrics viewer admin token policy binding"],
  ['role = "viewer"', "metrics viewer admin token role"],
  ["mountPath: /var/lib/prodex/policy.toml", "gateway policy file mount"],
  ["subPath: policy.toml", "gateway policy ConfigMap subPath"],
  ['backend = "postgres"', "PostgreSQL gateway state backend"],
  ['postgres_url_env = "PRODEX_GATEWAY_POSTGRES_URL"', "PostgreSQL gateway state URL env"],
  ["name: prodex-default-deny", "namespace default-deny network policy"],
  ["prodex.dev/network-tier: ingress", "restricted ingress namespace selector"],
  ["prodex.dev/network-tier: monitoring", "monitoring namespace selector for metrics scraping"],
  ["k8s-app: kube-dns", "DNS-only egress target"],
  ["prodex.dev/network-tier: data-store", "data-store egress namespace selector"],
  ["port: 5432", "PostgreSQL egress port"],
  ["port: 6379", "Redis egress port"],
  ["except:", "private network exceptions on broad HTTPS egress"],
  ["10.0.0.0/8", "RFC1918 10/8 excluded from broad HTTPS egress"],
  ["172.16.0.0/12", "RFC1918 172.16/12 excluded from broad HTTPS egress"],
  ["192.168.0.0/16", "RFC1918 192.168/16 excluded from broad HTTPS egress"],
]);

const REQUIRED_CONTROL_PLANE_MARKERS = Object.freeze([
  ["name: prodex-gateway", "gateway deployment/service surface"],
  ["name: prodex-control-plane", "control-plane deployment/service surface"],
  ["name: prodex-control-plane-secrets", "control-plane storage/coordination secret"],
  ["serviceAccountName: prodex-control-plane", "explicit control-plane service account"],
  ["app.kubernetes.io/component: control-plane", "control-plane component label"],
  ["placeholder-until-control-plane-adapter-exists", "explicit control-plane placeholder"],
  ['command: ["/usr/local/bin/prodex-control-plane"]', "control-plane binary command"],
  ['args: ["serve"]', "control-plane serve command is explicit and gated by replicas 0"],
  ["replicas: 0", "control-plane placeholder scaled to zero until adapter exists"],
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
    envExamplePath = "deploy/gateway.env.example",
    dockerfilePath = "Dockerfile",
    kubernetesPath = "deploy/kubernetes/prodex-gateway.yaml",
    backupRunbookPath = "docs/backup-restore.md",
    compose,
    envExample,
    dockerfile,
    kubernetes,
    backupRunbook,
  } = inputs;

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

  if (/\$\{PRODEX_GATEWAY_TOKEN:-/u.test(compose)) {
    checks.push(`${composePath}: PRODEX_GATEWAY_TOKEN must be required, not defaulted`);
  }
  if (/POSTGRES_PASSWORD:\s*\$\{PRODEX_POSTGRES_PASSWORD:-/u.test(compose)) {
    checks.push(`${composePath}: POSTGRES_PASSWORD must not default to a static value`);
  }
  if (!/POSTGRES_PASSWORD:\s*\$\{PRODEX_POSTGRES_PASSWORD:\?/u.test(compose)) {
    checks.push(`${composePath}: POSTGRES_PASSWORD must require PRODEX_POSTGRES_PASSWORD`);
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
    if (!/\[\[gateway\.admin_tokens\]\]/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must define admin token bindings`);
    }
    if (!/token_env\s*=\s*"PRODEX_GATEWAY_METRICS_TOKEN"/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must bind the metrics token env var`);
    }
    if (!/role\s*=\s*"viewer"/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must bind metrics token as viewer`);
    }
    if (/token_env\s*=\s*"PRODEX_GATEWAY_TOKEN"/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must not bind the root token as metrics viewer`);
    }
    if (!/\[gateway\.state\]/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must configure shared gateway state`);
    }
    if (!/backend\s*=\s*"postgres"/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must use the PostgreSQL state backend`);
    }
    if (!/postgres_url_env\s*=\s*"PRODEX_GATEWAY_POSTGRES_URL"/u.test(gatewayPolicyConfig)) {
      checks.push(`${kubernetesPath}: gateway policy must read PostgreSQL state URL from PRODEX_GATEWAY_POSTGRES_URL`);
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
    if (!/secretRef:\s*\n\s*name:\s*prodex-gateway-secrets/u.test(gatewayDeployment)) {
      checks.push(`${kubernetesPath}: gateway workload must mount prodex-gateway-secrets`);
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
  }
  const controlPlaneDeployment = kubernetesDocumentByKindAndName(kubernetes, "Deployment", "prodex-control-plane");
  if (!controlPlaneDeployment) {
    checks.push(`${kubernetesPath}: missing control-plane Deployment`);
  } else {
    requireWorkloadHardening(checks, controlPlaneDeployment, kubernetesPath, "control-plane Deployment");
    if (!/^\s*serviceAccountName:\s*prodex-control-plane\s*$/mu.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane Deployment must use prodex-control-plane service account`);
    }
    if (!/secretRef:\s*\n\s*name:\s*prodex-control-plane-secrets/u.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane workload must mount prodex-control-plane-secrets`);
    }
    if (/secretRef:\s*\n\s*name:\s*prodex-gateway-secrets/u.test(controlPlaneDeployment)) {
      checks.push(`${kubernetesPath}: control-plane workload must not mount prodex-gateway-secrets`);
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
  for (const [needle, description] of REQUIRED_BACKUP_MARKERS) {
    requireIncludes(checks, backupRunbook, needle, backupRunbookPath, description);
  }

  return checks;
}

function validFixture() {
  return {
    compose: `
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
  PRODEX_GATEWAY_TOKEN: \${PRODEX_GATEWAY_TOKEN:?required}
  POSTGRES_PASSWORD: \${PRODEX_POSTGRES_PASSWORD:?required}
`,
    envExample: "PRODEX_GATEWAY_TOKEN=\nPRODEX_GATEWAY_POSTGRES_URL=\n",
    dockerfile: `
COPY --from=builder /workspace/target/release/prodex-gateway /usr/local/bin/prodex-gateway
COPY --from=builder /workspace/target/release/prodex-control-plane /usr/local/bin/prodex-control-plane
USER prodex
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
          securityContext:
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            capabilities:
              drop: ["ALL"]
      envFrom:
        - secretRef:
            name: prodex-gateway-secrets
      volumeMounts:
        - name: gateway-policy
          mountPath: /var/lib/prodex/policy.toml
          subPath: policy.toml
          readOnly: true
      volumes:
        - name: gateway-policy
          configMap:
            name: prodex-gateway-policy
---
kind: Deployment
metadata:
  name: prodex-control-plane
spec:
  template:
    spec:
      serviceAccountName: prodex-control-plane
      securityContext:
        runAsNonRoot: true
        seccompProfile:
          type: RuntimeDefault
      containers:
        - name: prodex-control-plane
          securityContext:
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            capabilities:
              drop: ["ALL"]
      envFrom:
        - secretRef:
            name: prodex-control-plane-secrets
---
kind: ConfigMap
metadata:
  name: prodex-gateway-policy
data:
  policy.toml: |
    [gateway.state]
    backend = "postgres"
    postgres_url_env = "PRODEX_GATEWAY_POSTGRES_URL"

    [[gateway.admin_tokens]]
    name = "prometheus"
    token_env = "PRODEX_GATEWAY_METRICS_TOKEN"
    role = "viewer"
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
          securityContext:
            allowPrivilegeEscalation: false
            readOnlyRootFilesystem: true
            capabilities:
              drop: ["ALL"]
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
"--url-env"
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
except:
10.0.0.0/8
172.16.0.0/12
192.168.0.0/16
app.kubernetes.io/component: control-plane
placeholder-until-control-plane-adapter-exists
command: ["/usr/local/bin/prodex-control-plane"]
args: ["serve"]
replicas: 0
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
      kubernetes: valid.kubernetes.replace('token_env = "PRODEX_GATEWAY_METRICS_TOKEN"', 'token_env = "PRODEX_GATEWAY_TOKEN"'),
    }).some((error) => error.includes("gateway policy must bind the metrics token env var")),
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
        'postgres_url_env = "PRODEX_GATEWAY_POSTGRES_URL"',
        'postgres_url_env = "PRODEX_GATEWAY_WRONG_POSTGRES_URL"',
      ),
    }).some((error) => error.includes("PRODEX_GATEWAY_POSTGRES_URL")),
    "wrong PostgreSQL state URL env accepted",
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
      compose: `${valid.compose}\nPRODEX_GATEWAY_TOKEN: \${PRODEX_GATEWAY_TOKEN:-dev}\n`,
    }).some((error) => error.includes("PRODEX_GATEWAY_TOKEN must be required")),
    "defaulted gateway token accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      compose: valid.compose.replace(
        "POSTGRES_PASSWORD: ${PRODEX_POSTGRES_PASSWORD:?required}",
        "POSTGRES_PASSWORD: ${PRODEX_POSTGRES_PASSWORD}",
      ),
    }).some((error) => error.includes("POSTGRES_PASSWORD must require")),
    "optional Postgres password accepted",
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
      kubernetes: valid.kubernetes.replace("secretRef:\n            name: prodex-gateway-secrets", ""),
    }).some((error) => error.includes("gateway workload must mount")),
    "missing gateway workload secret binding accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "secretRef:\n            name: prodex-gateway-secrets",
        "secretRef:\n            name: prodex-control-plane-secrets",
      ),
    }).some((error) => error.includes("gateway workload must not mount")),
    "gateway control-plane secret binding accepted",
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
      kubernetes: valid.kubernetes.replace("name: prodex-gateway-migration-secrets", ""),
    }).some((error) => error.includes("migration-only database secret")),
    "missing migration-only database secret accepted",
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
      kubernetes: valid.kubernetes.replace("secretRef:\n            name: prodex-control-plane-secrets", ""),
    }).some((error) => error.includes("control-plane workload must mount")),
    "missing control-plane workload secret binding accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: valid.kubernetes.replace(
        "secretRef:\n            name: prodex-control-plane-secrets",
        "secretRef:\n            name: prodex-gateway-secrets",
      ),
    }).some((error) => error.includes("control-plane workload must not mount")),
    "control-plane gateway secret binding accepted",
  );
  assertSelfTest(
    validateDeploymentSecurity({
      ...valid,
      kubernetes: `${valid.kubernetes}\nkind: NetworkPolicy\nmetadata:\n  name: prodex-control-plane\nspec:\n  egress:\n    - to:\n        - ipBlock:\n            cidr: 0.0.0.0/0\n      ports:\n        - protocol: TCP\n          port: 443\n`,
    }).some((error) => error.includes("control-plane NetworkPolicy must not allow public provider egress")),
    "control-plane public provider egress accepted",
  );
}

function readInputs() {
  return {
    compose: fs.readFileSync("compose.yaml", "utf8"),
    envExample: fs.readFileSync("deploy/gateway.env.example", "utf8"),
    dockerfile: fs.readFileSync("Dockerfile", "utf8"),
    kubernetes: fs.readFileSync("deploy/kubernetes/prodex-gateway.yaml", "utf8"),
    backupRunbook: fs.readFileSync("docs/backup-restore.md", "utf8"),
  };
}

function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  const checks = validateDeploymentSecurity(readInputs());
  if (checks.length > 0) {
    for (const check of checks) process.stderr.write(`${check}\n`);
    process.exitCode = 1;
  }
}

main();
