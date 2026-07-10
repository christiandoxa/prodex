pub(crate) const DASHBOARD_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Prodex Dashboard</title>
<style>
:root{color-scheme:light dark;--bg:#f6f7f9;--fg:#111827;--muted:#667085;--line:#d9dee7;--panel:#fff;--accent:#0f766e;--warn:#b45309;--bad:#b42318;--soft:rgba(15,118,110,.10)}
@media (prefers-color-scheme:dark){:root{--bg:#101214;--fg:#eef2f7;--muted:#9aa4b2;--line:#2b333d;--panel:#171a1f;--accent:#2dd4bf;--warn:#f59e0b;--bad:#fb7185;--soft:rgba(45,212,191,.12)}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
header{padding:20px 24px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between;gap:16px}h1{font-size:22px;margin:0}main{padding:22px 24px;max-width:1320px;margin:auto}.muted{color:var(--muted)}
.grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:12px;margin-bottom:18px}.stat,.panel{background:var(--panel);border:1px solid var(--line);border-radius:8px}.stat{padding:14px}.stat b{font-size:26px;display:block}.panel{margin:16px 0;overflow:hidden}.panel h2{font-size:16px;margin:0;padding:14px 16px;border-bottom:1px solid var(--line)}.body{padding:14px 16px;display:grid;gap:12px}
.toolbar{display:flex;gap:8px;align-items:center;flex-wrap:wrap}button,select{border:1px solid var(--line);background:var(--panel);color:var(--fg);border-radius:6px;padding:7px 10px}button{cursor:pointer}button.primary{background:var(--accent);color:#fff;border-color:var(--accent)}button.danger{color:var(--bad)}input{background:var(--panel);color:var(--fg);border:1px solid var(--line);border-radius:6px;padding:8px;min-width:220px}
table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:10px 12px;border-bottom:1px solid var(--line);vertical-align:top}th{font-size:12px;color:var(--muted);font-weight:600}tr:last-child td{border-bottom:0}.pill{display:inline-block;border:1px solid var(--line);border-radius:999px;padding:2px 8px;font-size:12px;margin:0 4px 4px 0}.ok{color:var(--accent)}.warn{color:var(--warn)}.bad{color:var(--bad)}.small{font-size:12px}.actions{display:flex;gap:6px;flex-wrap:wrap}
.cmd{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;background:rgba(127,127,127,.12);padding:8px;border-radius:6px;overflow:auto;white-space:pre-wrap}.callout{border:1px solid var(--line);background:var(--soft);border-radius:8px;padding:12px}.split{display:grid;grid-template-columns:1fr 1fr;gap:12px}.scroll{overflow:auto}.nowrap{white-space:nowrap}
@media (max-width:900px){header{align-items:flex-start;flex-direction:column}.grid,.split{grid-template-columns:repeat(2,minmax(0,1fr))}th.hide,td.hide{display:none}}
@media (max-width:620px){main{padding:14px}.grid,.split{grid-template-columns:1fr}table{font-size:12px}th,td{padding:8px}.hide-sm{display:none}}
</style>
</head>
<body>
<header><div><h1>Prodex Dashboard</h1><div class="muted">Local control plane for profiles, providers, models, quota, runtime, and gateway commands</div></div><div class="toolbar"><button id="refresh" class="primary">Refresh</button><span id="status" class="muted"></span></div></header>
<main>
<section class="grid">
<div class="stat"><span class="muted">Profiles</span><b id="total">-</b></div>
<div class="stat"><span class="muted">Providers</span><b id="providerCount">-</b></div>
<div class="stat"><span class="muted">Models</span><b id="modelCount">-</b></div>
<div class="stat"><span class="muted">Ready quota</span><b id="ready">-</b></div>
</section>
<section class="panel"><h2>Overview</h2><div class="body">
<div>Active profile: <b id="active">-</b></div>
<div id="nextStep" class="callout muted">Loading setup path...</div>
<div class="toolbar"><input id="profileName" placeholder="openai profile name"><button id="create" class="primary">Create OpenAI profile</button><button id="login">Login command</button><button id="add">Add command</button><button id="import">Import current command</button></div>
<div id="command" class="cmd muted">Choose an action.</div>
</div></section>
<section class="panel"><h2>Setup / Add provider</h2><div class="body">
<div class="toolbar"><label for="providerSelect" class="muted">Provider</label><select id="providerSelect"></select></div>
<div id="setupCommands" class="split"></div>
<div class="scroll"><table><thead><tr><th>Provider</th><th>Auth</th><th>Default</th><th>Profiles</th><th>Routes</th><th>Launch</th></tr></thead><tbody id="providerRows"></tbody></table></div>
</div></section>
<section class="panel"><h2>Models</h2><div class="body">
<div class="toolbar"><label for="modelProvider" class="muted">Filter</label><select id="modelProvider"><option value="all">All providers</option></select><span id="modelHidden" class="muted small"></span></div>
<div class="scroll"><table><thead><tr><th>Provider</th><th>Model</th><th>Recommended</th><th>Context</th><th class="hide">Capabilities</th><th>Launch</th></tr></thead><tbody id="modelRows"></tbody></table></div>
</div></section>
<section class="panel"><h2>Usage</h2><table><thead><tr><th>Profile</th><th>Account</th><th>Status</th><th>Main</th><th>Reset</th></tr></thead><tbody id="usageRows"></tbody></table></section>
<section class="panel"><h2>Accounts</h2><table><thead><tr><th>Profile</th><th>Provider</th><th>Auth</th><th class="hide-sm">CODEX_HOME</th><th>Actions</th></tr></thead><tbody id="accountRows"></tbody></table></section>
<section class="panel"><h2>Runtime / Gateway</h2><div class="body" id="runtimePanel"></div></section>
</main>
<script>
const $ = (id) => document.getElementById(id);
let providerData=[], modelData=[], selectedProvider="openai";
function td(text, cls){const e=document.createElement("td");e.textContent=text ?? "-";if(cls)e.className=cls;return e}
function pill(text){const s=document.createElement("span");s.className="pill";s.textContent=text;return s}
function cmdBlock(label, text){const wrap=document.createElement("div");const name=document.createElement("div");name.className="muted small";name.textContent=label;const pre=document.createElement("div");pre.className="cmd";pre.textContent=Array.isArray(text)?text.join("\n"):text;wrap.append(name,pre);return wrap}
async function api(path, options){const r=await fetch(path,{headers:{"content-type":"application/json"},...options});if(!r.ok)throw new Error(await r.text());return r.json()}
function statusClass(status){if(!status)return "";const s=status.toLowerCase();if(s.includes("ready")||s.includes("available")||s.includes("ok"))return "ok";if(s.includes("error")||s.includes("blocked")||s.includes("exceeded"))return "bad";return "warn"}
function fmtContext(n){return n?Number(n).toLocaleString()+" tokens":"-"}
async function refresh(){
  $("status").textContent="Loading...";
  try{
    const [state, accounts, usage, providers, models, runtime] = await Promise.all([api("/api/state"),api("/api/accounts"),api("/api/usage"),api("/api/providers"),api("/api/models"),api("/api/runtime-status")]);
    providerData=providers.providers||[]; modelData=models.models||[];
    $("active").textContent=state.activeProfile||"-";$("total").textContent=usage.summary.total;$("ready").textContent=usage.summary.ready;$("providerCount").textContent=providerData.length;$("modelCount").textContent=modelData.length;
    renderNextStep(state, providerData);renderProviderSelect(providerData);renderProviders(providerData);renderModels();renderUsage(usage.profiles);renderAccounts(accounts.accounts);renderRuntime(runtime);
    $("status").textContent="Updated "+new Date().toLocaleTimeString();
  }catch(err){$("status").textContent="Error: "+err.message}
}
function renderNextStep(state, providers){
  const box=$("nextStep");
  if(!state.profileCount){box.textContent="Fresh setup: create an OpenAI profile or choose a provider below. The dashboard only generates commands for provider secrets; it does not store masked keys.";return}
  const active=providers.find(p=>p.active);box.textContent=active?`Ready: ${active.label} is active. Pick a model below, run ${active.commands.launch}, then check quota/logs here.`:"Profiles exist. Pick an active profile in Accounts, or use a provider setup command below.";
}
function renderProviderSelect(rows){
  const select=$("providerSelect");const current=select.value||selectedProvider;select.replaceChildren();
  rows.forEach(p=>{const o=document.createElement("option");o.value=p.id;o.textContent=p.label;select.append(o)});
  select.value=rows.some(p=>p.id===current)?current:(rows[0]?.id||"openai");selectedProvider=select.value;renderSetupCommands();
  const filter=$("modelProvider");const fcur=filter.value||"all";filter.replaceChildren();const all=document.createElement("option");all.value="all";all.textContent="All providers";filter.append(all);
  rows.forEach(p=>{const o=document.createElement("option");o.value=p.id;o.textContent=p.label;filter.append(o)});filter.value=rows.some(p=>p.id===fcur)?fcur:"all";
}
function renderSetupCommands(){
  selectedProvider=$("providerSelect").value;const p=providerData.find(x=>x.id===selectedProvider);const box=$("setupCommands");box.replaceChildren();if(!p)return;
  box.append(cmdBlock("Setup / import / login", p.commands.setup),cmdBlock("Launch", p.commands.launch),cmdBlock("Quota", p.commands.quota),cmdBlock("Gateway", p.commands.gateway));
}
function renderProviders(rows){
  const body=$("providerRows");body.replaceChildren();
  rows.forEach(p=>{const tr=document.createElement("tr");tr.onclick=()=>{$("providerSelect").value=p.id;renderSetupCommands()};
    const name=td(p.label+(p.active?" *":""));if(p.active)name.className="ok";tr.append(name,td(p.auth),td(p.defaultModel),td(String(p.configuredProfiles||0)));
    const routes=td("");(p.availableThrough||[]).forEach(r=>routes.append(pill(r)));tr.append(routes,td(p.commands.launch,"cmd"));body.append(tr);
  });
}
function renderModels(){
  const body=$("modelRows");body.replaceChildren();const filter=$("modelProvider").value;let rows=modelData.filter(m=>filter==="all"||m.provider===filter);const hidden=Math.max(0,rows.length-80);rows=rows.slice(0,80);
  rows.forEach(m=>{const tr=document.createElement("tr");tr.append(td(m.providerName||m.provider),td(`${m.display_name||m.id}\n${m.id}`),td(m.recommended?"yes":""),td(fmtContext(m.context_window)),td([...(m.endpoints||[]),...Object.entries(m.feature_flags||{}).filter(([,v])=>v).map(([k])=>k)].join(", "),"hide"),td(m.launchCommand,"cmd"));body.append(tr)});
  $("modelHidden").textContent=hidden?`${hidden} more rows hidden; narrow the provider filter.`:"";
}
function renderUsage(rows){
  const body=$("usageRows");body.replaceChildren();
  if(!rows.length){const tr=document.createElement("tr");const cell=td("No profiles yet. Use Setup / Add provider first.");cell.colSpan=5;tr.append(cell);body.append(tr);return}
  rows.forEach(row=>{const tr=document.createElement("tr");const q=row.quota||{};const name=td(row.name+(row.active?" *":""));if(row.active)name.className="ok";tr.append(name,td(q.account||row.workspaceId||"-"),td(q.status||"-",statusClass(q.status)),td(q.main||"-"),td(q.reset||"-"));body.append(tr)});
}
function renderAccounts(rows){
  const body=$("accountRows");body.replaceChildren();
  if(!rows.length){const tr=document.createElement("tr");const cell=td("No accounts configured yet.");cell.colSpan=5;tr.append(cell);body.append(tr);return}
  rows.forEach(row=>{const tr=document.createElement("tr");const profile=td(row.name+(row.active?" *":""));if(row.active)profile.className="ok";tr.append(profile,td(row.providerName));const auth=td("");auth.append(pill(row.auth.label+(row.auth.quotaCompatible?" quota":"")));tr.append(auth,td(row.codexHome,"hide-sm"));
    const actions=td("");actions.className="actions";const use=document.createElement("button");use.textContent="Use";use.onclick=async()=>{await api("/api/profile/active",{method:"POST",body:JSON.stringify({profile:row.name})});refresh()};const remove=document.createElement("button");remove.textContent="Remove";remove.className="danger";remove.onclick=async()=>{if(confirm("Remove profile entry "+row.name+"? Managed home is not deleted.")){await api("/api/profile/"+encodeURIComponent(row.name),{method:"DELETE"});refresh()}};actions.append(use,remove);tr.append(actions);body.append(tr)});
}
function renderRuntime(data){const box=$("runtimePanel");box.replaceChildren();box.append(cmdBlock("Runtime status", [`status: ${data.runtime.status}`,`log dir: ${data.runtime.logDir}`,`latest log: ${data.runtime.latestLog||"-"}`,data.runtime.doctorCommand]),cmdBlock("Gateway commands", [data.gateway.startCommand,data.gateway.providersCommand,data.gateway.modelsCommand]));}
function command(kind){const name=$("profileName").value.trim()||"<name>";$("command").textContent=kind==="login"?`prodex login --profile ${name}`:kind==="add"?`prodex profile add ${name} --activate`:`prodex profile import-current ${name}`}
async function createProfile(){const name=$("profileName").value.trim();if(!name){$("command").textContent="Enter a profile name first.";return}const result=await api("/api/profile",{method:"POST",body:JSON.stringify({name,activate:true})});$("command").textContent=`Created ${result.profile}. Next: prodex login --profile ${result.profile}`;refresh()}
$("refresh").onclick=refresh;$("providerSelect").onchange=renderSetupCommands;$("modelProvider").onchange=renderModels;$("login").onclick=()=>command("login");$("add").onclick=()=>command("add");$("import").onclick=()=>command("import");$("create").onclick=()=>createProfile().catch(err=>$("command").textContent="Create failed: "+err.message);
refresh();setInterval(refresh,10000);
</script>
</body>
</html>"#;
