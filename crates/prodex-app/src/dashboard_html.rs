pub(crate) const DASHBOARD_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Prodex Dashboard</title>
<style>
:root{color-scheme:light dark;--bg:#f6f7f9;--fg:#111827;--muted:#667085;--line:#d9dee7;--panel:#fff;--accent:#0f766e;--warn:#b45309;--bad:#b42318}
@media (prefers-color-scheme:dark){:root{--bg:#101214;--fg:#eef2f7;--muted:#9aa4b2;--line:#2b333d;--panel:#171a1f;--accent:#2dd4bf;--warn:#f59e0b;--bad:#fb7185}}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
header{padding:20px 24px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between;gap:16px}
h1{font-size:22px;margin:0}main{padding:22px 24px;max-width:1280px;margin:auto}.grid{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:12px;margin-bottom:18px}
.stat,.panel{background:var(--panel);border:1px solid var(--line);border-radius:8px}.stat{padding:14px}.stat b{font-size:26px;display:block}.muted{color:var(--muted)}.panel{margin:16px 0;overflow:hidden}
.panel h2{font-size:16px;margin:0;padding:14px 16px;border-bottom:1px solid var(--line)}.toolbar{display:flex;gap:8px;align-items:center;flex-wrap:wrap}
button{border:1px solid var(--line);background:var(--panel);color:var(--fg);border-radius:6px;padding:7px 10px;cursor:pointer}button.primary{background:var(--accent);color:#fff;border-color:var(--accent)}button.danger{color:var(--bad)}
table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:10px 12px;border-bottom:1px solid var(--line);vertical-align:top}th{font-size:12px;color:var(--muted);font-weight:600}tr:last-child td{border-bottom:0}
.pill{display:inline-block;border:1px solid var(--line);border-radius:999px;padding:2px 8px;font-size:12px}.ok{color:var(--accent)}.warn{color:var(--warn)}.bad{color:var(--bad)}.cmd{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;background:rgba(127,127,127,.12);padding:8px;border-radius:6px;overflow:auto}
input{background:var(--panel);color:var(--fg);border:1px solid var(--line);border-radius:6px;padding:8px;min-width:220px}.actions{display:flex;gap:6px;flex-wrap:wrap}.small{font-size:12px}
@media (max-width:860px){header{align-items:flex-start;flex-direction:column}.grid{grid-template-columns:repeat(2,minmax(0,1fr))}th:nth-child(4),td:nth-child(4){display:none}}
@media (max-width:560px){main{padding:14px}.grid{grid-template-columns:1fr}table{font-size:12px}th,td{padding:8px}}
</style>
</head>
<body>
<header><div><h1>Prodex Dashboard</h1><div class="muted">Profiles, active account, and quota usage</div></div><div class="toolbar"><button id="refresh" class="primary">Refresh</button><span id="status" class="muted"></span></div></header>
<main>
<section class="grid">
<div class="stat"><span class="muted">Profiles</span><b id="total">-</b></div>
<div class="stat"><span class="muted">Ready</span><b id="ready">-</b></div>
<div class="stat"><span class="muted">Blocked</span><b id="blocked">-</b></div>
<div class="stat"><span class="muted">Errors</span><b id="errors">-</b></div>
</section>
<section class="panel"><h2>Account Settings</h2><div style="padding:14px 16px;display:grid;gap:12px">
<div>Active profile: <b id="active">-</b></div>
<div class="toolbar"><input id="profileName" placeholder="profile name"><button id="create" class="primary">Create profile</button><button id="login">Login command</button><button id="add">Add command</button><button id="import">Import current command</button></div>
<div id="command" class="cmd muted">Choose an action.</div>
</div></section>
<section class="panel"><h2>Usage</h2><table><thead><tr><th>Profile</th><th>Account</th><th>Status</th><th>Main</th><th>Reset</th></tr></thead><tbody id="usageRows"></tbody></table></section>
<section class="panel"><h2>Accounts</h2><table><thead><tr><th>Profile</th><th>Provider</th><th>Auth</th><th>CODEX_HOME</th><th>Actions</th></tr></thead><tbody id="accountRows"></tbody></table></section>
</main>
<script>
const $ = (id) => document.getElementById(id);
function td(text, cls){const e=document.createElement("td");e.textContent=text ?? "-";if(cls)e.className=cls;return e}
function pill(text){const s=document.createElement("span");s.className="pill";s.textContent=text;return s}
async function api(path, options){const r=await fetch(path,{headers:{"content-type":"application/json"},...options});if(!r.ok)throw new Error(await r.text());return r.json()}
function statusClass(status){if(!status)return "";const s=status.toLowerCase();if(s.includes("ready")||s.includes("available")||s.includes("ok"))return "ok";if(s.includes("error")||s.includes("blocked")||s.includes("exceeded"))return "bad";return "warn"}
async function refresh(){
  $("status").textContent="Loading...";
  try{
    const [state, accounts, usage] = await Promise.all([api("/api/state"), api("/api/accounts"), api("/api/usage")]);
    $("active").textContent = state.activeProfile || "-";
    $("total").textContent = usage.summary.total;
    $("ready").textContent = usage.summary.ready;
    $("blocked").textContent = usage.summary.blocked;
    $("errors").textContent = usage.summary.errors;
    renderUsage(usage.profiles);
    renderAccounts(accounts.accounts);
    $("status").textContent = "Updated " + new Date().toLocaleTimeString();
  }catch(err){$("status").textContent = "Error: " + err.message}
}
function renderUsage(rows){
  const body=$("usageRows");body.replaceChildren();
  rows.forEach(row=>{const tr=document.createElement("tr");const q=row.quota||{};
    const name=td(row.name + (row.active ? " *" : "")); if(row.active) name.className="ok"; tr.append(name);
    tr.append(td(q.account || row.workspaceId || "-"));
    tr.append(td(q.status || "-", statusClass(q.status)));
    tr.append(td(q.main || "-"));
    tr.append(td(q.reset || "-"));
    body.append(tr);
  });
}
function renderAccounts(rows){
  const body=$("accountRows");body.replaceChildren();
  rows.forEach(row=>{const tr=document.createElement("tr");
    const profile=td(row.name + (row.active ? " *" : "")); if(row.active) profile.className="ok"; tr.append(profile);
    tr.append(td(row.providerName));
    const auth=td(""); auth.append(pill(row.auth.label + (row.auth.quotaCompatible ? " quota" : ""))); tr.append(auth);
    tr.append(td(row.codexHome));
    const actions=td(""); actions.className="actions";
    const use=document.createElement("button");use.textContent="Use";use.onclick=async()=>{await api("/api/profile/active",{method:"POST",body:JSON.stringify({profile:row.name})});refresh()};
    const remove=document.createElement("button");remove.textContent="Remove";remove.className="danger";remove.onclick=async()=>{if(confirm("Remove profile entry "+row.name+"? Managed home is not deleted.")){await api("/api/profile/"+encodeURIComponent(row.name),{method:"DELETE"});refresh()}};
    actions.append(use,remove); tr.append(actions); body.append(tr);
  });
}
function command(kind){
  const name=$("profileName").value.trim() || "<name>";
  const cmd = kind==="login" ? `prodex login --profile ${name}` : kind==="add" ? `prodex profile add ${name} --activate` : `prodex profile import-current ${name}`;
  $("command").textContent=cmd;
}
async function createProfile(){
  const name=$("profileName").value.trim();
  if(!name){$("command").textContent="Enter a profile name first.";return}
  const result=await api("/api/profile",{method:"POST",body:JSON.stringify({name,activate:true})});
  $("command").textContent=`Created ${result.profile}. Next: prodex login --profile ${result.profile}`;
  refresh();
}
$("refresh").onclick=refresh;$("login").onclick=()=>command("login");$("add").onclick=()=>command("add");$("import").onclick=()=>command("import");
$("create").onclick=()=>createProfile().catch(err=>$("command").textContent="Create failed: "+err.message);
refresh();setInterval(refresh,10000);
</script>
</body>
</html>"#;
