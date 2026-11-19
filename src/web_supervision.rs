//! Opt-in local action supervision. A capability holder can change modes and
//! approve operations; this is not a sandbox and is not suitable for remote use.
use crate::{cli, http};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::TcpStream;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all="lowercase")]
enum Mode { Always, Risk, Free }
// Only explicitly implemented actions can be prepared. The risk class is set here,
// never supplied by a model or the browser.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag="type", rename_all="snake_case", deny_unknown_fields)]
enum Action { RunningActions {}, StopShell { target: String }, WorkStart {}, WorkStatus {}, WorkEnd {}, SemanticSearch { query: String }, Shell { command: String }, Edit { proposal: String }, EditTransaction { proposals: Vec<String> }, EditTransactionUndo { undo_id: String }, EditUndo { undo_id: String }, WorkspaceInfo {}, McpStart {}, McpCall { session: String, name: String, arguments: Value } }
impl Action {
    fn kind(&self) -> &'static str { match self { Self::RunningActions {} => "running_actions", Self::StopShell { .. } => "stop_shell", Self::WorkStart {} => "work_start", Self::WorkStatus {} => "work_status", Self::WorkEnd {} => "work_end", Self::SemanticSearch { .. } => "semantic_search", Self::Shell { .. } => "shell", Self::Edit { .. } => "edit", Self::EditTransaction { .. } => "edit_transaction", Self::EditTransactionUndo { .. } => "edit_transaction_undo", Self::EditUndo { .. } => "edit_undo", Self::WorkspaceInfo {} => "workspace_info", Self::McpStart {} => "mcp_start", Self::McpCall { .. } => "mcp_call" } }
    fn valid(&self) -> bool { match self { Self::RunningActions {} | Self::WorkStart {} | Self::WorkStatus {} | Self::WorkEnd {} => true, Self::StopShell { target } => hex_id(target), Self::SemanticSearch { query } => !query.trim().is_empty() && query.len()<=4000 && query.chars().count()<=2000 && safe_text(query), Self::Shell { command } => command_valid(command), Self::Edit { proposal } => !proposal.is_empty() && proposal.len()<=7000, Self::EditTransaction { proposals } => !proposals.is_empty() && proposals.len()<=12 && proposals.iter().all(|p| !p.is_empty()) && proposals.iter().map(|p|p.len()).sum::<usize>()<=7000, Self::EditUndo { undo_id } | Self::EditTransactionUndo { undo_id } => hex_id(undo_id), Self::WorkspaceInfo {} | Self::McpStart {} => true, Self::McpCall { session, name, arguments } => hex_id(session) && !name.is_empty() && name.len() <= 128 && safe_text(name) && arguments.is_object() && serde_json::to_vec(arguments).is_ok_and(|v| v.len() <= 4096) && safe_value(arguments) } }
    fn requires_approval(&self, mode: Mode) -> bool {
        match mode { Mode::Always => true, Mode::Risk => !matches!(self, Self::WorkspaceInfo { .. } | Self::WorkStatus { .. } | Self::RunningActions { .. }), Mode::Free => false }
    }
    fn run(self, root: &std::path::Path, _profile: cli::WebExecutionProfile) -> Result<Value, &'static str> {
        match self {
            Self::RunningActions {} | Self::StopShell { .. } | Self::WorkStart {} | Self::WorkStatus {} | Self::WorkEnd {} | Self::SemanticSearch { .. } | Self::Shell { .. } | Self::McpStart {} | Self::McpCall { .. } | Self::Edit { .. } | Self::EditTransaction { .. } | Self::EditTransactionUndo { .. } | Self::EditUndo { .. } => Err("This action requires the session supervisor."),
            Self::WorkspaceInfo {} => {
                let meta = std::fs::metadata(root).map_err(|_| "Workspace information is unavailable.")?;
                if !meta.is_dir() { return Err("Workspace information is unavailable."); }
                Ok(json!({"workspace":root.display().to_string(),"isDirectory":true}))
            }
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
struct PoolScope { run: String, agent: String }
struct PoolRun { chat: String, root: std::path::PathBuf, version: u64, expires: Instant, agents: HashSet<String>, starts: u8, calls: u8, busy: bool }
struct Pending { undo_record_digest: Option<[u8;32]>, work_snapshot: Option<String>, edit_snapshot: Option<String>, transaction_snapshots: Option<Vec<String>>, retry_idempotent: bool, trace: String, profile: cli::WebExecutionProfile, pool: Option<PoolScope>, chat: String, action: Action, root: std::path::PathBuf, version: u64, expires: Instant, requires_approval: bool, server: Option<Vec<String>> }
#[derive(Serialize, Deserialize)]
#[serde(tag="kind", deny_unknown_fields)]
enum DurableUndo {
    Single { chat: String, undo: cli::WebEditUndo },
    Transaction { chat: String, undo: cli::WebTransactionUndo },
}
fn save_undo(root: &std::path::Path, id:&str, record:&DurableUndo) -> cli::Result<()> {
    let bytes=serde_json::to_vec(record)?;cli::save_web_undo(root,id,&bytes)?;Ok(())
}
fn undo_digest<T:Serialize>(record:&T)->Option<[u8;32]> {
    use sha2::{Digest,Sha256};
    Some(Sha256::digest(serde_json::to_vec(record).ok()?).into())
}
fn load_undo(root:&std::path::Path,id:&str)->Option<DurableUndo> {
    serde_json::from_slice(&cli::load_web_undo(root,id).ok()?).ok()
}
struct McpSession { profile: cli::WebExecutionProfile, pool: Option<PoolScope>, chat: String, root: std::path::PathBuf, version: u64, expires: Instant, server: Vec<String>, client: cli::WebMcpSession }
#[derive(Clone, Serialize)]
#[serde(rename_all="camelCase")]
struct AuditEvent { action_ref: String, kind: &'static str, status: &'static str, source: &'static str, unix_seconds: u64 }
#[derive(Default)]
struct State { active: HashMap<String, (String, Arc<AtomicBool>)>, events: HashMap<String, VecDeque<AuditEvent>>, policies: HashMap<String, (Mode, u64)>, pending: HashMap<String, Pending>, sessions: HashMap<String, McpSession>, pool_runs: HashMap<String, PoolRun>, audit_failed: HashSet<std::path::PathBuf>, detail_failed: HashSet<std::path::PathBuf>, next_version: u64 }
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static RUNNING: AtomicUsize = AtomicUsize::new(0);
fn state() -> &'static Mutex<State> {
    static JANITOR: OnceLock<()> = OnceLock::new();
    let state=STATE.get_or_init(|| Mutex::new(State::default()));
    JANITOR.get_or_init(|| { std::thread::spawn(move || loop { std::thread::sleep(Duration::from_secs(5)); if let Ok(mut s)=state.lock() { expire(&mut s,Instant::now()); } }); });
    state
}
const MAX_EVENTS_PER_CHAT: usize=100;
fn audit(s: &mut State, pending: &Pending, status: &'static str) {
    let event=AuditEvent { action_ref:pending.trace.clone(),kind:pending.action.kind(),status,source:if pending.pool.is_some() { "pool" } else { "manual" },unix_seconds:SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) };
    let record=json!({"version":1,"chat":pending.chat.clone(),"event":event.clone()});
    let stored=serde_json::to_vec(&record).ok().is_some_and(|bytes|crate::web_audit_store::append(&pending.root,&bytes).is_ok());
    if !stored { s.audit_failed.insert(pending.root.clone()); }
    let events=s.events.entry(pending.chat.clone()).or_default();
    if events.len()>=MAX_EVENTS_PER_CHAT { events.pop_front(); }
    events.push_back(event);
}
fn encrypted_details_enabled()->bool {std::env::var("ISA_WEB_AUDIT_ENCRYPTED").as_deref()==Ok("1")}
fn audit_warning(s:&State,root:&std::path::Path)->Option<&'static str> { audit_warning_for(s,root,encrypted_details_enabled()) }
fn audit_warning_for(s:&State,root:&std::path::Path,details_enabled:bool)->Option<&'static str> {
    let metadata=s.audit_failed.contains(root);
    let details=details_enabled && s.detail_failed.contains(root);
    match (metadata,details) {(true,true)=>Some("both_incomplete"),(true,false)=>Some("metadata_incomplete"),(false,true)=>Some("encrypted_details_incomplete"),_=>None}
}
// Heuristics reduce accidental credential logging, but arbitrary free text may
// contain secrets that cannot be identified reliably. Do not claim otherwise.
fn redact_details(value:&mut Value) {
    match value {
        Value::Object(map)=>for (key,item) in map.iter_mut() {
            let k=key.to_ascii_lowercase().replace('-',"").replace('_',"");
            if matches!(k.as_str(),"password"|"authorization"|"apikey"|"token"|"accesstoken"|"refreshtoken"|"secret"|"clientsecret"|"privatekey"|"session"|"undoid"|"target"|"idtoken"|"cookie"|"setcookie") { *item=json!("[REDACTED]"); } else {redact_details(item)}
        },
        Value::Array(items)=>for item in items {redact_details(item)},
        Value::String(text)=>if let Ok(token)=std::env::var("ISA_WEB_EXEC_TOKEN") {if token.len()>=32 {*text=text.replace(&token,"[REDACTED]")}},
        _=>(),
    }
}
fn store_detail(s:&mut State,root:&std::path::Path,chat:&str,trace:&str,kind:&str,mut payload:Value) -> bool {
    if !encrypted_details_enabled(){return false}
    redact_details(&mut payload);
    let text=match serde_json::to_string(&payload) {Ok(text)=>text,Err(_)=>{s.detail_failed.insert(root.to_path_buf());return false}};
    let truncated=text.len()>16*1024;let mut end=text.len().min(16*1024);while !text.is_char_boundary(end){end-=1;}
    let record=json!({"version":1,"chat":chat,"actionRef":trace,"kind":kind,"payload":&text[..end],"truncated":truncated});
    // Before extending the experimental Windows log, authenticate its existing
    // records. This is bounded by the storage limit but can be expensive and
    // is not atomic with the subsequent append against external processes.
    if cfg!(windows) && crate::windows_supervision_gate::audit_enabled() && durable_details(root,chat).is_err() {s.detail_failed.insert(root.to_path_buf());return false;}
    let written=serde_json::to_vec(&record).ok().and_then(|bytes|crate::web_audit_crypto::seal(root,&bytes).ok()).is_some_and(|data|crate::web_audit_store::append_sealed(root,&data).is_ok());
    if !written {s.detail_failed.insert(root.to_path_buf());}written
}
fn audit_detail(s:&mut State,pending:&Pending,kind:&str,payload:Value) {
    store_detail(s,&pending.root,&pending.chat,&pending.trace,kind,payload);
}
fn durable_details(root:&std::path::Path,chat:&str)->Result<Vec<Value>,()> {
    crate::web_audit_crypto::check_key(root).map_err(|_|())?;
    let bytes=crate::web_audit_store::read_sealed(root).map_err(|_|())?;let mut details=VecDeque::new();
    for line in bytes.split(|b|*b==b'\n').filter(|line|!line.is_empty()) {
        let plain=crate::web_audit_crypto::open(root,line).map_err(|_|())?;let value:Value=serde_json::from_slice(&plain).map_err(|_|())?;let o=value.as_object().ok_or(())?;
        if o.len()!=6||o.get("version").and_then(Value::as_u64)!=Some(1)||!o.get("chat").and_then(Value::as_str).is_some_and(id_valid)||!o.get("actionRef").and_then(Value::as_str).is_some_and(hex_id)||!matches!(o.get("kind").and_then(Value::as_str),Some("arguments"|"result"|"usage"))||!o.get("payload").and_then(Value::as_str).is_some_and(|p|p.len()<=16*1024)||!o.get("truncated").is_some_and(Value::is_boolean){return Err(())}
        if o.get("chat").and_then(Value::as_str)==Some(chat) {if details.len()>=20 {details.pop_front();}let mut item=o.clone();item.remove("chat");item.remove("version");details.push_back(Value::Object(item));}
    }
    Ok(details.into_iter().collect())
}
fn durable_events(root:&std::path::Path,chat:&str)->Result<Vec<Value>,()> {
    let bytes=crate::web_audit_store::read(root).map_err(|_|())?;let mut events=VecDeque::new();
    for line in bytes.split(|b|*b==b'\n').filter(|line|!line.is_empty()) {
        if line.len()>4096 {return Err(())}
        let value:Value=serde_json::from_slice(line).map_err(|_|())?;
        let record=value.as_object().ok_or(())?;
        if record.len()!=3 || record.get("version").and_then(Value::as_u64)!=Some(1) {return Err(())}
        let name=record.get("chat").and_then(Value::as_str).filter(|id|id_valid(id)).ok_or(())?;
        let event=record.get("event").and_then(Value::as_object).ok_or(())?;
        let valid_text=|key:&str|event.get(key).and_then(Value::as_str).is_some_and(|v|!v.is_empty()&&v.len()<=64&&v.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'));
        if event.len()!=5 || !event.get("actionRef").and_then(Value::as_str).is_some_and(hex_id) || !valid_text("kind") || !valid_text("status") || !matches!(event.get("source").and_then(Value::as_str),Some("manual"|"pool")) || event.get("unixSeconds").and_then(Value::as_u64).is_none() {return Err(())}
        if name==chat {if events.len()>=MAX_EVENTS_PER_CHAT {events.pop_front();}events.push_back(Value::Object(event.clone()));}
    }
    Ok(events.into_iter().collect())
}
fn expire(s: &mut State, now: Instant) {
    s.pool_runs.retain(|_, r| r.expires>now);
    let expired:Vec<String>=s.pending.iter().filter(|(_,p)|p.expires<=now || p.pool.as_ref().is_some_and(|scope| !s.pool_runs.contains_key(&scope.run))).map(|(id,_)|id.clone()).collect();
    for id in expired { if let Some(p)=s.pending.remove(&id) { audit(s,&p,"expired"); } }
    let State { pool_runs, sessions, .. }=s;
    sessions.retain(|_, v| v.expires>now && v.pool.as_ref().map_or(true, |scope| pool_runs.contains_key(&scope.run)));
}
fn hex_id(s: &str) -> bool { s.len()==48 && s.bytes().all(|b| b.is_ascii_hexdigit()) }
fn safe_text(s: &str) -> bool { !s.chars().any(|c| c.is_control() || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) }
fn safe_value(v: &Value) -> bool { match v { Value::String(s)=>safe_text(s), Value::Array(a)=>a.iter().all(safe_value), Value::Object(o)=>o.iter().all(|(k,v)| safe_text(k)&&safe_value(v)), _=>true } }
fn mcp_command() -> Option<Vec<String>> {
    let raw=std::env::var("ISA_WEB_MCP_COMMAND").ok()?; if raw.len()>8192 { return None; }
    let command:Vec<String>=serde_json::from_str(&raw).ok()?;
    if command.is_empty() || command.len()>64 || command[0].is_empty() || command.iter().any(|a| a.len()>4096 || !safe_text(a)) { return None; }
    Some(command)
}
fn id_valid(s: &str) -> bool { !s.is_empty() && s.len() <= 100 && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') }
fn command_valid(s: &str) -> bool { !s.trim().is_empty() && s.len() <= 4096 && !s.chars().any(|c| (c.is_control() && c != '\n' && c != '\t') || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) }
fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    let mut different = a.len() ^ b.len(); for i in 0..a.len().max(b.len()) { different |= (a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0)) as usize; } different == 0
}
pub(crate) fn authorized(request: &http::Request, stream: &TcpStream) -> bool {
    let local = match stream.local_addr() { Ok(a) if a.ip().is_loopback() && a.is_ipv4() => a, _ => return false };
    if !stream.peer_addr().is_ok_and(|a| a.ip().is_loopback()) { return false; }
    if !request.is_same_origin_json() || request.header("host") != Some(local.to_string().as_str()) || request.header("origin") != Some(format!("http://{local}").as_str()) { return false; }
    if request.header("forwarded").is_some() || request.header("x-forwarded-for").is_some() || request.header("x-forwarded-host").is_some() { return false; }
    let expected = match std::env::var("ISA_WEB_EXEC_TOKEN") { Ok(s) if (32..=256).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_graphic()) => s, _ => return false };
    let supplied = request.header("x-isa-exec-token").unwrap_or("");
    supplied.len() <= 256 && constant_eq(expected.as_bytes(), supplied.as_bytes())
}
fn root() -> Option<std::path::PathBuf> {
    let value = std::env::var("ISA_WEB_EXEC_WORKSPACE").ok()?;
    let root = std::path::Path::new(&value).canonicalize().ok()?; root.is_dir().then_some(root)
}
fn random_id() -> Result<String, &'static str> {
    let mut bytes = [0u8; 24]; getrandom::getrandom(&mut bytes).map_err(|_| "Secure random source unavailable")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct Policy { chat: String, mode: Mode }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct EventsRequest { chat: String }
#[derive(Deserialize,Serialize)] #[serde(deny_unknown_fields)]
struct TokenUsage { input:u64, output:u64, total:u64, cached:Option<u64> }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct UsageRequest { chat:String, calls:Vec<TokenUsage>, truncated:bool }
fn usage_valid(request:&UsageRequest)->bool {
    const JS_SAFE:u64=9_007_199_254_740_991;
    id_valid(&request.chat) && !request.calls.is_empty() && request.calls.len()<=32 && request.calls.iter().all(|u|u.input<=JS_SAFE && u.output<=JS_SAFE && u.total<=JS_SAFE && u.cached.map_or(true,|v|v<=JS_SAFE))
}
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct Prepare { chat: String, command: String }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct PrepareAction { chat: String, action: Action, #[serde(default, rename="poolRun")] pool_run: Option<String>, #[serde(default)] agent: Option<String> }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct NewPoolRun { chat: String, agents: Vec<String> }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct ClosePoolRun { chat: String, run: String }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct Cancel { chat: String, id: String, #[serde(default)] denied: bool }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct Execute { chat: String, id: String, approved: bool }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct CloseMcp { chat: String, session: String }
fn err(status: &'static str, message: &'static str) -> (&'static str, Value) { (status, json!({"error":message})) }

const MAX_POOL_STARTS: u8=8;
const MAX_POOL_CALLS: u8=8;
fn transaction_failure(outcome: cli::WebTransactionOutcome, root: &std::path::Path, operation: &str) -> (&'static str, Value) {
    match outcome {
        cli::WebTransactionOutcome::Applied => ("500 Internal Server Error",json!({"error":"Invalid transaction outcome; inspect files."})),
        cli::WebTransactionOutcome::NotStarted => ("409 Conflict",json!({"error":format!("{operation}: no target was replaced; inspect files before retrying."),"status":"not_started"})),
        cli::WebTransactionOutcome::RolledBack => ("409 Conflict",json!({"error":format!("{operation}: failed and replaced files were restored; external observers may have seen changes. Inspect files."),"status":"rolled_back"})),
        cli::WebTransactionOutcome::Partial { unresolved } => {
            let files:Vec<String>=unresolved.iter().filter_map(|p|p.strip_prefix(root).ok()).map(|p|p.to_string_lossy().into_owned()).collect();
            ("500 Internal Server Error",json!({"error":format!("{operation}: rollback incomplete; inspect all files before retrying."),"status":"partial","unresolved":files}))
        }
    }
}
fn reserve_pool_action(run: &mut PoolRun, action: &Action) -> Result<(), (&'static str, &'static str)> {
    if run.busy { return Err(("409 Conflict", "Another pool action is running; action was consumed.")); }
    match action {
        Action::McpStart {} if run.starts<MAX_POOL_STARTS => run.starts+=1,
        Action::McpCall { .. } if run.calls<MAX_POOL_CALLS => run.calls+=1,
        _ => return Err(("429 Too Many Requests", "Pool action budget exhausted or invalid.")),
    }
    run.busy=true; Ok(())
}
// Retry only admission failures proven to occur before dispatch. This never
// retries a child process, MCP request, timeout, transport error, or tool error.
fn retry_admission<F>(attempts: usize, delay: Duration, retry_status: &'static str, mut acquire: F) -> Result<(), (&'static str, &'static str)>
where F: FnMut() -> Result<(), (&'static str, &'static str)> {
    for n in 0..attempts {
        match acquire() {
            Ok(()) => return Ok(()),
            Err(e) if e.0==retry_status && n+1<attempts => std::thread::sleep(delay),
            Err(e) => return Err(e),
        }
    }
    Err(("503 Service Unavailable", "Execution capacity unavailable."))
}
// Pool actions use a shared server-side serial gate and consume budget before
// execution. It is intentionally independent of provider retry logic.
struct PoolGuard(String);
impl Drop for PoolGuard { fn drop(&mut self) { if let Ok(mut s)=state().lock() { if let Some(run)=s.pool_runs.get_mut(&self.0) { run.busy=false; } } } }

// Removing a running registration does not imply that escaped descendants died.
struct ActiveGuard(String);
impl Drop for ActiveGuard { fn drop(&mut self) { if let Ok(mut s)=state().lock() { s.active.remove(&self.0); } } }

// Consume before validation and dispatch, so approval IDs cannot be replayed,
// including after denial, expiration, cross-chat access or a wrong endpoint.
fn consume(s: &mut State, chat: &str, id: &str, root: &std::path::Path, approved: bool, legacy: bool, now: Instant) -> Result<Pending, (&'static str, &'static str)> {
    let p = s.pending.remove(id).ok_or(("409 Conflict", "Action expired or already consumed."))?;
    if p.chat != chat || p.root.as_path() != root || p.expires <= now || s.policies.get(chat).map(|v| v.1) != Some(p.version) || (p.requires_approval && !approved) {
        let status=if p.expires<=now { "expired" } else if p.chat==chat && p.requires_approval && !approved { "denied" } else { "rejected" };
        audit(s,&p,status); return Err(("403 Forbidden", "Action denied, expired, or policy changed."));
    }
    if legacy && !matches!(&p.action, Action::Shell { .. }) { audit(s,&p,"rejected"); return Err(("403 Forbidden", "Action type is not supported by this endpoint.")); }
    audit(s,&p,if p.requires_approval { "approved" } else { "authorized_without_prompt" });
    Ok(p)
}

// Only a platform hint: configuration, authentication and individual action
// checks remain necessary, and Unix does not imply sandbox support.
pub(crate) fn platform_supported() -> bool { cfg!(unix) }

pub fn handle(path: &str, request: &http::Request, stream: &TcpStream) -> (&'static str, Value) {
    let windows_enabled=crate::windows_supervision_gate::enabled();
    let windows_shell=crate::windows_supervision_gate::shell_enabled();
    let windows_audit=crate::windows_supervision_gate::audit_enabled();
    if (!platform_supported() && !(windows_enabled && (crate::windows_supervision_gate::endpoint_allowed(path,windows_shell) || crate::windows_supervision_gate::audit_endpoint_allowed(path,windows_audit)))) || !authorized(request, stream) { return err("403 Forbidden", "Local execution is disabled or unauthorized."); }
    let Some(root) = root() else { return err("503 Service Unavailable", "Execution workspace is not configured."); };
    let Ok(profile)=cli::WebExecutionProfile::configured() else { return err("503 Service Unavailable", "Invalid or unsupported execution profile; local actions are disabled."); };
    if request.body.len() > 8192 { return err("400 Bad Request", "Request too large."); }
    match path {
        "/api/actions/sessions/list" => {
            let Ok(body)=serde_json::from_slice::<Value>(&request.body) else { return err("400 Bad Request", "Invalid request."); };
            if !body.as_object().is_some_and(|o|o.is_empty()) { return err("400 Bad Request", "Expected an empty request."); }
            match cli::web_list_sessions(&root) { Ok(mut v)=>{v["workspace"]=json!(root.display().to_string());("200 OK",v)},Err(_)=>err("422 Unprocessable Entity", "CLI sessions are unavailable or the session directory is unsafe.") }
        }
        "/api/actions/sessions/read" => {
            #[derive(Deserialize)] #[serde(deny_unknown_fields)] struct ReadSession { id:String, workspace:String }
            let Ok(body)=serde_json::from_slice::<ReadSession>(&request.body) else { return err("400 Bad Request", "Invalid session request."); };
            if body.workspace!=root.display().to_string() { return err("409 Conflict", "Execution workspace changed; list sessions again."); }
            match cli::web_read_session(&root,&body.id) { Ok(mut v)=>{v["workspace"]=json!(root.display().to_string());("200 OK",v)},Err(_)=>err("422 Unprocessable Entity", "Session unavailable, unsafe, invalid or exceeds import limits.") }
        }
        "/api/actions/workspaces" => {
            let Ok(body)=serde_json::from_slice::<Value>(&request.body) else { return err("400 Bad Request", "Invalid request."); };
            if !body.as_object().is_some_and(|object|object.is_empty()) { return err("400 Bad Request", "Expected an empty request."); }
            let agent=crate::agent_workspace();
            let comparison=agent.as_ref().map(|a|a==&root);
            ("200 OK",json!({"executionWorkspace":root.display().to_string(),"agentWorkspace":agent.as_ref().map(|p|p.display().to_string()),"sameWorkspace":comparison,"executionProfile":profile.name()}))
        }
        "/api/exec/policy" => {
            let Ok(body) = serde_json::from_slice::<Policy>(&request.body) else { return err("400 Bad Request", "Invalid policy."); };
            if !id_valid(&body.chat) { return err("400 Bad Request", "Invalid chat ID."); }
            let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            if !s.policies.contains_key(&body.chat) && s.policies.len() >= 1000 { return err("503 Service Unavailable", "Policy limit reached."); }
            if s.policies.get(&body.chat).map(|p| p.0) != Some(body.mode) {
                let invalidated:Vec<String>=s.pending.iter().filter(|(_,p)|p.chat==body.chat).map(|(id,_)|id.clone()).collect();
                for id in invalidated { if let Some(p)=s.pending.remove(&id) { audit(&mut s,&p,"policy_changed"); } }
                s.next_version = s.next_version.saturating_add(1); let version = s.next_version;
                s.policies.insert(body.chat.clone(), (body.mode,version)); s.pending.retain(|_, p| p.chat != body.chat); s.sessions.retain(|_, v| v.chat != body.chat); s.pool_runs.retain(|_, r| r.chat != body.chat);
            }
            ("200 OK", json!({"mode":body_mode(body.mode),"workspace":root.display().to_string(),"executionProfile":profile.name()}))
        }
        "/api/actions/usage" => {
            let Ok(body)=serde_json::from_slice::<UsageRequest>(&request.body) else {return err("400 Bad Request", "Invalid usage record.");};
            if !usage_valid(&body) {return err("400 Bad Request", "Invalid usage record.");}
            if !encrypted_details_enabled() {return ("200 OK",json!({"encryptedDetails":"disabled"}));}
            let Ok(trace)=random_id() else {return err("503 Service Unavailable", "Secure random source unavailable.");};
            let Ok(mut s)=state().lock() else {return err("503 Service Unavailable", "Supervisor unavailable.");};
            let now=SystemTime::now().duration_since(UNIX_EPOCH).map(|v|v.as_secs()).unwrap_or(0);
            let written=store_detail(&mut s,&root,&body.chat,&trace,"usage",json!({"unixSeconds":now,"calls":body.calls,"truncated":body.truncated}));
            ("200 OK",json!({"encryptedDetails":if written&&!s.detail_failed.contains(&root) {"best_effort"} else {"degraded"},"encryptedDetailsExperimental":windows_audit}))
        }
        "/api/actions/events" => {
            let Ok(body)=serde_json::from_slice::<EventsRequest>(&request.body) else { return err("400 Bad Request", "Invalid events request."); };
            if !id_valid(&body.chat) { return err("400 Bad Request", "Invalid chat ID."); }
            let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            let durable=durable_events(&root,&body.chat);
            // Notice a removed/replaced log while this process has a later event.
            // This cannot detect tampering across restarts or prove completeness.
            let matches_memory=durable.as_ref().ok().is_some_and(|events| s.events.get(&body.chat).and_then(|v|v.back()).map_or(true,|last|events.last()==serde_json::to_value(last).ok().as_ref()));
            if !matches_memory {s.audit_failed.insert(root.clone());}
            let (encrypted,details)=if !encrypted_details_enabled() {("disabled",Vec::new())} else {
                match durable_details(&root,&body.chat) {
                    Ok(details) if !s.detail_failed.contains(&root) => ("best_effort",details),
                    _ => {s.detail_failed.insert(root.clone());("degraded",Vec::new())}
                }
            };
            match durable {
                Ok(events) if !s.audit_failed.contains(&root) => ("200 OK",json!({"events":events,"volatile":false,"persistence":"best_effort","encryptedDetails":encrypted,"details":details,"limit":MAX_EVENTS_PER_CHAT})),
                _ => { let events:Vec<_>=s.events.get(&body.chat).map(|v|v.iter().cloned().collect()).unwrap_or_default(); ("200 OK",json!({"events":events,"volatile":true,"persistence":"degraded","encryptedDetails":encrypted,"encryptedDetailsExperimental":windows_audit,"details":details,"limit":MAX_EVENTS_PER_CHAT})) }
            }
        }
        "/api/actions/pool/start" => {
            let Ok(body)=serde_json::from_slice::<NewPoolRun>(&request.body) else { return err("400 Bad Request", "Invalid pool run."); };
            if !id_valid(&body.chat) || body.agents.is_empty() || body.agents.len()>50 || body.agents.iter().any(|a| !id_valid(a)) { return err("400 Bad Request", "Invalid pool agents."); }
            let agents:HashSet<_>=body.agents.into_iter().collect();
            let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            let Some(&(_,version))=s.policies.get(&body.chat) else { return err("409 Conflict", "Chat policy is not registered."); };
            expire(&mut s,Instant::now());
            if s.pool_runs.len()>=32 { return err("503 Service Unavailable", "Pool run limit reached."); }
            let Ok(id)=random_id() else { return err("503 Service Unavailable", "Secure random source unavailable."); };
            s.pool_runs.insert(id.clone(),PoolRun { chat:body.chat,root:root.clone(),version,expires:Instant::now()+Duration::from_secs(300),agents,starts:0,calls:0,busy:false });
            ("200 OK",json!({"run":id,"maxStarts":MAX_POOL_STARTS,"maxCalls":MAX_POOL_CALLS,"expiresInSeconds":300}))
        }
        "/api/actions/pool/close" => {
            let Ok(body)=serde_json::from_slice::<ClosePoolRun>(&request.body) else { return err("400 Bad Request", "Invalid pool run."); };
            if !id_valid(&body.chat) || !hex_id(&body.run) { return err("400 Bad Request", "Invalid pool run."); }
            let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            if s.pool_runs.get(&body.run).is_some_and(|r|r.chat==body.chat) {
                s.pool_runs.remove(&body.run);
                let invalidated:Vec<String>=s.pending.iter().filter(|(_,p)|p.pool.as_ref().map(|x|x.run.as_str())==Some(body.run.as_str())).map(|(id,_)|id.clone()).collect();
                for id in invalidated { if let Some(p)=s.pending.remove(&id) { audit(&mut s,&p,"cancelled"); } }
                s.sessions.retain(|_,v|v.pool.as_ref().map(|p|p.run.as_str())!=Some(body.run.as_str()));
            }
            ("200 OK",json!({"closed":true}))
        }
        "/api/exec/prepare" | "/api/actions/prepare" => {
            let (chat, action, pool, legacy) = if path == "/api/exec/prepare" {
                let Ok(body) = serde_json::from_slice::<Prepare>(&request.body) else { return err("400 Bad Request", "Invalid action."); };
                (body.chat, Action::Shell { command: body.command }, None, true)
            } else {
                let Ok(body) = serde_json::from_slice::<PrepareAction>(&request.body) else { return err("400 Bad Request", "Invalid or unsupported action."); };
                let pool=match (body.pool_run,body.agent) { (None,None)=>None, (Some(run),Some(agent)) if hex_id(&run) && id_valid(&agent)=>Some(PoolScope {run,agent}), _=>return err("400 Bad Request", "Invalid pool action attribution.") };
                (body.chat, body.action, pool, false)
            };
            if !id_valid(&chat) || !action.valid() { return err("400 Bad Request", "Invalid chat ID or action."); }
            if windows_enabled && (pool.is_some() || !(matches!(&action,Action::WorkspaceInfo {}) || (windows_shell && ((legacy && matches!(&action,Action::Shell { .. })) || matches!(&action,Action::RunningActions {} | Action::StopShell { .. }))))) { return err("403 Forbidden", "Unsupported action in experimental Windows supervision."); }
            let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            expire(&mut s,Instant::now());
            let Some(&(mode, version)) = s.policies.get(&chat) else { return err("409 Conflict", "Chat policy is not registered."); };
            if s.pending.len() >= 64 { return err("503 Service Unavailable", "Pending approval limit reached."); }
            let Ok(id) = random_id() else { return err("503 Service Unavailable", "Secure random source unavailable."); };
            let Ok(trace) = random_id() else { return err("503 Service Unavailable", "Secure random source unavailable."); };
            if let Some(scope)=&pool {
                if !matches!(&action,Action::McpStart {} | Action::McpCall { .. }) { return err("400 Bad Request", "Unsupported pool action."); }
                let Some(run)=s.pool_runs.get(&scope.run) else { return err("409 Conflict", "Pool run unavailable or expired."); };
                if run.chat!=chat || run.root!=root || run.version!=version || run.expires<=Instant::now() || !run.agents.contains(&scope.agent) { return err("403 Forbidden", "Pool run or agent is invalid."); }
            }
            let mut retry_idempotent=false;
            let server=match &action {
                Action::McpStart {} => { let Some(command)=mcp_command() else { return err("503 Service Unavailable", "MCP server is not configured."); }; Some(command) },
                Action::McpCall { session, name, arguments } => {
                    let Some(v)=s.sessions.get(session) else { return err("409 Conflict", "MCP session is unavailable or expired."); };
                    if v.profile!=profile || v.pool!=pool || v.chat!=chat || v.root!=root || v.version!=version || v.expires<=Instant::now() || mcp_command().as_ref()!=Some(&v.server) || v.client.tools().iter().filter(|t| t.get("name").and_then(Value::as_str)==Some(name.as_str())).count()!=1 { return err("403 Forbidden", "MCP session or tool is invalid."); }
                    if v.client.validate_call(name,arguments).is_err() { return err("422 Unprocessable Entity", "Tool arguments or input schema are invalid or unsupported; no tool was called."); }
                    retry_idempotent=v.client.tools().iter().find(|t|t.get("name").and_then(Value::as_str)==Some(name.as_str())).and_then(|t|t.get("annotations")).and_then(|a|a.get("idempotentHint")).and_then(Value::as_bool)==Some(true);
                    Some(v.server.clone())
                }, _=>None,
            };
            let mut transaction_snapshots=None;let mut work_snapshot=None;let mut undo_record_digest=None;
            let (edit_review,edit_snapshot)=match &action {
                Action::Edit { proposal } => match cli::web_prepare_apply(&root,proposal) { Ok((review,snapshot)) => (Some(review),Some(snapshot)), Err(_) => return err("422 Unprocessable Entity", "Edit is invalid, unavailable, protected, stale, or not an exact unique match.") },
                Action::EditTransaction { proposals } => match cli::web_prepare_transaction(&root,proposals) {
                    Ok((review,snapshots)) => { transaction_snapshots=Some(snapshots); (Some(review),None) },
                    Err(_) => return err("422 Unprocessable Entity", "Transaction contains invalid, duplicate, linked, protected, stale, or unavailable targets; no files were written.")
                },
                Action::EditTransactionUndo { undo_id } => {
                    let Some(DurableUndo::Transaction {chat:owner,undo})=load_undo(&root,undo_id) else { return err("409 Conflict", "Transaction undo unavailable."); };
                    if owner!=chat { return err("403 Forbidden", "Transaction undo is not available for this chat."); }
                    undo_record_digest=undo_digest(&undo);if undo_record_digest.is_none() {return err("409 Conflict", "Transaction undo unavailable.");}
                    match cli::web_prepare_transaction_undo(&root,&undo) { Ok((review,snapshots))=>{transaction_snapshots=Some(snapshots);(Some(review),None)}, Err(_)=>return err("409 Conflict", "Transaction target changed or undo is unavailable; nothing was written.") }
                },
                Action::EditUndo { undo_id } => {
                    let Some(DurableUndo::Single {chat:owner,undo})=load_undo(&root,undo_id) else { return err("409 Conflict", "Undo record unavailable."); };
                    if owner!=chat { return err("403 Forbidden", "Undo record is not available for this chat."); }
                    undo_record_digest=undo_digest(&undo);if undo_record_digest.is_none() {return err("409 Conflict", "Undo record unavailable.");}
                    match cli::web_prepare_undo(&root,&undo) { Ok((review,snapshot))=>(Some(review),Some(snapshot)), Err(_)=>return err("409 Conflict", "File changed or undo is unavailable; nothing was written.") }
                },
                _=>(None,None)
            };
            if matches!(&action,Action::WorkEnd {}) { work_snapshot=Some(match cli::web_work_prepare_end(&root) {Ok(snapshot)=>snapshot,Err(_)=>return err("409 Conflict", "Work baseline is unavailable or unsafe; nothing was removed.")}); }
            let requires_approval = action.requires_approval(mode);
            let mut response = json!({"id":id,"workspace":root.display().to_string(),"mode":body_mode(mode),"requiresApproval":requires_approval,"expiresInSeconds":120,"executionProfile":profile.name()});
            if windows_shell && matches!(&action,Action::Shell { .. }) { response["shellDialect"]=json!("powershell"); }
            if let Some(review)=edit_review { response["review"]=review; }
            if legacy { if let Action::Shell { command } = &action { response["command"] = json!(command); } }
            else { response["action"] = json!(&action); }
            if let Some(command)=&server { response["serverCommand"]=json!(command); }
            if matches!(&action,Action::McpCall { .. }) { response["maxAttempts"]=json!(if retry_idempotent { 2 } else { 1 }); }
            if let Some(scope)=&pool { response["poolRun"]=json!(scope.run); response["agent"]=json!(scope.agent); }
            let pending=Pending { undo_record_digest, work_snapshot, edit_snapshot, transaction_snapshots, retry_idempotent, trace, profile, pool, chat, action, root:root.clone(),version,expires:Instant::now()+Duration::from_secs(120),requires_approval,server };
            audit(&mut s,&pending,"proposed"); audit_detail(&mut s,&pending,"arguments",json!(&pending.action));
            if let Some(warning)=audit_warning(&s,&root) {response["auditWarning"]=json!(warning);}
            s.pending.insert(id, pending);
            ("200 OK", response)
        }
        "/api/exec/cancel" | "/api/actions/cancel" => {
            let Ok(body) = serde_json::from_slice::<Cancel>(&request.body) else { return err("400 Bad Request", "Invalid action."); };
            if !id_valid(&body.chat) || body.id.len()!=48 || !body.id.bytes().all(|b| b.is_ascii_hexdigit()) { return err("400 Bad Request", "Invalid action."); }
            let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            if s.pending.get(&body.id).is_some_and(|p| p.chat == body.chat) { if let Some(p)=s.pending.remove(&body.id) { audit(&mut s,&p,if body.denied { "denied" } else { "cancelled" }); } }
            ("200 OK", json!({"cancelled":true}))
        }
        "/api/actions/stop" => {
            let Ok(body)=serde_json::from_slice::<Cancel>(&request.body) else { return err("400 Bad Request", "Invalid stop request."); };
            if !id_valid(&body.chat) || !hex_id(&body.id) { return err("400 Bad Request", "Invalid stop request."); }
            let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            let mut requested=false;
            if s.pending.get(&body.id).is_some_and(|p|p.chat==body.chat && matches!(&p.action,Action::Shell { .. })) {
                if let Some(p)=s.pending.remove(&body.id) { audit(&mut s,&p,"cancelled");requested=true; }
            }
            if let Some((chat,flag))=s.active.get(&body.id) { if chat==&body.chat { flag.store(true,Ordering::SeqCst);requested=true; } }
            ("200 OK",json!({"requested":requested}))
        }
        "/api/actions/mcp/close" => {
            let Ok(body)=serde_json::from_slice::<CloseMcp>(&request.body) else { return err("400 Bad Request", "Invalid session."); };
            if !id_valid(&body.chat) || !hex_id(&body.session) { return err("400 Bad Request", "Invalid session."); }
            let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            if s.sessions.get(&body.session).is_some_and(|v| v.chat==body.chat) { s.sessions.remove(&body.session); }
            ("200 OK",json!({"closed":true}))
        }
        "/api/exec/run" | "/api/actions/run" => {
            let Ok(body) = serde_json::from_slice::<Execute>(&request.body) else { return err("400 Bad Request", "Invalid approval."); };
            if !id_valid(&body.chat) || body.id.len()!=48 || !body.id.bytes().all(|b| b.is_ascii_hexdigit()) { return err("400 Bad Request", "Invalid approval."); }
            let (pending,cancellation) = {
                let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
                let p=match consume(&mut s, &body.chat, &body.id, &root, body.approved, path == "/api/exec/run", Instant::now()) {
                    Ok(p) => p,
                    Err((status, message)) => return err(status, message),
                };
                let cancellation=if matches!(&p.action,Action::Shell { .. }) {
                    let flag=Arc::new(AtomicBool::new(false));s.active.insert(body.id.clone(),(body.chat.clone(),flag.clone()));Some(flag)
                } else { None };
                (p,cancellation)
            };
            let _active_guard=cancellation.as_ref().map(|_|ActiveGuard(body.id.clone()));
            if pending.profile!=profile { if let Ok(mut s)=state().lock() { audit(&mut s,&pending,"not_started"); } return err("403 Forbidden", "Execution profile changed; action was consumed."); }
            // Repeat the allowlist after consuming approval; never dispatch a
            // stale or injected unsupported action on Windows.
            if windows_enabled && (pending.pool.is_some() || !(matches!(&pending.action,Action::WorkspaceInfo {}) || (windows_shell && ((path=="/api/exec/run" && matches!(&pending.action,Action::Shell { .. })) || matches!(&pending.action,Action::RunningActions {} | Action::StopShell { .. }))))) {
                if let Ok(mut s)=state().lock() { audit(&mut s,&pending,"not_started"); }
                return err("403 Forbidden", "Unsupported action was consumed without running.");
            }
            let mcp=matches!(&pending.action,Action::McpStart {} | Action::McpCall { .. });
            let attempts=if mcp { 3 } else { 1 };
            // In-memory status and cooperative stop must remain available when
            // execution slots are occupied; neither spawns or reads output.
            let needs_slot=!matches!(&pending.action,Action::RunningActions {} | Action::StopShell { .. });
            if needs_slot {
                let admission=retry_admission(attempts,Duration::from_millis(150),"503 Service Unavailable",|| {
                    RUNNING.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < 2).then_some(n+1))
                        .map(|_|()).map_err(|_|("503 Service Unavailable", "Execution capacity reached; action was consumed."))
                });
                if let Err((status,message))=admission { if let Ok(mut s)=state().lock() { audit(&mut s,&pending,"not_started"); } return err(status,message); }
            }
            struct Guard(bool); impl Drop for Guard { fn drop(&mut self) { if self.0 { RUNNING.fetch_sub(1, Ordering::SeqCst); } } } let _guard=Guard(needs_slot);
            let _pool_guard=if let Some(scope)=&pending.pool {
                let admission=retry_admission(attempts,Duration::from_millis(150),"409 Conflict",|| {
                    let mut s=state().lock().map_err(|_|("503 Service Unavailable", "Supervisor unavailable."))?;
                    let Some(run)=s.pool_runs.get_mut(&scope.run) else { return Err(("403 Forbidden", "Pool run unavailable or expired.")); };
                    if run.chat!=body.chat || run.root!=root || run.version!=pending.version || run.expires<=Instant::now() || !run.agents.contains(&scope.agent) { return Err(("403 Forbidden", "Pool run or agent is invalid.")); }
                    reserve_pool_action(run,&pending.action)
                });
                if let Err((status,message))=admission { if let Ok(mut s)=state().lock() { audit(&mut s,&pending,"not_started"); } return err(status,message); }
                Some(PoolGuard(scope.run.clone()))
            } else { None };
            // Waiting must not resurrect an action invalidated while admission was busy.
            if mcp {
                let current=state().lock().ok().is_some_and(|s| s.policies.get(&pending.chat).map(|v|v.1)==Some(pending.version));
                if !current || pending.expires<=Instant::now() || cli::WebExecutionProfile::configured()!=Ok(profile) || self::root()!=Some(root.clone()) {
                    if let Ok(mut s)=state().lock() { audit(&mut s,&pending,"not_started"); }
                    return err("403 Forbidden", "Policy, workspace, profile or approval changed while waiting; action was not started.");
                }
            }
            let mut dispatched=false;
            let mut mark_dispatched=|| { dispatched=true; if let Ok(mut s)=state().lock() { audit(&mut s,&pending,"dispatched"); } };
            let kind=pending.action.kind();
            let outcome=(|| match pending.action.clone() {
                Action::McpStart {} => {
                    let Some(server)=pending.server.clone() else { return err("403 Forbidden", "MCP server configuration missing."); };
                    if mcp_command().as_ref()!=Some(&server) { return err("403 Forbidden", "MCP server configuration changed."); }
                    { let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); }; s.sessions.retain(|_,v|v.expires>Instant::now()); if s.sessions.len()>=4 { return err("503 Service Unavailable", "MCP session limit reached."); } }
                    mark_dispatched();
                    let client=match cli::WebMcpSession::start(&root,&server,profile) { Ok(client)=>client, Err(_)=>return err("500 Internal Server Error", "MCP start or discovery failed; server actions may have occurred.") };
                    let Ok(session)=random_id() else { return err("503 Service Unavailable", "Secure random source unavailable; server actions may have occurred."); };
                    let tools=client.tools().to_vec();
                    let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable; server actions may have occurred."); };
                    if cli::WebExecutionProfile::configured()!=Ok(profile) || pending.pool.as_ref().is_some_and(|p| !s.pool_runs.contains_key(&p.run)) || s.policies.get(&body.chat).map(|v|v.1)!=Some(pending.version) || mcp_command().as_ref()!=Some(&server) || self::root()!=Some(root.clone()) { return err("403 Forbidden", "Policy or configuration changed; server actions may have occurred."); }
                    s.sessions.retain(|_,v|v.expires>Instant::now()); if s.sessions.len()>=4 { return err("503 Service Unavailable", "MCP session limit reached; server actions may have occurred."); }
                    s.sessions.insert(session.clone(),McpSession { profile, pool:pending.pool.clone(), chat:body.chat.clone(),root:root.clone(),version:pending.version,expires:Instant::now()+Duration::from_secs(300),server,client });
                    ("200 OK",json!({"result":{"session":session,"tools":tools,"expiresInSeconds":300}}))
                }
                Action::McpCall { session,name,arguments } => {
                    let mut session = {
                        let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
                        let Some(v)=s.sessions.remove(&session) else { return err("409 Conflict", "MCP session unavailable or already used."); };
                        if v.profile!=profile || v.pool!=pending.pool || v.chat!=body.chat || v.root!=root || v.version!=pending.version || s.policies.get(&body.chat).map(|v|v.1)!=Some(v.version) || v.expires<=Instant::now() || pending.server.as_ref()!=Some(&v.server) || mcp_command().as_ref()!=Some(&v.server) { return err("403 Forbidden", "MCP session expired or policy changed."); } v
                    };
                    mark_dispatched();
                    let first=match session.client.call(&name,arguments.clone()) { Ok(result)=>result, Err(_)=>return err("500 Internal Server Error", "MCP call failed; its outcome may be unknown. It was not retried.") };
                    if pending.retry_idempotent && first.get("isError").and_then(Value::as_bool)==Some(true) {
                        // Only complete, well-formed tool errors are retried. The tool's
                        // untrusted idempotentHint is not proof that effects are safe.
                        std::thread::sleep(Duration::from_millis(150));
                        let allowed=if let Ok(mut s)=state().lock() {
                            let policy_ok=s.policies.get(&pending.chat).map(|v|v.1)==Some(pending.version) && pending.expires>Instant::now() && session.expires>Instant::now();
                            let pool_ok=if let Some(scope)=&pending.pool {
                                if let Some(run)=s.pool_runs.get_mut(&scope.run) {
                                    if policy_ok && run.busy && run.chat==pending.chat && run.root==root && run.version==pending.version && run.expires>Instant::now() && run.calls<MAX_POOL_CALLS && run.agents.contains(&scope.agent) { run.calls+=1; true } else { false }
                                } else { false }
                            } else { true };
                            policy_ok && pool_ok
                        } else { false };
                        if allowed && cli::WebExecutionProfile::configured()==Ok(profile) && self::root()==Some(root.clone()) && mcp_command().as_ref()==pending.server.as_ref() {
                            if let Ok(mut s)=state().lock() { audit(&mut s,&pending,"retrying"); }
                            return match session.client.call(&name,arguments) { Ok(result)=>("200 OK",json!({"result":result,"attempts":2})), Err(_)=>err("500 Internal Server Error", "MCP retry failed; its outcome may be unknown. No further retry was made.") };
                        }
                    }
                    ("200 OK",json!({"result":first,"attempts":1}))
                }
                Action::RunningActions {} => {
                    mark_dispatched();
                    match state().lock() {
                        Ok(s) => { let mut ids:Vec<&str>=s.active.iter().filter(|(_, (chat,_))| chat==&pending.chat).map(|(id,_)|id.as_str()).collect(); ids.sort_unstable(); ("200 OK",json!({"result":{"runningShellActions":ids.len(),"actionIds":ids}})) },
                        Err(_) => err("503 Service Unavailable", "Supervisor unavailable."),
                    }
                }
                Action::StopShell { target } => {
                    mark_dispatched();
                    match state().lock() {
                        Ok(s) => { let requested=if let Some((chat,flag))=s.active.get(&target) { if chat==&pending.chat { flag.store(true,Ordering::SeqCst);true } else { false } } else { false }; ("200 OK",json!({"result":{"requested":requested}})) },
                        Err(_) => err("503 Service Unavailable", "Supervisor unavailable."),
                    }
                }
                Action::WorkStart {} => {
                    mark_dispatched();match cli::web_work_start(&root) { Ok(result)=>("200 OK",json!({"result":result})), Err(_)=>err("409 Conflict", "Could not start work tracking; an existing baseline is never overwritten. Inspect the workspace.") }
                }
                Action::WorkStatus {} => {
                    mark_dispatched();match cli::web_work_status(&root) { Ok(result)=>("200 OK",json!({"result":result})), Err(_)=>err("409 Conflict", "Work status unavailable. A baseline may be missing, invalid or exceed limits.") }
                }
                Action::WorkEnd {} => {
                    let Some(snapshot)=pending.work_snapshot.as_deref() else { return err("403 Forbidden", "Work baseline snapshot missing."); };
                    mark_dispatched();match cli::web_work_end(&root,snapshot) { Ok(result)=>("200 OK",json!({"result":result})), Err(_)=>err("409 Conflict", "Baseline changed, could not be removed or outcome is unknown; inspect workspace before retrying.") }
                }
                Action::SemanticSearch { query } => {
                    mark_dispatched();
                    match cli::web_semantic_search(&root,&query) {
                        Ok(result)=>("200 OK",json!({"result":result,"workspace":root.display().to_string()})),
                        Err(_)=>err("422 Unprocessable Entity", "Semantic search failed. Verify the local index and its Ollama embedding model; the query may have reached Ollama.")
                    }
                }
                Action::Shell { command } => {
                    let Some(flag)=cancellation.as_deref() else { return err("503 Service Unavailable", "Command cancellation unavailable."); };
                    if flag.load(Ordering::SeqCst) { return err("409 Conflict", "Command stopped before starting."); }
                    mark_dispatched();
                    match cli::web_run_command(&root,&command,profile,flag) { Ok(result)=>("200 OK",json!({"result":result})), Err(_)=>err("500 Internal Server Error", "Command could not be run; its outcome may be unknown.") }
                }
                Action::Edit { proposal } => {
                    let Some(snapshot)=pending.edit_snapshot.as_deref() else { return err("403 Forbidden", "Edit snapshot missing."); };
                    let Ok(undo_id)=random_id() else { return err("503 Service Unavailable", "Secure random source unavailable; edit not started."); };
                    mark_dispatched();
                    match cli::web_apply_prepared(&root,&proposal,snapshot,|undo|save_undo(&root,&undo_id,&DurableUndo::Single {chat:body.chat.clone(),undo:undo.clone()})) {
                        Ok(mut result) => { result["undoId"]=json!(undo_id); ("200 OK",json!({"result":result})) },
                        Err(_)=>err("409 Conflict", "Edit was not applied or its outcome is unknown. Check the file and undo storage before retrying.")
                    }
                }
                Action::EditTransaction { proposals } => {
                    let Some(snapshots)=pending.transaction_snapshots.as_deref() else { return err("403 Forbidden", "Transaction snapshots missing."); };
                    let Ok(undo_id)=random_id() else { return err("503 Service Unavailable", "Secure random source unavailable; transaction not started."); };
                    mark_dispatched();
                    let outcome=cli::web_apply_transaction(&root,&proposals,snapshots,|undo|save_undo(&root,&undo_id,&DurableUndo::Transaction {chat:body.chat.clone(),undo:undo.clone()}));
                    if outcome==cli::WebTransactionOutcome::Applied { ("200 OK",json!({"result":{"status":"applied","files":proposals.len(),"atomic":false,"undoAvailable":true,"undoId":undo_id}})) }
                    else { transaction_failure(outcome,&root,"Transaction") }
                }
                Action::EditTransactionUndo { undo_id } => {
                    let Some(DurableUndo::Transaction {chat:owner,undo})=load_undo(&root,&undo_id) else { return err("409 Conflict", "Transaction undo unavailable."); };
                    if owner!=body.chat { return err("403 Forbidden", "Transaction undo is not available for this chat."); }
                    let Some(snapshots)=pending.transaction_snapshots.as_deref() else {return err("403 Forbidden", "Undo approval snapshot missing.");};
                    if undo_digest(&undo)!=pending.undo_record_digest || pending.undo_record_digest.is_none() {return err("409 Conflict", "Undo record changed since review.");}
                    mark_dispatched();
                    let (outcome,already)=cli::web_apply_transaction_undo(&root,&undo,snapshots);
                    if outcome==cli::WebTransactionOutcome::Applied { ("200 OK",json!({"result":{"status":"undone","alreadyUndone":already,"atomic":false}})) }
                    else { transaction_failure(outcome,&root,"Transaction undo") }
                }
                Action::EditUndo { undo_id } => {
                    let Some(DurableUndo::Single {chat:owner,undo})=load_undo(&root,&undo_id) else { return err("409 Conflict", "Undo record unavailable."); };
                    if owner!=body.chat { return err("403 Forbidden", "Undo record is not available for this chat."); }
                    let Some(snapshot)=pending.edit_snapshot.as_deref() else {return err("403 Forbidden", "Undo approval snapshot missing.");};
                    if undo_digest(&undo)!=pending.undo_record_digest || pending.undo_record_digest.is_none() {return err("409 Conflict", "Undo record changed since review.");}
                    mark_dispatched();
                    match cli::web_apply_undo(&root,&undo,snapshot) { Ok(result)=>("200 OK",json!({"result":result})), Err(_)=>err("409 Conflict", "Undo was not applied or its outcome is unknown; inspect the file.") }
                }
                action => { mark_dispatched(); match action.run(&root,profile) { Ok(result) => ("200 OK", json!({"result":result})), Err(message) => err("500 Internal Server Error", message) } },
            })();
            let status=if !dispatched { "not_started" } else if outcome.0!="200 OK" { "unknown" } else if kind=="shell" {
                let r=&outcome.1["result"];
                if r["timedOut"]==true || r["cancelled"]==true || r["pipesIncomplete"]==true || r["exitCode"].is_null() { "unknown" }
                else if r["exitCode"]==0 { "completed" } else { "failed" }
            } else if kind=="mcp_call" && outcome.1["result"]["isError"]==true { "failed" } else { "completed" };
            let mut outcome=outcome;
            let warning=if let Ok(mut s)=state().lock() { audit(&mut s,&pending,status); audit_detail(&mut s,&pending,"result",outcome.1.clone()); audit_warning(&s,&root) } else {Some("both_incomplete")};
            if let Some(warning)=warning {if let Some(body)=outcome.1.as_object_mut() {body.insert("auditWarning".into(),json!(warning));}}
            outcome
        }
        _ => err("404 Not Found", "Not found.")
    }
}
fn body_mode(mode: Mode) -> &'static str { match mode { Mode::Always => "always", Mode::Risk => "risk", Mode::Free => "free" } }

#[cfg(test)]
mod tests {
    use super::*;
    fn pending(expires: Instant) -> Pending {
        Pending { undo_record_digest:None, work_snapshot:None, edit_snapshot:None, transaction_snapshots:None, retry_idempotent:false, trace:"test-reference".into(), profile:cli::WebExecutionProfile::Host, pool:None, chat:"a".into(), action:Action::WorkspaceInfo {}, root:"/workspace".into(), version:7, expires, requires_approval:true, server:None }
    }
    #[test]
    fn expired_and_changed_actions_are_consumed_without_running() {
        let now=Instant::now(); let mut s=State::default(); s.policies.insert("a".into(),(Mode::Always,7));
        s.pending.insert("expired".into(), pending(now));
        assert!(consume(&mut s,"a","expired",std::path::Path::new("/workspace"),true,false,now).is_err());
        assert!(!s.pending.contains_key("expired"));
        s.pending.insert("root".into(), pending(now+Duration::from_secs(1)));
        assert!(consume(&mut s,"a","root",std::path::Path::new("/other"),true,false,now).is_err());
        s.pending.insert("version".into(), pending(now+Duration::from_secs(1))); s.policies.insert("a".into(),(Mode::Always,8));
        assert!(consume(&mut s,"a","version",std::path::Path::new("/workspace"),true,false,now).is_err());
    }
    #[test]
    fn concurrent_consumers_cannot_reuse_an_approval() {
        let now=Instant::now(); let mut s=State::default(); s.policies.insert("a".into(),(Mode::Always,7)); s.pending.insert("one".into(),pending(now+Duration::from_secs(60)));
        let state=std::sync::Arc::new(Mutex::new(s)); let barrier=std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads:Vec<_>=(0..2).map(|_| { let state=state.clone();let barrier=barrier.clone();std::thread::spawn(move || { barrier.wait();let mut s=state.lock().unwrap();consume(&mut s,"a","one",std::path::Path::new("/workspace"),true,false,now).is_ok() }) }).collect();
        assert_eq!(threads.into_iter().map(|t| t.join().unwrap()).filter(|ok| *ok).count(),1);
    }
    #[test]
    fn audit_warning_distinguishes_failed_metadata_and_details() {
        let root=std::path::PathBuf::from("workspace");let other=std::path::Path::new("other");let mut s=State::default();
        assert_eq!(audit_warning_for(&s,&root,true),None);s.detail_failed.insert(root.clone());
        assert_eq!(audit_warning_for(&s,&root,false),None);assert_eq!(audit_warning_for(&s,&root,true),Some("encrypted_details_incomplete"));
        s.audit_failed.insert(root.clone());assert_eq!(audit_warning_for(&s,&root,true),Some("both_incomplete"));
        assert_eq!(audit_warning_for(&s,&root,false),Some("metadata_incomplete"));assert_eq!(audit_warning_for(&s,other,true),None);
    }
    #[test]
    fn encrypted_detail_redacts_structured_credentials() {
        let mut value=json!({"arguments":{"password":"secret-a","API_key":"secret-b","token":"secret-c","nested":[{"Authorization":"secret-d"}]},"message":"ordinary data"});
        redact_details(&mut value);let text=value.to_string();for secret in ["secret-a","secret-b","secret-c","secret-d"] {assert!(!text.contains(secret));}assert!(text.contains("ordinary data"));
    }
    #[test]
    fn pool_gate_serializes_and_counts_attempts_across_agents() {
        let mut run=PoolRun { chat:"a".into(),root:"/workspace".into(),version:7,expires:Instant::now()+Duration::from_secs(60),agents:["one".into(),"two".into()].into_iter().collect(),starts:0,calls:0,busy:false };
        let start=Action::McpStart {}; let call=Action::McpCall { session:"a".repeat(48),name:"echo".into(),arguments:json!({}) };
        assert!(reserve_pool_action(&mut run,&start).is_ok());
        assert_eq!(reserve_pool_action(&mut run,&call).unwrap_err().0,"409 Conflict"); assert_eq!(run.calls,0);
        run.busy=false; for _ in 0..MAX_POOL_CALLS { assert!(reserve_pool_action(&mut run,&call).is_ok()); run.busy=false; }
        assert_eq!(reserve_pool_action(&mut run,&call).unwrap_err().0,"429 Too Many Requests"); assert_eq!(run.calls,MAX_POOL_CALLS);
    }
    #[test]
    fn admission_retries_only_known_busy_failures() {
        let mut attempts=0;
        let ok=retry_admission(3,Duration::ZERO,"503 Service Unavailable",|| { attempts+=1; if attempts<3 { Err(("503 Service Unavailable","busy")) } else { Ok(()) } });
        assert!(ok.is_ok());assert_eq!(attempts,3);
        let mut attempts=0;
        let failed=retry_admission(3,Duration::ZERO,"503 Service Unavailable",|| { attempts+=1; Err(("403 Forbidden","invalid")) });
        assert_eq!(failed.unwrap_err().0,"403 Forbidden");assert_eq!(attempts,1);
        let mut attempts=0;
        let failed=retry_admission(3,Duration::ZERO,"409 Conflict",|| { attempts+=1; Err(("409 Conflict","busy")) });
        assert_eq!(failed.unwrap_err().0,"409 Conflict");assert_eq!(attempts,3);
    }
    #[test]
    fn pool_busy_admission_does_not_consume_budget_until_acquired() {
        let mut run=PoolRun { chat:"a".into(),root:"/workspace".into(),version:7,expires:Instant::now()+Duration::from_secs(60),agents:HashSet::new(),starts:0,calls:0,busy:true };
        let mut attempts=0;
        let result=retry_admission(3,Duration::ZERO,"409 Conflict",|| { attempts+=1; if attempts==3 { run.busy=false; } reserve_pool_action(&mut run,&Action::McpStart {}) });
        assert!(result.is_ok());assert_eq!(attempts,3);assert_eq!(run.starts,1);assert!(run.busy);
    }
    #[test]
    fn event_ring_is_bounded_and_expiration_is_recorded() {
        let now=Instant::now();let mut s=State::default();s.policies.insert("a".into(),(Mode::Always,7));
        let p=pending(now);for _ in 0..105 { audit(&mut s,&p,"proposed"); }
        assert_eq!(s.events["a"].len(),100);
        s.pending.insert("id".into(),p);expire(&mut s,now);
        assert_eq!(s.events["a"].len(),100);assert_eq!(s.events["a"].back().unwrap().status,"expired");
    }
    #[test]
    fn action_schema_and_risk_are_backend_defined() {
        let shell: Action = serde_json::from_value(json!({"type":"shell","command":"echo hello"})).unwrap();
        let info: Action = serde_json::from_value(json!({"type":"workspace_info"})).unwrap();
        assert!(shell.valid() && info.valid());
        assert!(shell.requires_approval(Mode::Always) && shell.requires_approval(Mode::Risk));
        assert!(!shell.requires_approval(Mode::Free));
        assert!(info.requires_approval(Mode::Always));
        assert!(!info.requires_approval(Mode::Risk) && !info.requires_approval(Mode::Free));
        assert!(serde_json::from_value::<Action>(json!({"type":"workspace_info","risk":"low"})).is_err());
        assert!(serde_json::from_value::<Action>(json!({"type":"mcp_call"})).is_err());
    }
}
