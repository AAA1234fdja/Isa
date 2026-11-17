//! Opt-in local command supervision. A capability holder can change modes and
//! approve operations; this is not a sandbox and is not suitable for remote use.
use crate::{cli, http};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use std::net::TcpStream;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all="lowercase")]
enum Mode { Always, Risk, Free }
struct Pending { chat: String, command: String, root: std::path::PathBuf, version: u64, expires: Instant, requires_approval: bool }
#[derive(Default)]
struct State { policies: HashMap<String, (Mode, u64)>, pending: HashMap<String, Pending>, next_version: u64 }
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static RUNNING: AtomicUsize = AtomicUsize::new(0);
fn state() -> &'static Mutex<State> { STATE.get_or_init(|| Mutex::new(State::default())) }
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
struct Cancel { chat: String, id: String }
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct Execute { chat: String, id: String, approved: bool }
fn err(status: &'static str, message: &'static str) -> (&'static str, Value) { (status, json!({"error":message})) }

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
                s.policies.insert(body.chat.clone(), (body.mode,version)); s.pending.retain(|_, p| p.chat != body.chat);
            }
            ("200 OK", json!({"mode":body_mode(body.mode),"workspace":root.display().to_string()}))
        }
        "/api/exec/prepare" => {
            let Ok(body) = serde_json::from_slice::<Prepare>(&request.body) else { return err("400 Bad Request", "Invalid action."); };
            if !id_valid(&body.chat) || !command_valid(&body.command) { return err("400 Bad Request", "Invalid chat ID or command."); }
            let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            s.pending.retain(|_, p| p.expires > Instant::now());
            let Some(&(mode, version)) = s.policies.get(&body.chat) else { return err("409 Conflict", "Chat policy is not registered."); };
            if s.pending.len() >= 64 { return err("503 Service Unavailable", "Pending approval limit reached."); }
            let Ok(id) = random_id() else { return err("503 Service Unavailable", "Secure random source unavailable."); };
            let requires_approval = mode != Mode::Free; // Arbitrary shell is always high risk.
            s.pending.insert(id.clone(), Pending { chat:body.chat, command:body.command.clone(),root:root.clone(),version,expires:Instant::now()+Duration::from_secs(120),requires_approval });
            ("200 OK", json!({"id":id,"command":body.command,"workspace":root.display().to_string(),"mode":body_mode(mode),"requiresApproval":requires_approval,"expiresInSeconds":120}))
        }
        "/api/exec/cancel" => {
            let Ok(body) = serde_json::from_slice::<Cancel>(&request.body) else { return err("400 Bad Request", "Invalid action."); };
            if !id_valid(&body.chat) || body.id.len()!=48 || !body.id.bytes().all(|b| b.is_ascii_hexdigit()) { return err("400 Bad Request", "Invalid action."); }
            let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
            if s.pending.get(&body.id).is_some_and(|p| p.chat == body.chat) { s.pending.remove(&body.id); }
            ("200 OK", json!({"cancelled":true}))
        }
        "/api/exec/run" => {
            let Ok(body) = serde_json::from_slice::<Execute>(&request.body) else { return err("400 Bad Request", "Invalid approval."); };
            if !id_valid(&body.chat) || body.id.len()!=48 || !body.id.bytes().all(|b| b.is_ascii_hexdigit()) { return err("400 Bad Request", "Invalid approval."); }
            let pending = {
                let Ok(mut s) = state().lock() else { return err("503 Service Unavailable", "Supervisor unavailable."); };
                // Consume before running: duplicate or racing requests cannot execute twice.
                let Some(p) = s.pending.remove(&body.id) else { return err("409 Conflict", "Action expired or already consumed."); };
                if p.chat != body.chat || p.root != root || p.expires <= Instant::now() || s.policies.get(&body.chat).map(|v| v.1) != Some(p.version) || (p.requires_approval && !body.approved) { return err("403 Forbidden", "Action denied, expired, or policy changed."); }
                p
            };
            if RUNNING.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < 2).then_some(n+1)).is_err() { return err("503 Service Unavailable", "Execution capacity reached; action was consumed."); }
            struct Guard; impl Drop for Guard { fn drop(&mut self) { RUNNING.fetch_sub(1, Ordering::SeqCst); } } let _guard=Guard;
            match cli::web_run_command(&root, &pending.command) { Ok(result) => ("200 OK", json!({"result":result})), Err(_) => err("500 Internal Server Error", "Command could not be run; its outcome may be unknown.") }
        }
        _ => err("404 Not Found", "Not found.")
    }
}
fn body_mode(mode: Mode) -> &'static str { match mode { Mode::Always => "always", Mode::Risk => "risk", Mode::Free => "free" } }
