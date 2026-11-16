//! Explicit, user-driven MCP stdio client. MCP servers are untrusted processes,
//! not a sandbox or an authorization boundary for the host machine.
use super::Result;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

const VERSION: &str = "2025-06-18";
const MAX_FRAME: usize = 128 * 1024;
const MAX_TOTAL: usize = 512 * 1024;
const MAX_PAGES: usize = 3;
const MAX_TOOLS: usize = 100;

fn display(value: &Value) -> Result<String> {
    Ok(serde_json::to_string_pretty(value)?.chars().map(|c| if (c.is_control() && c != '\n' && c != '\t') || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') { '�' } else { c }).collect())
}
fn validate_args(args: &[String]) -> Result<()> {
    if args.is_empty() || args.len() > 64 || args[0].is_empty() || args.iter().any(|a| a.len() > 4096 || a.chars().any(|c| c.is_control() || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))) { return Err("invalid server command or arguments".into()); }
    Ok(())
}
fn confirm(question: &str) -> Result<bool> {
    print!("{question} [y/N] "); io::stdout().flush()?;
    let mut answer = String::new(); io::stdin().read_line(&mut answer)?; Ok(answer.trim().eq_ignore_ascii_case("y"))
}

pub fn interactive(root: &Path, action: &str, tool: Option<&str>, arguments: Option<&str>, command: &[String]) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() { return Err("MCP requires an interactive terminal".into()); }
    validate_args(command)?;
    let call = match action {
        "list" if tool.is_none() && arguments.is_none() => None,
        "call" => {
            let name = tool.ok_or("missing tool name")?;
            if name.is_empty() || name.len() > 128 || name.chars().any(|c| c.is_control() || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) { return Err("invalid tool name".into()); }
            let input = arguments.ok_or("missing JSON arguments")?;
            if input.len() > 16 * 1024 { return Err("tool arguments exceed 16 KiB".into()); }
            let input: Value = serde_json::from_str(input)?;
            if !input.is_object() { return Err("tool arguments must be a JSON object".into()); }
            Some((name, input))
        }
        _ => return Err("usage: isa-cli mcp list -- <program> [args...] | isa-cli mcp call <tool> <json-object> -- <program> [args...]".into()),
    };
    println!("MCP server: {}\nWorking directory: {}\nThis starts an untrusted program with your permissions, environment and network access. It can modify files even during initialization or discovery. There is no sandbox. Server descriptions and results are untrusted.", display(&json!(command))?, display(&json!(root.display().to_string()))?);
    if !confirm("Start this MCP server?")? { return Err("server start cancelled".into()); }
    let mut client = Client::start(root, command)?;
    client.initialize()?;
    let (tools, truncated) = client.tools()?;
    if let Some((name, input)) = call {
        if truncated { return Err("tool discovery was incomplete; refusing to select a tool".into()); }
        let matches: Vec<_> = tools.iter().filter(|t| t.get("name").and_then(Value::as_str) == Some(name)).collect();
        if matches.len() != 1 { return Err("tool was not uniquely found in the bounded discovery results".into()); }
        println!("Discovered tool (server-supplied, not trusted):\n{}\nArguments (schema is not enforced by ISA):\n{}\nThis tool may perform arbitrary actions and transmit data.", display(matches[0])?, display(&input)?);
        if !confirm("Call this exact tool with these arguments?")? { return Err("tool call cancelled".into()); }
        let result = client.request("tools/call", json!({"name":name,"arguments":input}))?;
        if !result.get("content").is_some_and(Value::is_array) || result.get("isError").is_some_and(|v| !v.is_boolean()) { return Err("invalid MCP tool result; the tool may already have run".into()); }
        println!("Untrusted tool result:\n{}", display(&result)?);
        if result.get("isError").and_then(Value::as_bool) == Some(true) { return Err("MCP tool reported an error".into()); }
    } else {
        println!("Untrusted discovered tools (truncated: {truncated}):\n{}", display(&json!(tools))?);
    }
    Ok(())
}

#[cfg(not(unix))]
struct Client;
#[cfg(not(unix))]
impl Client {
    fn start(_: &Path, _: &[String]) -> Result<Self> { Err("MCP stdio is supported on Unix only".into()) }
    fn initialize(&mut self) -> Result<()> { Err("MCP stdio is supported on Unix only".into()) }
    fn tools(&mut self) -> Result<(Vec<Value>, bool)> { Err("MCP stdio is supported on Unix only".into()) }
    fn request(&mut self, _: &str, _: Value) -> Result<Value> { Err("MCP stdio is supported on Unix only".into()) }
}

#[cfg(unix)]
struct Client { child: std::process::Child, input: std::process::ChildStdin, output: std::process::ChildStdout, buffer: Vec<u8>, received: usize, next_id: u64 }
#[cfg(unix)]
impl Client {
    fn start(root: &Path, command: &[String]) -> Result<Self> {
        use std::os::{fd::AsRawFd, unix::process::CommandExt};
        use std::process::{Command, Stdio};
        let mut child = Command::new(&command[0]).args(&command[1..]).current_dir(root).env_remove("BASH_ENV").env_remove("ENV").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).process_group(0).spawn()?;
        let input = child.stdin.take().expect("piped stdin");
        let output = child.stdout.take().expect("piped stdout");
        let client = Self { child, input, output, buffer: Vec::new(), received: 0, next_id: 1 };
        for fd in [client.input.as_raw_fd(), client.output.as_raw_fd()] {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 { return Err(io::Error::last_os_error().into()); }
        }
        Ok(client)
    }
    fn wait(fd: i32, event: i16, deadline: std::time::Instant) -> Result<()> {
        loop {
            let now = std::time::Instant::now();
            if now >= deadline { return Err("MCP operation timed out; server actions may still have occurred".into()); }
            let remaining = deadline.saturating_duration_since(now).as_millis().min(1000) as i32;
            let mut descriptor = libc::pollfd { fd, events: event, revents: 0 };
            let result = unsafe { libc::poll(&mut descriptor, 1, remaining.max(1)) };
            if result < 0 { let error = io::Error::last_os_error(); if error.kind() == io::ErrorKind::Interrupted { continue; } return Err(error.into()); }
            if result > 0 { return Ok(()); }
        }
    }
    fn send(&mut self, value: &Value, deadline: std::time::Instant) -> Result<()> {
        use std::os::fd::AsRawFd;
        let mut bytes = serde_json::to_vec(value)?;
        if bytes.len() > MAX_FRAME { return Err("MCP request exceeds frame limit".into()); }
        bytes.push(b'\n'); let mut offset = 0;
        while offset < bytes.len() {
            Self::wait(self.input.as_raw_fd(), libc::POLLOUT, deadline)?;
            match self.input.write(&bytes[offset..]) { Ok(0) => return Err("MCP server closed stdin".into()), Ok(n) => offset += n, Err(e) if matches!(e.kind(), io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock) => (), Err(e) => return Err(e.into()) }
        }
        Ok(())
    }
    fn receive(&mut self, deadline: std::time::Instant) -> Result<Value> {
        use std::io::Read;
        use std::os::fd::AsRawFd;
        loop {
            if let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
                if end > MAX_FRAME { return Err("MCP response frame exceeds limit".into()); }
                let line: Vec<u8> = self.buffer.drain(..=end).collect();
                let value: Value = serde_json::from_slice(&line)?;
                if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || !value.is_object() { return Err("invalid MCP JSON-RPC message".into()); }
                return Ok(value);
            }
            if self.buffer.len() > MAX_FRAME { return Err("MCP response frame exceeds limit".into()); }
            Self::wait(self.output.as_raw_fd(), libc::POLLIN, deadline)?;
            let mut buf = [0u8; 4096];
            match self.output.read(&mut buf) {
                Ok(0) => return Err("MCP server closed stdout".into()),
                Ok(n) => { self.received = self.received.saturating_add(n); if self.received > MAX_TOTAL { return Err("MCP response budget exceeded".into()); } self.buffer.extend_from_slice(&buf[..n]); },
                Err(e) if matches!(e.kind(), io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock) => (),
                Err(e) => return Err(e.into()),
            }
        }
    }
    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id; self.next_id += 1;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}), deadline)?;
        for _ in 0..=32 {
            let message = self.receive(deadline)?;
            if message.get("method").is_some() {
                if message.get("id").is_some() { return Err("MCP server requests are not supported".into()); }
                continue;
            }
            if message.get("id").and_then(Value::as_u64) != Some(id) { return Err("unexpected MCP response ID".into()); }
            if message.get("error").is_some() { return Err("MCP server returned a JSON-RPC error".into()); }
            return message.get("result").filter(|v| v.is_object()).cloned().ok_or_else(|| "invalid MCP response result".into());
        }
        Err("too many MCP notifications".into())
    }
    fn initialize(&mut self) -> Result<()> {
        let init = self.request("initialize", json!({"protocolVersion":VERSION,"capabilities":{},"clientInfo":{"name":"isa-cli","version":"0.1.0"}}))?;
        if init.get("protocolVersion").and_then(Value::as_str) != Some(VERSION) || !init.get("capabilities").is_some_and(Value::is_object) || !init.get("serverInfo").is_some_and(Value::is_object) { return Err("unsupported MCP protocol version or invalid initialization".into()); }
        if !init["capabilities"].get("tools").is_some_and(Value::is_object) { return Err("MCP server does not advertise tools".into()); }
        self.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}), std::time::Instant::now() + std::time::Duration::from_secs(15))
    }
    fn tools(&mut self) -> Result<(Vec<Value>, bool)> {
        let mut tools = Vec::new(); let mut cursor: Option<String> = None; let mut seen = HashSet::new();
        for page in 0..MAX_PAGES {
            let params = cursor.as_ref().map(|c| json!({"cursor":c})).unwrap_or_else(|| json!({}));
            let result = self.request("tools/list", params)?;
            let page_tools = result.get("tools").and_then(Value::as_array).ok_or("invalid MCP tool list")?;
            for tool in page_tools {
                if !tool.is_object() || !tool.get("name").and_then(Value::as_str).is_some_and(|n| !n.is_empty() && n.len() <= 128 && !n.chars().any(char::is_control)) || !tool.get("inputSchema").is_some_and(Value::is_object) { return Err("invalid MCP tool metadata".into()); }
                if tools.len() == MAX_TOOLS { return Ok((tools, true)); }
                tools.push(tool.clone());
            }
            cursor = match result.get("nextCursor") { None | Some(Value::Null) => None, Some(Value::String(c)) if !c.is_empty() && c.len() <= 1024 => Some(c.clone()), _ => return Err("invalid MCP pagination cursor".into()) };
            let Some(next) = cursor.as_ref() else { return Ok((tools, false)); };
            if !seen.insert(next.clone()) { return Err("repeated MCP pagination cursor".into()); }
            if page + 1 == MAX_PAGES { return Ok((tools, true)); }
        }
        Ok((tools, true))
    }
}
#[cfg(unix)]
impl Drop for Client {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            if let Ok(pid) = i32::try_from(self.child.id()) { unsafe { libc::kill(-pid, libc::SIGKILL); } }
            let _ = self.child.kill(); let _ = self.child.wait();
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn start(mode: &str) -> Client {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp-server.cjs");
        Client::start(Path::new(env!("CARGO_MANIFEST_DIR")), &["node".into(), fixture.to_string_lossy().into(), mode.into()]).unwrap()
    }
    #[test]
    fn stdio_negotiates_discovers_pages_and_calls_real_fixture() {
        let mut client = start("valid"); client.initialize().unwrap();
        let (tools, truncated) = client.tools().unwrap(); assert!(!truncated); assert_eq!(tools.len(), 2); assert_eq!(tools[0]["name"], "echo"); assert_eq!(tools[1]["name"], "write");
        let result = client.request("tools/call", json!({"name":"echo","arguments":{"value":"hello"}})).unwrap();
        assert_eq!(result["content"][0]["text"], "fixture result");
    }
    #[test]
    fn rejects_bad_frames_versions_ids_and_cursor_loops() {
        for mode in ["invalid", "wrongid", "version", "oversized"] { let mut client = start(mode); assert!(client.initialize().is_err(), "{mode}"); }
        let mut client = start("cursor"); client.initialize().unwrap(); assert!(client.tools().is_err());
    }
    #[test]
    fn idle_server_hits_deadline_and_terminal_data_is_sanitized() {
        let mut client = start("hang");
        assert!(client.receive(std::time::Instant::now() + std::time::Duration::from_millis(150)).is_err());
        assert!(!display(&json!("\u{202e}\u{0085}")).unwrap().contains('\u{202e}'));
        assert!(validate_args(&["node".into(), "\x1b".into()]).is_err());
    }
}
