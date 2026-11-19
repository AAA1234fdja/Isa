//! Opt-in local action supervision. A capability holder can change modes and
//! approve operations; this is not a sandbox and is not suitable for remote use.
use crate::{cli, http};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::net::TcpStream;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all="lowercase")]
enum Mode { Always, Risk, Free }
// Only explicitly implemented actions can be prepared. The risk class is set here,
// never supplied by a model or the browser.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag="type", rename_all="snake_case", deny_unknown_fields)]
enum Action { Shell { command: String }, WorkspaceInfo {}, McpStart {}, McpCall { session: String, name: String, arguments: Value } }
impl Action {
    fn valid(&self) -> bool { match self { Self::Shell { command } => command_valid(command), Self::WorkspaceInfo {} | Self::McpStart {} => true, Self::McpCall { session, name, arguments } => hex_id(session) && !name.is_empty() && name.len() <= 128 && safe_text(name) && arguments.is_object() && serde_json::to_vec(arguments).is_ok_and(|v| v.len() <= 4096) && safe_value(arguments) } }
    fn requires_approval(&self, mode: Mode) -> bool {
        match mode { Mode::Always => true, Mode::Risk => !matches!(self, Self::WorkspaceInfo { .. }), Mode::Free => false }
    }
    fn run(self, root: &std::path::Path) -> Result<Value, &'static str> {
        match self {
            Self::Shell { command } => cli::web_run_command(root, &command).map_err(|_| "Command could not be run; its outcome may be unknown."),
            Self::McpStart {} | Self::McpCall { .. } => Err("MCP actions require the session supervisor."),
            Self::WorkspaceInfo {} => {
                let meta = std::fs::metadata(root).map_err(|_| "Workspace information is unavailable.")?;
                if !meta.is_dir() { return Err("Workspace information is unavailable."); }
                Ok(json!({"workspace":root.display().to_string(),"isDirectory":true}))
            }
        }
    }
}
#[derive(Clone)]
struct PoolScope { run: String, agent: String }
struct PoolRun { chat: String, root: std::path::PathBuf, version: u64, expires: Instant, agents: HashSet<String>, starts: u8, calls: u8, busy: bool }
struct Pending { pool: Option<PoolScope>, chat: String, action: Action, root: std::path::PathBuf, version: u64, expires: Instant, requires_approval: bool, server: Option<Vec<String>> }
struct McpSession { pool_run: Option<String>, chat: String, root: std::path::PathBuf, version: u64, expires: Instant, server: Vec<String>, client: cli::WebMcpSession }
#[derive(Default)]
struct State { policies: HashMap<String, (Mode, u64)>, pending: HashMap<String, Pending>, sessions: HashMap<String, McpSession>, pool_runs: HashMap<String, PoolRun>, next_version: u64 }
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static RUNNING: AtomicUsize = AtomicUsize::new(0);
fn state() -> &'static Mutex<State> {
    static JANITOR: OnceLock<()> = OnceLock::new();
    let state=STATE.get_or_init(|| Mutex::new(State::default()));
    JANITOR.get_or_init(|| { std::thread::spawn(move || loop { std::thread::sleep(Duration::from_secs(5)); if let Ok(mut s)=state.lock() { expire(&mut s,Instant::now()); } }); });
    state
}
fn expire(s: &mut State, now: Instant) {
    s.pool_runs.retain(|_, r| r.expires>now);
    let State { pool_runs, pending, sessions, .. }=s;
    pending.retain(|_, p| p.expires>now && p.pool.as_ref().map_or(true, |scope| pool_runs.contains_key(&scope.run)));
    sessions.retain(|_, v| v.expires>now && v.pool_run.as_ref().map_or(true, |id| pool_runs.contains_key(id)));
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
fn authorized(request: &http::Request, stream: &TcpStream) -> bool {
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
    let mut bytes = [0u8; 24]; std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut bytes)).map_err(|_| "Secure random source unavailable")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct Policy { chat: String, mode: Mode }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct Prepare { chat: String, command: String }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct PrepareAction { chat: String, action: Action, #[serde(default, rename="poolRun")] pool_run: Option<String>, #[serde(default)] agent: Option<String> }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct NewPoolRun { chat: String, agents: Vec<String> }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct ClosePoolRun { chat: String, run: String }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct Cancel { chat: String, id: String }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct Execute { chat: String, id: String, approved: bool }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct CloseMcp { chat: String, session: String }
fn err(status: &'static str, message: &'static str) -> (&'static str, Value) { (status, json!({"error":message})) }

fn reserve_pool_action(run: &mut PoolRun, action: &Action) -> Result<(), (&'static str, &'static str)> {
    if run.busy { return Err(("409 Conflict", "Another pool action is running; action was consumed.")); }
    match action {
        Action::McpStart {} if run.starts<3 => run.starts+=1,
        Action::McpCall { .. } if run.calls<3 => run.calls+=1,
        _ => return Err(("429 Too Many Requests", "Pool action budget exhausted or invalid.")),
    }
    run.busy=true; Ok(())
}
// Pool actions use a shared server-side serial gate and consume budget before
// execution. It is intentionally independent of provider retry logic.
struct PoolGuard(String);
impl Drop for PoolGuard { fn drop(&mut self) { if let Ok(mut s)=state().lock() { if let Some(run)=s.pool_runs.get_mut(&self.0) { run.busy=false; } } } }

// Consume before validation and dispatch, so approval IDs cannot be replayed,
// including after denial, expiration, cross-chat access or a wrong endpoint.
fn consume(s: &mut State, chat: &str, id: &str, root: &std::path::Path, approved: bool, legacy: bool, now: Instant) -> Result<Pending, (&'static str, &'static str)> {
    let p = s.pending.remove(id).ok_or(("409 Conflict", "Action expired or already consumed."))?;
    if p.chat != chat || p.root.as_path() != root || p.expires <= now || s.policies.get(chat).map(|v| v.1) != Some(p.version) || (p.requires_approval && !approved) {
        return Err(("403 Forbidden", "Action denied, expired, or policy changed."));
    }
    if legacy && !matches!(&p.action, Action::Shell { .. }) { return Err(("403 Forbidden", "Action type is not supported by this endpoint.")); }
    Ok(p)
}

pub fn handle(path: &str, request: &http::Request, stream: &TcpStream) -> (&'static str, Value) {
    if !cfg!(unix) || !authorized(request, stream) { return err("403 Forbidden", "Local execution is disabled or unauthorized."); }
    let Some(root) = root() else { return err("503 Service Unavailable", "Execution workspace is not configured."); };
    if request.body.len() > 8192 { return err("400 Bad Request", "Request too large."); }
    match path {
        "/api/exec/policy" => {
            let Ok(body) = serde_json::from_slice::<Policy>(&request.body) else { return err("400 Bad Request", "Invalid policy."); };
            if !id_valid(&body.chat) { return err("400 Bad Request", "Invalid chat ID."); }
            let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            if !s.policies.contains_key(&body.chat) && s.policies.len() >= 1000 { return err("503 Service Unavailable", "Policy limit reached."); }
            if s.policies.get(&body.chat).map(|p| p.0) != Some(body.mode) {
                s.next_version = s.next_version.saturating_add(1); let version = s.next_version;
                s.policies.insert(body.chat.clone(), (body.mode,version)); s.pending.retain(|_, p| p.chat != body.chat); s.sessions.retain(|_, v| v.chat != body.chat); s.pool_runs.retain(|_, r| r.chat != body.chat);
            }
            ("200 OK", json!({"mode":body_mode(body.mode),"workspace":root.display().to_string()}))
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
            ("200 OK",json!({"run":id,"maxStarts":3,"maxCalls":3,"expiresInSeconds":300}))
        }
        "/api/actions/pool/close" => {
            let Ok(body)=serde_json::from_slice::<ClosePoolRun>(&request.body) else { return err("400 Bad Request", "Invalid pool run."); };
            if !id_valid(&body.chat) || !hex_id(&body.run) { return err("400 Bad Request", "Invalid pool run."); }
            let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            if s.pool_runs.get(&body.run).is_some_and(|r|r.chat==body.chat) {
                s.pool_runs.remove(&body.run);
                s.pending.retain(|_,p|p.pool.as_ref().map(|x|x.run.as_str())!=Some(body.run.as_str()));
                s.sessions.retain(|_,v|v.pool_run.as_deref()!=Some(body.run.as_str()));
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
            let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            s.pending.retain(|_, p| p.expires > Instant::now());
            let Some(&(mode, version)) = s.policies.get(&chat) else { return err("409 Conflict", "Chat policy is not registered."); };
            if s.pending.len() >= 64 { return err("503 Service Unavailable", "Pending approval limit reached."); }
            let Ok(id) = random_id() else { return err("503 Service Unavailable", "Secure random source unavailable."); };
            if let Some(scope)=&pool {
                if !matches!(&action,Action::McpStart {} | Action::McpCall { .. }) { return err("400 Bad Request", "Unsupported pool action."); }
                let Some(run)=s.pool_runs.get(&scope.run) else { return err("409 Conflict", "Pool run unavailable or expired."); };
                if run.chat!=chat || run.root!=root || run.version!=version || run.expires<=Instant::now() || !run.agents.contains(&scope.agent) { return err("403 Forbidden", "Pool run or agent is invalid."); }
            }
            let server=match &action {
                Action::McpStart {} => { let Some(command)=mcp_command() else { return err("503 Service Unavailable", "MCP server is not configured."); }; Some(command) },
                Action::McpCall { session, name, .. } => {
                    let Some(v)=s.sessions.get(session) else { return err("409 Conflict", "MCP session is unavailable or expired."); };
                    if v.pool_run.as_deref()!=pool.as_ref().map(|p|p.run.as_str()) || v.chat!=chat || v.root!=root || v.version!=version || v.expires<=Instant::now() || mcp_command().as_ref()!=Some(&v.server) || v.client.tools().iter().filter(|t| t.get("name").and_then(Value::as_str)==Some(name.as_str())).count()!=1 { return err("403 Forbidden", "MCP session or tool is invalid."); }
                    Some(v.server.clone())
                }, _=>None,
            };
            let requires_approval = action.requires_approval(mode);
            let mut response = json!({"id":id,"workspace":root.display().to_string(),"mode":body_mode(mode),"requiresApproval":requires_approval,"expiresInSeconds":120});
            if legacy { if let Action::Shell { command } = &action { response["command"] = json!(command); } }
            else { response["action"] = json!(&action); }
            if let Some(command)=&server { response["serverCommand"]=json!(command); }
            if let Some(scope)=&pool { response["poolRun"]=json!(scope.run); response["agent"]=json!(scope.agent); }
            s.pending.insert(id, Pending { pool, chat, action, root:root.clone(),version,expires:Instant::now()+Duration::from_secs(120),requires_approval,server });
            ("200 OK", response)
        }
        "/api/exec/cancel" | "/api/actions/cancel" => {
            let Ok(body) = serde_json::from_slice::<Cancel>(&request.body) else { return err("400 Bad Request", "Invalid action."); };
            if !id_valid(&body.chat) || body.id.len()!=48 || !body.id.bytes().all(|b| b.is_ascii_hexdigit()) { return err("400 Bad Request", "Invalid action."); }
            let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            if s.pending.get(&body.id).is_some_and(|p| p.chat == body.chat) { s.pending.remove(&body.id); }
            ("200 OK", json!({"cancelled":true}))
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
            let pending = {
                let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
                match consume(&mut s, &body.chat, &body.id, &root, body.approved, path == "/api/exec/run", Instant::now()) {
                    Ok(p) => p,
                    Err((status, message)) => return err(status, message),
                }
            };
            if RUNNING.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < 2).then_some(n+1)).is_err() { return err("503 Service Unavailable", "Execution capacity reached; action was consumed."); }
            struct Guard; impl Drop for Guard { fn drop(&mut self) { RUNNING.fetch_sub(1, Ordering::SeqCst); } } let _guard=Guard;
            let _pool_guard=if let Some(scope)=&pending.pool {
                let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
                let Some(run)=s.pool_runs.get_mut(&scope.run) else { return err("409 Conflict", "Pool run unavailable or expired."); };
                if run.chat!=body.chat || run.root!=root || run.version!=pending.version || run.expires<=Instant::now() || !run.agents.contains(&scope.agent) { return err("403 Forbidden", "Pool run or agent is invalid."); }
                if let Err((status,message))=reserve_pool_action(run,&pending.action) { return err(status,message); }
                Some(PoolGuard(scope.run.clone()))
            } else { None };
            match pending.action {
                Action::McpStart {} => {
                    let Some(server)=pending.server else { return err("403 Forbidden", "MCP server configuration missing."); };
                    if mcp_command().as_ref()!=Some(&server) { return err("403 Forbidden", "MCP server configuration changed."); }
                    { let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); }; s.sessions.retain(|_,v|v.expires>Instant::now()); if s.sessions.len()>=4 { return err("503 Service Unavailable", "MCP session limit reached."); } }
                    let client=match cli::WebMcpSession::start(&root,&server) { Ok(client)=>client, Err(_)=>return err("500 Internal Server Error", "MCP start or discovery failed; server actions may have occurred.") };
                    let Ok(session)=random_id() else { return err("503 Service Unavailable", "Secure random source unavailable; server actions may have occurred."); };
                    let tools=client.tools().to_vec();
                    let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable; server actions may have occurred."); };
                    if pending.pool.as_ref().is_some_and(|p| !s.pool_runs.contains_key(&p.run)) || s.policies.get(&body.chat).map(|v|v.1)!=Some(pending.version) || mcp_command().as_ref()!=Some(&server) || self::root()!=Some(root.clone()) { return err("403 Forbidden", "Policy or configuration changed; server actions may have occurred."); }
                    s.sessions.retain(|_,v|v.expires>Instant::now()); if s.sessions.len()>=4 { return err("503 Service Unavailable", "MCP session limit reached; server actions may have occurred."); }
                    s.sessions.insert(session.clone(),McpSession { pool_run:pending.pool.as_ref().map(|p|p.run.clone()), chat:body.chat,root,version:pending.version,expires:Instant::now()+Duration::from_secs(300),server,client });
                    ("200 OK",json!({"result":{"session":session,"tools":tools,"expiresInSeconds":300}}))
                }
                Action::McpCall { session,name,arguments } => {
                    let mut session = {
                        let Ok(mut s)=state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
                        let Some(v)=s.sessions.remove(&session) else { return err("409 Conflict", "MCP session unavailable or already used."); };
                        if v.pool_run.as_deref()!=pending.pool.as_ref().map(|p|p.run.as_str()) || v.chat!=body.chat || v.root!=root || v.version!=pending.version || s.policies.get(&body.chat).map(|v|v.1)!=Some(v.version) || v.expires<=Instant::now() || pending.server.as_ref()!=Some(&v.server) || mcp_command().as_ref()!=Some(&v.server) { return err("403 Forbidden", "MCP session expired or policy changed."); } v
                    };
                    match session.client.call(&name,arguments) { Ok(result)=>("200 OK",json!({"result":result})), Err(_)=>err("500 Internal Server Error", "MCP call failed; its outcome may be unknown. Do not blindly retry.") }
                }
                action => match action.run(&root) { Ok(result) => ("200 OK", json!({"result":result})), Err(message) => err("500 Internal Server Error", message) },
            }
        }
        _ => err("404 Not Found", "Not found.")
    }
}
fn body_mode(mode: Mode) -> &'static str { match mode { Mode::Always => "always", Mode::Risk => "risk", Mode::Free => "free" } }

#[cfg(test)]
mod tests {
    use super::*;
    fn pending(expires: Instant) -> Pending {
        Pending { pool:None, chat:"a".into(), action:Action::WorkspaceInfo {}, root:"/workspace".into(), version:7, expires, requires_approval:true, server:None }
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
    fn pool_gate_serializes_and_counts_attempts_across_agents() {
        let mut run=PoolRun { chat:"a".into(),root:"/workspace".into(),version:7,expires:Instant::now()+Duration::from_secs(60),agents:["one".into(),"two".into()].into_iter().collect(),starts:0,calls:0,busy:false };
        let start=Action::McpStart {}; let call=Action::McpCall { session:"a".repeat(48),name:"echo".into(),arguments:json!({}) };
        assert!(reserve_pool_action(&mut run,&start).is_ok());
        assert_eq!(reserve_pool_action(&mut run,&call).unwrap_err().0,"409 Conflict"); assert_eq!(run.calls,0);
        run.busy=false; for _ in 0..3 { assert!(reserve_pool_action(&mut run,&call).is_ok()); run.busy=false; }
        assert_eq!(reserve_pool_action(&mut run,&call).unwrap_err().0,"429 Too Many Requests"); assert_eq!(run.calls,3);
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
