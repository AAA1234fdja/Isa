#![cfg(unix)]
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use std::sync::Mutex;
static START_LOCK: Mutex<()> = Mutex::new(());
use serde_json::{json, Value};
const TOKEN: &str = "test-only-local-capability-0123456789";
struct Server { child: Child, address: String, root: PathBuf }
impl Server {
    fn start(enabled: bool) -> Self { Self::configured(enabled,false) }
    fn configured(enabled: bool, mcp: bool) -> Self { Self::profile(enabled,mcp,None) }
    fn profile(enabled: bool, mcp: bool, profile: Option<&str>) -> Self { Self::fixture(enabled,mcp,profile,"valid") }
    fn fixture(enabled: bool, mcp: bool, profile: Option<&str>, fixture_mode: &str) -> Self {
        let _guard=START_LOCK.lock().unwrap();
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root=std::env::temp_dir().join(format!("isa-supervision-{}-{}",std::process::id(), NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed))); fs::create_dir_all(&root).unwrap();
        let socket=TcpListener::bind("127.0.0.1:0").unwrap(); let address=socket.local_addr().unwrap().to_string(); drop(socket);
        let mut command=Command::new(env!("CARGO_BIN_EXE_isa")); command.current_dir(&root).env("ISA_ADDRESS",&address).env_remove("ISA_WEB_EXEC_TOKEN").env_remove("ISA_WEB_EXEC_WORKSPACE").env_remove("ISA_WEB_AGENT_WORKSPACE").env_remove("ISA_WEB_AUDIT_ENCRYPTED").stdout(Stdio::null()).stderr(Stdio::null());
        if enabled { command.env("ISA_WEB_EXEC_TOKEN",TOKEN).env("ISA_WEB_EXEC_WORKSPACE",&root); }
        command.env_remove("ISA_WEB_MCP_COMMAND").env_remove("ISA_WEB_EXEC_PROFILE").env("ISA_TEST_SECRET","should-not-inherit");
        if let Some(profile)=profile { command.env("ISA_WEB_EXEC_PROFILE",profile); }
        if mcp { let source=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp-server.cjs"); let (fixture,marker)=if profile==Some("sandboxed") { fs::copy(source,root.join("mcp-server.cjs")).unwrap(); (PathBuf::from("/workspace/mcp-server.cjs"),PathBuf::from("/workspace/marker")) } else { (source,root.join("marker")) }; let output=Command::new("node").args(["-p","process.execPath"]).output().unwrap(); assert!(output.status.success()); let node=String::from_utf8(output.stdout).unwrap(); command.env("ISA_WEB_MCP_COMMAND",json!([node.trim(),fixture.to_string_lossy(),fixture_mode,marker.to_string_lossy()]).to_string()); }
        let child=command.spawn().unwrap(); let mut server=Self { child,address,root };
        for _ in 0..200 { if server.child.try_wait().unwrap().is_some() { panic!("server exited"); } if TcpStream::connect(&server.address).is_ok() { return server; } std::thread::sleep(Duration::from_millis(25)); } panic!("server failed to start");
    }
    fn restart_plain(&mut self) {
        self.child.kill().unwrap();self.child.wait().unwrap();
        self.child=Command::new(env!("CARGO_BIN_EXE_isa")).current_dir(&self.root).env("ISA_ADDRESS",&self.address).env("ISA_WEB_EXEC_TOKEN",TOKEN).env("ISA_WEB_EXEC_WORKSPACE",&self.root).env_remove("ISA_WEB_MCP_COMMAND").env_remove("ISA_WEB_EXEC_PROFILE").env_remove("ISA_WEB_AUDIT_ENCRYPTED").stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        for _ in 0..200 { if self.child.try_wait().unwrap().is_some() { panic!("server exited"); } if TcpStream::connect(&self.address).is_ok() { return; } std::thread::sleep(Duration::from_millis(25)); } panic!("server failed to restart");
    }
    fn post(&self,path:&str,value:Value,token:&str,origin:Option<&str>)->(u16,Value) {
        let mut socket=TcpStream::connect(&self.address).unwrap(); socket.set_read_timeout(Some(Duration::from_secs(10))).unwrap(); let body=value.to_string();
        let origin=origin.map(|o|format!("Origin: {o}\r\n")).unwrap_or_default();
        write!(socket,"POST {path} HTTP/1.1\r\nHost: {}\r\n{origin}Content-Type: application/json\r\nX-ISA-Exec-Token: {token}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",self.address,body.len()).unwrap();
        let mut response=String::new();socket.read_to_string(&mut response).unwrap();let (head,body)=response.split_once("\r\n\r\n").unwrap();(head.lines().next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap(),serde_json::from_str(body).unwrap())
    }
    fn req(&self,path:&str,value:Value)->(u16,Value) { self.post(path,value,TOKEN,Some(&format!("http://{}",self.address))) }
}
impl Drop for Server { fn drop(&mut self){let _=self.child.kill();let _=self.child.wait();let _=fs::remove_dir_all(&self.root);} }
#[test]
fn supervision_authenticates_and_enforces_modes_and_single_use() {
    let s=Server::start(true); let marker=s.root.join("marker");
    let policy=json!({"chat":"chat-a","mode":"always"});
    assert_eq!(s.post("/api/exec/policy",policy.clone(),"incorrect",Some(&format!("http://{}",s.address))).0,403);
    assert_eq!(s.post("/api/exec/policy",policy.clone(),TOKEN,Some("http://evil.test")).0,403);
    assert_eq!(s.post("/api/exec/policy",policy.clone(),TOKEN,None).0,403);
    assert_eq!(s.req("/api/exec/prepare",json!({"chat":"chat-a","command":"echo hi"})).0,409);
    assert_eq!(s.req("/api/exec/policy",policy).0,200);
    let (code,p)=s.req("/api/exec/prepare",json!({"chat":"chat-a","command":"printf hi > marker"}));assert_eq!(code,200);assert_eq!(p["requiresApproval"],true);
    let id=p["id"].as_str().unwrap();assert_eq!(s.req("/api/exec/run",json!({"chat":"chat-a","id":id,"approved":false})).0,403);assert!(!marker.exists());
    assert_eq!(s.req("/api/exec/run",json!({"chat":"chat-a","id":id,"approved":true})).0,409);
    let (_,p)=s.req("/api/exec/prepare",json!({"chat":"chat-a","command":"printf hi > marker"}));let id=p["id"].as_str().unwrap();
    let (code,r)=s.req("/api/exec/run",json!({"chat":"chat-a","id":id,"approved":true}));assert_eq!(code,200);assert_eq!(r["result"]["exitCode"],0);assert_eq!(fs::read_to_string(&marker).unwrap(),"hi");assert_eq!(s.req("/api/exec/run",json!({"chat":"chat-a","id":id,"approved":true})).0,409);
    assert_eq!(s.req("/api/exec/policy",json!({"chat":"chat-a","mode":"risk"})).0,200);let (_,p)=s.req("/api/exec/prepare",json!({"chat":"chat-a","command":"echo ok"}));assert_eq!(p["requiresApproval"],true);
    let id=p["id"].as_str().unwrap();assert_eq!(s.req("/api/exec/policy",json!({"chat":"chat-a","mode":"free"})).0,200);assert_eq!(s.req("/api/exec/run",json!({"chat":"chat-a","id":id,"approved":true})).0,409);
    let (_,p)=s.req("/api/exec/prepare",json!({"chat":"chat-a","command":"printf '%s' \"${ISA_WEB_EXEC_TOKEN-unset}\""}));assert_eq!(p["requiresApproval"],false);
    let (code,r)=s.req("/api/exec/run",json!({"chat":"chat-a","id":p["id"],"approved":false}));assert_eq!(code,200);assert_eq!(r["result"]["stdout"],"unset");
    let (_,p)=s.req("/api/exec/prepare",json!({"chat":"chat-a","command":"echo cancelled"}));assert_eq!(s.req("/api/exec/cancel",json!({"chat":"chat-a","id":p["id"]})).0,200);assert_eq!(s.req("/api/exec/run",json!({"chat":"chat-a","id":p["id"],"approved":false})).0,409);
    assert_eq!(s.req("/api/exec/prepare",json!({"chat":"chat-b","command":"echo hi"})).0,409);
}
#[test]
fn disabled_by_default() { let s=Server::start(false);assert_eq!(s.req("/api/exec/policy",json!({"chat":"x","mode":"free"})).0,403); }
#[test]
fn platform_hint_does_not_mean_supervision_is_enabled() {
    let s=Server::start(false);
    let (status, options)=s.req("/api/chat-options",json!({}));
    assert_eq!(status,200);
    assert_eq!(options["localActionsPlatformSupported"],true);
    assert_eq!(s.req("/api/exec/policy",json!({"chat":"x","mode":"free"})).0,403);
}

#[test]
fn common_supervisor_enforces_action_type_and_risk_policy() {
    let s=Server::start(true);
    let prepare=|action:Value| s.req("/api/actions/prepare",json!({"chat":"a","action":action}));
    assert_eq!(prepare(json!({"type":"workspace_info"})).0,409);
    assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"always"})).0,200);
    assert_eq!(prepare(json!({"type":"mcp_call","name":"unsafe"})).0,400);
    assert_eq!(prepare(json!({"type":"workspace_info","risk":"low"})).0,400);
    assert_eq!(prepare(json!({"type":"shell","command":" "})).0,400);
    let (status,p)=prepare(json!({"type":"workspace_info"}));assert_eq!(status,200);assert_eq!(p["action"],json!({"type":"workspace_info"}));assert_eq!(p["requiresApproval"],true);
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,403);
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);
    let (_,p)=prepare(json!({"type":"workspace_info"}));
    assert_eq!(s.req("/api/exec/run",json!({"chat":"a","id":p["id"],"approved":true})).0,403);
    let (_,p)=prepare(json!({"type":"workspace_info"}));
    let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,200);assert_eq!(r["result"]["isDirectory"],true);assert_eq!(r["result"]["workspace"],s.root.canonicalize().unwrap().to_string_lossy().as_ref());
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);
    let (_,p)=prepare(json!({"type":"workspace_info"}));assert_eq!(s.req("/api/actions/run",json!({"chat":"b","id":p["id"],"approved":true})).0,403);
    let (_,p)=prepare(json!({"type":"workspace_info"}));assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"})).0,200);assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);
    let (_,p)=prepare(json!({"type":"workspace_info"}));assert_eq!(p["requiresApproval"],false);assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,200);
    let (_,p)=prepare(json!({"type":"shell","command":"printf common"}));assert_eq!(p["requiresApproval"],true);assert_eq!(p["action"],json!({"type":"shell","command":"printf common"}));let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,200);assert_eq!(r["result"]["stdout"],"common");
    let (_,p)=prepare(json!({"type":"workspace_info"}));assert_eq!(s.req("/api/actions/cancel",json!({"chat":"a","id":p["id"]})).0,200);assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);
    assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"free"})).0,200);let (_,p)=prepare(json!({"type":"shell","command":"echo ok"}));assert_eq!(p["requiresApproval"],false);
}

#[test]
fn mcp_start_and_tool_call_are_separately_supervised() {
    let s=Server::configured(true,true);let started=s.root.join("marker.started");let called=s.root.join("marker");
    assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"always"})).0,200);
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_start"}}));assert_eq!(p["requiresApproval"],true);assert!(!started.exists());
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,403);assert!(!started.exists());
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_start"}}));
    let (code,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(code,200);assert!(started.exists());assert!(!called.exists());let session=r["result"]["session"].as_str().unwrap();assert_eq!(r["result"]["tools"].as_array().unwrap().len(),2);
    let action=json!({"type":"mcp_call","session":session,"name":"write","arguments":{"value":"test"}});
    assert_eq!(s.req("/api/actions/prepare",json!({"chat":"other","action":action})).0,409);
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":action}));assert_eq!(p["requiresApproval"],true);assert!(!called.exists());
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,403);assert!(!called.exists());
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":action}));let (code,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(code,200);assert_eq!(r["result"]["content"][0]["text"],"fixture result");assert_eq!(serde_json::from_str::<Value>(&fs::read_to_string(&called).unwrap()).unwrap(),json!({"name":"write","arguments":{"value":"test"}}));
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);assert_eq!(s.req("/api/actions/prepare",json!({"chat":"a","action":action})).0,409);
}
#[test]
fn mcp_rejects_unknown_tools_and_invalidates_sessions_on_policy_change() {
    let s=Server::configured(true,true);s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"}));
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_start"}}));assert_eq!(p["requiresApproval"],true);
    let (_,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));let session=r["result"]["session"].as_str().unwrap();
    assert_eq!(s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_call","session":session,"name":"missing","arguments":{}}})).0,403);
    assert_eq!(s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_call","session":session,"name":"echo","arguments":{"bad":"\u{202e}"}}})).0,400);
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_call","session":session,"name":"echo","arguments":{}}}));assert_eq!(p["requiresApproval"],true);
    s.req("/api/exec/policy",json!({"chat":"a","mode":"free"}));assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);assert!(!s.root.join("marker").exists());
}

#[test]
fn pool_action_budgets_are_shared_and_bound_to_agents_sessions_and_chat() {
    let s=Server::configured(true,true);
    assert_eq!(s.req("/api/exec/policy",json!({"chat":"pool-chat","mode":"risk"})).0,200);
    let (code,r)=s.req("/api/actions/pool/start",json!({"chat":"pool-chat","agents":["agent-a","agent-b"]}));assert_eq!(code,200);let run=r["run"].as_str().unwrap();assert_eq!(r["maxCalls"],8);
    let p=|agent:&str, action:Value| s.req("/api/actions/prepare",json!({"chat":"pool-chat","poolRun":run,"agent":agent,"action":action}));
    assert_eq!(p("other",json!({"type":"mcp_start"})).0,403);
    assert_eq!(p("agent-a",json!({"type":"workspace_info"})).0,400);
    for i in 0..8 {
        let agent=if i%2==0 {"agent-a"} else {"agent-b"};
        let (code,pending)=p(agent,json!({"type":"mcp_start"}));assert_eq!(code,200);assert_eq!(pending["requiresApproval"],true);assert_eq!(pending["agent"],agent);
        let (code,result)=s.req("/api/actions/run",json!({"chat":"pool-chat","id":pending["id"],"approved":true}));assert_eq!(code,200);let session=result["result"]["session"].as_str().unwrap();
        let call=json!({"type":"mcp_call","session":session,"name":"echo","arguments":{"i":i}});
        assert_eq!(s.req("/api/actions/prepare",json!({"chat":"pool-chat","action":call})).0,403);
        let other=if agent=="agent-a" {"agent-b"} else {"agent-a"};
        assert_eq!(p(other,call.clone()).0,403);
        let (code,pending)=p(agent,call);assert_eq!(code,200);assert_eq!(pending["requiresApproval"],true);
        let (code,_)=s.req("/api/actions/run",json!({"chat":"pool-chat","id":pending["id"],"approved":true}));assert_eq!(code,200);
    }
    let (_,pending)=p("agent-b",json!({"type":"mcp_start"}));assert_eq!(s.req("/api/actions/run",json!({"chat":"pool-chat","id":pending["id"],"approved":true})).0,429);
    assert_eq!(s.req("/api/actions/pool/close",json!({"chat":"pool-chat","run":run})).0,200);
    assert_eq!(p("agent-a",json!({"type":"mcp_start"})).0,409);
}

#[cfg(target_os="linux")]
#[test]
fn limited_profile_is_opt_in_and_bounds_web_process_resources() {
    let s=Server::profile(true,false,Some("limited"));
    let (code,policy)=s.req("/api/exec/policy",json!({"chat":"a","mode":"free"}));assert_eq!(code,200);assert_eq!(policy["executionProfile"],"limited");
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"shell","command":"printf '%s\\n' \"${ISA_TEST_SECRET-unset}\" \"${ISA_WEB_EXEC_TOKEN-unset}\" \"$(ulimit -c)\" \"$(ulimit -n)\" \"$(ulimit -v)\""}}));
    assert_eq!(p["executionProfile"],"limited");
    let (code,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false}));assert_eq!(code,200);assert_eq!(r["result"]["exitCode"],0);
    let lines:Vec<_>=r["result"]["stdout"].as_str().unwrap().lines().collect();assert_eq!(lines[0],"unset");assert_eq!(lines[1],"unset");assert_eq!(lines[2],"0");assert!(lines[3].parse::<u64>().unwrap()<=128);assert!(lines[4].parse::<u64>().unwrap()<=3_906_250);
}
#[cfg(target_os="linux")]
#[test]
fn limited_profile_applies_to_mcp_and_invalid_profile_fails_closed() {
    let s=Server::profile(true,true,Some("limited"));s.req("/api/exec/policy",json!({"chat":"a","mode":"free"}));
    let (code,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_start"}}));assert_eq!(code,200);assert_eq!(p["executionProfile"],"limited");
    let (code,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false}));assert_eq!(code,200);assert!(r["result"]["tools"].is_array());
    s.req("/api/actions/mcp/close",json!({"chat":"a","session":r["result"]["session"]}));
    let disabled=Server::profile(true,false,Some("unknown"));assert_eq!(disabled.req("/api/exec/policy",json!({"chat":"a","mode":"free"})).0,503);
}

#[test]
fn event_history_is_bounded_redacted_and_scoped_to_chat() {
    let s=Server::start(true);
    assert_eq!(s.post("/api/actions/events",json!({"chat":"a"}),"wrong",Some(&format!("http://{}",s.address))).0,403);
    assert_eq!(s.post("/api/actions/events",json!({"chat":"a"}),TOKEN,Some("http://evil.test")).0,403);
    s.req("/api/exec/policy",json!({"chat":"a","mode":"always"}));
    let (_,p)=s.req("/api/exec/prepare",json!({"chat":"a","command":"printf private-command-and-output"}));
    assert_eq!(s.req("/api/exec/run",json!({"chat":"a","id":p["id"],"approved":false})).0,403);
    let (_,p2)=s.req("/api/exec/prepare",json!({"chat":"a","command":"printf private-command-and-output"}));
    let (code,_)=s.req("/api/exec/run",json!({"chat":"a","id":p2["id"],"approved":true}));assert_eq!(code,200);
    let (code,h)=s.req("/api/actions/events",json!({"chat":"a"}));assert_eq!(code,200);let events=h["events"].as_array().unwrap();
    let statuses:Vec<_>=events.iter().map(|e|e["status"].as_str().unwrap()).collect();assert_eq!(statuses,vec!["proposed","denied","proposed","approved","dispatched","completed"]);
    assert_eq!(events[0]["actionRef"],events[1]["actionRef"]);assert_ne!(events[0]["actionRef"],p["id"]);assert_ne!(events[0]["actionRef"],events[2]["actionRef"]);
    for e in events { let mut keys:Vec<_>=e.as_object().unwrap().keys().map(String::as_str).collect();keys.sort();assert_eq!(keys,vec!["actionRef","kind","source","status","unixSeconds"]); }
    let text=h.to_string();assert!(!text.contains("private-command-and-output"));assert!(!text.contains(TOKEN));assert!(!text.contains(p["id"].as_str().unwrap()));
    assert!(s.req("/api/actions/events",json!({"chat":"b"})).1["events"].as_array().unwrap().is_empty());
    for _ in 0..55 { let (_,p)=s.req("/api/exec/prepare",json!({"chat":"a","command":"echo redacted"}));s.req("/api/exec/cancel",json!({"chat":"a","id":p["id"]})); }
    let (_,h)=s.req("/api/actions/events",json!({"chat":"a"}));assert_eq!(h["events"].as_array().unwrap().len(),100);assert!(!h.to_string().contains("echo redacted"));
}
#[test]
fn history_reports_cancellation_denial_and_policy_invalidation() {
    let s=Server::start(true);s.req("/api/exec/policy",json!({"chat":"a","mode":"always"}));
    let (_,p)=s.req("/api/exec/prepare",json!({"chat":"a","command":"echo denied"}));s.req("/api/exec/cancel",json!({"chat":"a","id":p["id"],"denied":true}));
    let (_,p)=s.req("/api/exec/prepare",json!({"chat":"a","command":"echo cancelled"}));s.req("/api/exec/cancel",json!({"chat":"a","id":p["id"]}));
    s.req("/api/exec/prepare",json!({"chat":"a","command":"echo changed"}));s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"}));
    let (_,h)=s.req("/api/actions/events",json!({"chat":"a"}));let statuses:Vec<_>=h["events"].as_array().unwrap().iter().map(|e|e["status"].as_str().unwrap()).collect();
    assert_eq!(statuses,vec!["proposed","denied","proposed","cancelled","proposed","policy_changed"]);
}

#[test]
fn idempotent_mcp_tool_error_is_retried_but_other_errors_are_not() {
    for (mode,expected_attempts) in [("retry",2),("toolerror",1)] {
        let s=Server::fixture(true,true,None,mode);
        assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"always"})).0,200);
        let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_start"}}));
        let (_,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));
        let session=r["result"]["session"].as_str().unwrap();
        let (status,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_call","session":session,"name":"echo","arguments":{}}}));
        assert_eq!(status,200);assert_eq!(p["maxAttempts"],expected_attempts);
        let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));
        assert_eq!(status,200);assert_eq!(r["attempts"],expected_attempts);
        assert_eq!(fs::read_to_string(s.root.join("marker.calls")).unwrap(),expected_attempts.to_string());
        let (_,events)=s.req("/api/actions/events",json!({"chat":"a"}));
        let retries=events["events"].as_array().unwrap().iter().filter(|e|e["status"]=="retrying").count();assert_eq!(retries,if expected_attempts==2 {1} else {0});
    }
}

#[cfg(target_os="linux")]
#[test]
fn sandboxed_profile_isolates_files_and_allows_workspace_network_and_mcp() {
    if std::env::var_os("ISA_SANDBOX_TEST").is_none() { return; }
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
    let s=Server::profile(true,true,Some("sandboxed"));
    let outside=s.root.with_extension("outside");fs::write(&outside,"secret").unwrap();
    let (code,policy)=s.req("/api/exec/policy",json!({"chat":"a","mode":"free"}));assert_eq!(code,200);assert_eq!(policy["executionProfile"],"sandboxed");
    let command=format!("test ! -e '{}' && echo isolated > /workspace/proof && : > /dev/tcp/127.0.0.1/{port}",outside.display());
    let (code,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"shell","command":command}}));assert_eq!(code,200);
    let (code,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false}));assert_eq!(code,200);assert_eq!(r["result"]["exitCode"],0);assert_eq!(fs::read_to_string(s.root.join("proof")).unwrap().trim(),"isolated");
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_start"}}));let (code,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false}));assert_eq!(code,200);assert!(r["result"]["tools"].is_array());
    assert!(s.root.join("marker.started").exists());fs::remove_file(outside).unwrap();
}

#[test]
fn supervised_web_edit_is_one_use_and_rejects_stale_or_protected_files() {
    let s=Server::start(true);let file=s.root.join("a.txt");fs::write(&file,"old text\n").unwrap();
    let proposal="File: a.txt\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE";
    assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"})).0,200);
    let prepare=||s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit","proposal":proposal}}));
    let (code,p)=prepare();assert_eq!(code,200);assert_eq!(p["requiresApproval"],true);assert_eq!(p["review"]["file"],"a.txt");
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,403);assert_eq!(fs::read_to_string(&file).unwrap(),"old text\n");
    let (_,p)=prepare();fs::write(&file,"changed old text\n").unwrap();assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);assert_eq!(fs::read_to_string(&file).unwrap(),"changed old text\n");
    fs::write(&file,"old text\n").unwrap();let (_,p)=prepare();let (code,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(code,200);assert_eq!(r["result"]["applied"],true);assert_eq!(fs::read_to_string(&file).unwrap(),"new text\n");assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);
    // Policy changes invalidate prepared approvals; free mode still consumes once.
    fs::write(&file,"old text\n").unwrap(); let (_,p)=prepare(); assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"free"})).0,200);
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);
    fs::write(&file,"old text\n").unwrap();let (_,p)=prepare();assert_eq!(p["requiresApproval"],false);
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,200);
    fs::write(&file,"old old").unwrap();assert_eq!(prepare().0,422);
    #[cfg(unix)] { std::os::unix::fs::symlink(&file,s.root.join("link.txt")).unwrap();let link="File: link.txt\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE";assert_eq!(s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit","proposal":link}})).0,422); }
    fs::write(s.root.join(".env"),"old").unwrap();let protected="File: .env\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE";assert_eq!(s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit","proposal":protected}})).0,422);
    let (_,h)=s.req("/api/actions/events",json!({"chat":"a"}));assert!(!h.to_string().contains(proposal));assert!(h["events"].as_array().unwrap().iter().any(|e|e["kind"]=="edit"&&e["status"]=="completed"));
}

#[test]
fn supervised_mcp_rejects_invalid_arguments_before_tool_dispatch() {
    let s=Server::fixture(true,true,None,"schema"); let marker=s.root.join("marker");
    assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"})).0,200);
    let (_,start)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_start"}}));
    let (code,result)=s.req("/api/actions/run",json!({"chat":"a","id":start["id"],"approved":true}));assert_eq!(code,200);
    let session=result["result"]["session"].as_str().unwrap();
    for arguments in [json!({}),json!({"value":1}),json!({"value":"ok","extra":true})] {
        let (status,_)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_call","session":session,"name":"echo","arguments":arguments}}));
        assert_eq!(status,422);assert!(!marker.exists());
    }
    let (status,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"mcp_call","session":session,"name":"echo","arguments":{"value":"ok"}}}));assert_eq!(status,200);
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,200);assert!(marker.exists());
}

#[test]
fn undo_is_supervised_chat_bound_reusable_and_rejects_stale_files() {
    let s=Server::start(true);let file=s.root.join("undo.txt");fs::write(&file,"old old\n").unwrap();
    // The resulting replacement occurs twice, so an inverse SEARCH is not a real undo.
    let proposal="File: undo.txt\n<<<<<<< SEARCH\nold old\n=======\nnew new\n>>>>>>> REPLACE";
    assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"})).0,200);
    assert_eq!(s.req("/api/exec/policy",json!({"chat":"b","mode":"risk"})).0,200);
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit","proposal":proposal}}));
    let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,200);let undo=r["result"]["undoId"].as_str().unwrap();
    let prepare=|chat:&str|s.req("/api/actions/prepare",json!({"chat":chat,"action":{"type":"edit_undo","undo_id":undo}}));
    assert_eq!(prepare("b").0,403);let (status,p)=prepare("a");assert_eq!(status,200);assert_eq!(p["requiresApproval"],true);
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,403);assert_eq!(fs::read_to_string(&file).unwrap(),"new new\n");
    let (_,p)=prepare("a");fs::write(&file,"changed\n").unwrap();assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);assert_eq!(fs::read_to_string(&file).unwrap(),"changed\n");assert_eq!(prepare("a").0,409);
    fs::write(&file,"old old\n").unwrap();let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit","proposal":proposal}}));let (_,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));let undo2=r["result"]["undoId"].as_str().unwrap();
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit_undo","undo_id":undo2}}));let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,200);assert_eq!(r["result"]["undone"],true);assert_eq!(fs::read_to_string(&file).unwrap(),"old old\n");let (status,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit_undo","undo_id":undo2}}));assert_eq!(status,200);assert_eq!(p["review"]["alreadyUndone"],true);let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,200);assert_eq!(r["result"]["alreadyUndone"],true);
}

#[test]
fn running_shell_can_be_stopped_on_server_and_pending_shell_can_be_cancelled() {
    let s=Server::start(true);assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"})).0,200);
    let (_,p)=s.req("/api/exec/prepare",json!({"chat":"a","command":"echo started > running; sleep 30; echo late > finished"}));let id=p["id"].as_str().unwrap().to_string();
    std::thread::scope(|scope| {
        let request=scope.spawn(||s.req("/api/exec/run",json!({"chat":"a","id":id,"approved":true})));
        for _ in 0..200 { if s.root.join("running").exists() { break } std::thread::sleep(Duration::from_millis(10)); }
        assert!(s.root.join("running").exists());
        assert_eq!(s.req("/api/actions/stop",json!({"chat":"other","id":id})).1["requested"],false);
        assert_eq!(s.req("/api/actions/stop",json!({"chat":"a","id":id})).1["requested"],true);
        let (status,result)=request.join().unwrap();assert_eq!(status,200);assert_eq!(result["result"]["cancelled"],true);assert!(!s.root.join("finished").exists());
    });
    let (_,p)=s.req("/api/exec/prepare",json!({"chat":"a","command":"echo should-not-run > finished"}));
    assert_eq!(s.req("/api/actions/stop",json!({"chat":"a","id":p["id"]})).1["requested"],true);
    assert_eq!(s.req("/api/exec/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);assert!(!s.root.join("finished").exists());
}

#[test]
fn supervised_transaction_rejects_aliases_and_binds_all_snapshots() {
    let s=Server::start(true);
    fs::create_dir(s.root.join("dir")).unwrap();
    fs::write(s.root.join("dir/a"),"old a").unwrap();fs::write(s.root.join("b"),"old b").unwrap();
    let proposal=|name:&str| format!("File: {name}\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE");
    let prepare=|proposals:Vec<String>| s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit_transaction","proposals":proposals}}));
    s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"}));
    let (code,p)=prepare(vec![proposal("dir/a"),proposal("b")]);assert_eq!(code,200);assert_eq!(p["requiresApproval"],true);assert_eq!(p["review"]["files"].as_array().unwrap().len(),2);assert_eq!(p["review"]["atomic"],false);assert_eq!(p["review"]["undoAvailable"],true);
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,403);assert_eq!(fs::read_to_string(s.root.join("dir/a")).unwrap(),"old a");
    let (_,p)=prepare(vec![proposal("dir/a"),proposal("b")]);fs::write(s.root.join("b"),"changed old b").unwrap();
    let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,409);assert_eq!(r["status"],"not_started");assert_eq!(fs::read_to_string(s.root.join("dir/a")).unwrap(),"old a");
    fs::write(s.root.join("b"),"old b").unwrap();let (_,p)=prepare(vec![proposal("dir/a"),proposal("b")]);
    let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,200);assert_eq!(r["result"]["status"],"applied");assert_eq!(r["result"]["undoAvailable"],true);assert_eq!(fs::read_to_string(s.root.join("dir/a")).unwrap(),"new a");assert_eq!(fs::read_to_string(s.root.join("b")).unwrap(),"new b");assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);
    let undo=r["result"]["undoId"].as_str().unwrap();
    let undo_prepare=|| s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit_transaction_undo","undo_id":undo}}));
    let (code,u)=undo_prepare();assert_eq!(code,200);assert_eq!(u["requiresApproval"],true);assert_eq!(u["review"]["files"].as_array().unwrap().len(),2);
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":u["id"],"approved":false})).0,403);
    let (_,u)=undo_prepare();fs::write(s.root.join("b"),"changed").unwrap();assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":u["id"],"approved":true})).0,409);assert_eq!(fs::read_to_string(s.root.join("dir/a")).unwrap(),"new a");assert_eq!(undo_prepare().0,409);
    fs::write(s.root.join("dir/a"),"old a").unwrap();fs::write(s.root.join("b"),"old b").unwrap();let (_,p)=prepare(vec![proposal("dir/a"),proposal("b")]);let (_,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));let undo=r["result"]["undoId"].as_str().unwrap();let (_,u)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit_transaction_undo","undo_id":undo}}));let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":u["id"],"approved":true}));assert_eq!(status,200);assert_eq!(r["result"]["status"],"undone");assert_eq!(fs::read_to_string(s.root.join("dir/a")).unwrap(),"old a");assert_eq!(fs::read_to_string(s.root.join("b")).unwrap(),"old b");let (status,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit_transaction_undo","undo_id":undo}}));assert_eq!(status,200);assert_eq!(p["review"]["alreadyUndone"],true);let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,200);assert_eq!(r["result"]["alreadyUndone"],true);
    assert_eq!(prepare(vec![proposal("dir/a"),proposal("dir/./a")]).0,422);
    std::os::unix::fs::symlink(s.root.join("dir/a"),s.root.join("link")).unwrap();assert_eq!(prepare(vec![proposal("link"),proposal("b")]).0,422);
    fs::hard_link(s.root.join("dir/a"),s.root.join("hard")).unwrap();assert_eq!(prepare(vec![proposal("dir/a"),proposal("b")]).0,422);assert_eq!(prepare(vec![proposal("hard"),proposal("b")]).0,422);
    fs::remove_file(s.root.join("hard")).unwrap();fs::write(s.root.join(".env"),"old").unwrap();assert_eq!(prepare(vec![proposal(".env"),proposal("b")]).0,422);
    assert_eq!(prepare(vec![proposal("../outside"),proposal("b")]).0,422);
    let (_,p)=prepare(vec![proposal("dir/a"),proposal("b")]);s.req("/api/exec/policy",json!({"chat":"a","mode":"free"}));assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);
    let (_,p)=prepare(vec![proposal("dir/a"),proposal("b")]);assert_eq!(p["requiresApproval"],false);assert_eq!(s.req("/api/actions/cancel",json!({"chat":"a","id":p["id"]})).0,200);assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,409);
    s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"}));
    let (_,p)=prepare(vec![proposal("dir/a"),proposal("b")]);fs::hard_link(s.root.join("dir/a"),s.root.join("hard")).unwrap();assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);assert_eq!(fs::read_to_string(s.root.join("b")).unwrap(),"old b");
}

#[test]
fn undo_records_survive_server_restart_for_single_and_transaction() {
    let mut s=Server::start(true);fs::write(s.root.join("a"),"old").unwrap();fs::write(s.root.join("b"),"old").unwrap();fs::write(s.root.join("c"),"old").unwrap();
    let prop=|name:&str|format!("File: {name}\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE");
    s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"}));
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit","proposal":prop("a")}}));let (_,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));let single=r["result"]["undoId"].as_str().unwrap().to_owned();
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit_transaction","proposals":[prop("b"),prop("c")]}}));let (_,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));let batch=r["result"]["undoId"].as_str().unwrap().to_owned();
    assert!(s.root.join(".isa/web-undo").join(&single).exists());assert!(s.root.join(".isa/web-undo").join(&batch).exists());s.restart_plain();s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"}));
    for (kind,id) in [("edit_undo",single),("edit_transaction_undo",batch)] { let (status,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":kind,"undo_id":id}}));assert_eq!(status,200);let (status,_)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,200);let (status,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":kind,"undo_id":id}}));assert_eq!(status,200);let (status,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,200);assert_eq!(r["result"]["alreadyUndone"],true); }
    for name in ["a","b","c"] {assert_eq!(fs::read_to_string(s.root.join(name)).unwrap(),"old")}
}

#[test]
fn workspace_bindings_are_read_only_and_require_local_capability() {
    let s=Server::start(true);let origin=format!("http://{}",s.address);
    assert_eq!(s.post("/api/actions/workspaces",json!({}),"wrong",Some(&origin)).0,403);
    assert_eq!(s.post("/api/actions/workspaces",json!({}),TOKEN,Some("http://evil.test")).0,403);
    assert_eq!(s.req("/api/actions/workspaces",json!({"unexpected":true})).0,400);
    let (status,r)=s.req("/api/actions/workspaces",json!({}));assert_eq!(status,200);assert_eq!(r["executionWorkspace"],s.root.canonicalize().unwrap().to_string_lossy().as_ref());assert!(r["agentWorkspace"].is_null());assert!(r["sameWorkspace"].is_null());assert_eq!(r["executionProfile"],"host");
    assert_eq!(s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"workspace_info"}})).0,409);
}

#[test]
fn cli_session_bridge_is_authenticated_read_only_and_workspace_bound() {
    let s=Server::start(true);let sessions=s.root.join(".isa/sessions");fs::create_dir_all(&sessions).unwrap();
    let original=json!({"version":1,"messages":[{"role":"user","content":"private task"},{"role":"assistant","content":"answer"}]}).to_string();fs::write(sessions.join("demo.json"),&original).unwrap();
    let origin=format!("http://{}",s.address);
    assert_eq!(s.post("/api/actions/sessions/list",json!({}),"wrong",Some(&origin)).0,403);
    assert_eq!(s.post("/api/actions/sessions/read",json!({"id":"demo","workspace":s.root}),TOKEN,Some("http://evil.test")).0,403);
    let (status,list)=s.req("/api/actions/sessions/list",json!({}));assert_eq!(status,200);assert_eq!(list["sessions"],json!(["demo"]));assert_eq!(list["truncated"],false);let workspace=list["workspace"].as_str().unwrap();
    assert_eq!(s.req("/api/actions/sessions/read",json!({"id":"demo","workspace":"/different"})).0,409);
    assert_eq!(s.req("/api/actions/sessions/read",json!({"id":"../demo","workspace":workspace})).0,422);
    let (status,r)=s.req("/api/actions/sessions/read",json!({"id":"demo","workspace":workspace}));assert_eq!(status,200);assert_eq!(r["messages"][0]["text"],"private task");assert_eq!(r["messages"][1]["role"],"assistant");assert_eq!(fs::read_to_string(sessions.join("demo.json")).unwrap(),original);
    std::os::unix::fs::symlink(sessions.join("demo.json"),sessions.join("link.json")).unwrap();assert_eq!(s.req("/api/actions/sessions/read",json!({"id":"link","workspace":workspace})).0,422);
    fs::write(sessions.join("oversize.json"),vec![b'x';256*1024+1]).unwrap();assert_eq!(s.req("/api/actions/sessions/read",json!({"id":"oversize","workspace":workspace})).0,422);
    fs::write(sessions.join("invalid.json"),json!({"version":1,"messages":[{"role":"system","content":"unsafe"}]}).to_string()).unwrap();assert_eq!(s.req("/api/actions/sessions/read",json!({"id":"invalid","workspace":workspace})).0,422);
}

#[test]
fn semantic_index_search_is_authenticated_scoped_and_supervised() {
    let s=Server::start(true);let origin=format!("http://{}",s.address);let action=json!({"type":"semantic_search","query":"locate session"});
    assert_eq!(s.post("/api/actions/prepare",json!({"chat":"a","action":action}),"bad",Some(&origin)).0,403);
    assert_eq!(s.req("/api/actions/prepare",json!({"chat":"a","action":action})).0,409);
    s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"}));
    assert_eq!(s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"semantic_search","query":""}})).0,400);
    assert_eq!(s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"semantic_search","query":"bad\u{202e}"}})).0,400);
    let (status,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":action}));assert_eq!(status,200);assert_eq!(p["requiresApproval"],true);
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,403);
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":action}));let (status,_)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(status,422); // Missing index, no silent fallback.
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);
    s.req("/api/exec/policy",json!({"chat":"a","mode":"free"}));let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":action}));assert_eq!(p["requiresApproval"],false);
    let (_,history)=s.req("/api/actions/events",json!({"chat":"a"}));assert!(!history.to_string().contains("locate session"));
}

#[test]
fn work_tracker_is_scoped_supervised_and_baseline_bound() {
    let s=Server::start(true);fs::write(s.root.join("source.txt"),"old\nkeep\n").unwrap();
    let prepare=|kind:&str|s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":kind}}));
    let run=|id:&Value,approved:bool|s.req("/api/actions/run",json!({"chat":"a","id":id,"approved":approved}));
    s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"}));
    let (code,p)=prepare("work_start");assert_eq!(code,200);assert_eq!(p["requiresApproval"],true);assert_eq!(run(&p["id"],false).0,403);assert!(!s.root.join(".isa/work/baseline.json").exists());
    let (_,p)=prepare("work_start");let (code,r)=run(&p["id"],true);assert_eq!(code,200);assert_eq!(r["result"]["started"],true);
    fs::write(s.root.join("source.txt"),"new\nkeep\n").unwrap();let (code,p)=prepare("work_status");assert_eq!(code,200);assert_eq!(p["requiresApproval"],false);let (code,r)=run(&p["id"],false);assert_eq!(code,200);assert_eq!(r["result"]["added"],1);assert_eq!(r["result"]["removed"],1);assert_eq!(r["result"]["changedFiles"],1);
    let baseline=s.root.join(".isa/work/baseline.json");let old=fs::read(&baseline).unwrap();let (_,p)=prepare("work_end");assert_eq!(p["requiresApproval"],true);fs::write(&baseline,b"changed baseline").unwrap();assert_eq!(run(&p["id"],true).0,409);assert_eq!(fs::read(&baseline).unwrap(),b"changed baseline");
    fs::write(&baseline,old).unwrap();let (_,p)=prepare("work_end");let (code,r)=run(&p["id"],true);assert_eq!(code,200);assert_eq!(r["result"]["ended"],true);assert!(!baseline.exists());assert_eq!(run(&p["id"],true).0,409);
}

#[test]
fn running_actions_are_supervised_scoped_and_do_not_expose_commands() {
    let s=Server::start(true);
    for chat in ["a","b"] { assert_eq!(s.req("/api/exec/policy",json!({"chat":chat,"mode":"risk"})).0,200); }
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"shell","command":"touch running-marker; sleep 8"}}));
    std::thread::scope(|scope| {
        let task=scope.spawn(||s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})));
        let deadline=std::time::Instant::now()+Duration::from_secs(4);
        while !s.root.join("running-marker").exists() { assert!(std::time::Instant::now()<deadline,"shell did not start");std::thread::sleep(Duration::from_millis(20)); }
        for (chat,count) in [("a",1),("b",0)] {
            let (code,read)=s.req("/api/actions/prepare",json!({"chat":chat,"action":{"type":"running_actions"}}));assert_eq!(code,200);assert_eq!(read["requiresApproval"],false);
            let (code,result)=s.req("/api/actions/run",json!({"chat":chat,"id":read["id"],"approved":false}));assert_eq!(code,200);assert_eq!(result["result"]["runningShellActions"],count);if chat=="a" {assert_eq!(result["result"]["actionIds"][0],p["id"])} else {assert!(result["result"]["actionIds"].as_array().unwrap().is_empty())}assert!(!result.to_string().contains("running-marker"));
        }
        let (_,other)=s.req("/api/actions/prepare",json!({"chat":"b","action":{"type":"stop_shell","target":p["id"]}}));assert_eq!(other["requiresApproval"],true);
        let (_,result)=s.req("/api/actions/run",json!({"chat":"b","id":other["id"],"approved":true}));assert_eq!(result["result"]["requested"],false);
        let (_,denied)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"stop_shell","target":p["id"]}}));assert_eq!(denied["requiresApproval"],true);assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":denied["id"],"approved":false})).0,403);
        let (_,stop)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"stop_shell","target":p["id"]}}));let (_,result)=s.req("/api/actions/run",json!({"chat":"a","id":stop["id"],"approved":true}));assert_eq!(result["result"]["requested"],true);
        assert_eq!(task.join().unwrap().0,200);
    });
    s.req("/api/exec/policy",json!({"chat":"a","mode":"always"}));
    let (_,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"running_actions"}}));assert_eq!(p["requiresApproval"],true);assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,403);
}

#[test]
fn audit_history_survives_restart_and_reports_storage_failure() {
    use std::os::unix::fs::symlink;
    let mut s=Server::start(true);s.req("/api/exec/policy",json!({"chat":"a","mode":"free"}));
    let secret="test-secret-not-for-audit";
    let (code,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"shell","command":format!("printf {secret}")}}));assert_eq!(code,200);
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":false})).0,200);
    let log=s.root.join(".isa/web-audit/events.jsonl");let text=fs::read_to_string(&log).unwrap();assert!(!text.contains(secret));assert!(!text.contains("printf"));assert!(!text.contains(TOKEN));
    s.restart_plain();let (code,history)=s.req("/api/actions/events",json!({"chat":"a"}));assert_eq!(code,200);assert_eq!(history["persistence"],"best_effort");assert_eq!(history["volatile"],false);assert!(history["events"].as_array().unwrap().iter().any(|event|event["kind"]=="shell"));
    let (_,other)=s.req("/api/actions/events",json!({"chat":"other"}));assert!(other["events"].as_array().unwrap().is_empty());
    fs::remove_file(&log).unwrap();let target=s.root.join("do-not-touch");fs::write(&target,"original").unwrap();symlink(&target,&log).unwrap();
    s.req("/api/exec/policy",json!({"chat":"a","mode":"free"}));let (code,_)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"workspace_info"}}));assert_eq!(code,200);
    let (_,degraded)=s.req("/api/actions/events",json!({"chat":"a"}));assert_eq!(degraded["persistence"],"degraded");assert_eq!(degraded["volatile"],true);assert_eq!(fs::read_to_string(target).unwrap(),"original");
}

#[test]
fn encrypted_usage_endpoint_authenticates_and_validates_without_plaintext_fallback() {
    let s=Server::start(true);let origin=format!("http://{}",s.address);
    let body=json!({"chat":"a","calls":[{"input":10,"output":2,"total":12,"cached":null}],"truncated":false});
    assert_eq!(s.post("/api/actions/usage",body.clone(),"bad",Some(&origin)).0,403);
    assert_eq!(s.post("/api/actions/usage",body.clone(),TOKEN,Some("http://evil.test")).0,403);
    let (code,r)=s.req("/api/actions/usage",body);assert_eq!(code,200);assert_eq!(r["encryptedDetails"],"disabled");assert!(!s.root.join(".isa/web-audit/sealed.jsonl").exists());
    for invalid in [json!({"chat":"a","calls":[],"truncated":false}),json!({"chat":"a","calls":[{"input":-1,"output":0,"total":0,"cached":null}],"truncated":false}),json!({"chat":"a","calls":[{"input":1,"output":1,"total":2,"cached":null,"token":"bad"}],"truncated":false}),json!({"chat":"a","calls":[{"input":9007199254740992u64,"output":0,"total":0,"cached":null}],"truncated":false})] {assert_eq!(s.req("/api/actions/usage",invalid).0,400);}
}

#[test]
fn undo_approval_rejects_replaced_target_and_changed_undo_record() {
    let s=Server::start(true);assert_eq!(s.req("/api/exec/policy",json!({"chat":"a","mode":"risk"})).0,200);
    let path=s.root.join("guarded.txt");fs::write(&path,"old").unwrap();
    let proposal="File: guarded.txt\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE";
    let (code,p)=s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit","proposal":proposal}}));assert_eq!(code,200);
    let (code,r)=s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true}));assert_eq!(code,200);let id=r["result"]["undoId"].as_str().unwrap();
    let prepare=||s.req("/api/actions/prepare",json!({"chat":"a","action":{"type":"edit_undo","undo_id":id}}));
    let (code,p)=prepare();assert_eq!(code,200);let replacement=s.root.join("replacement");fs::write(&replacement,"new").unwrap();fs::rename(&replacement,&path).unwrap();
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);assert_eq!(fs::read_to_string(&path).unwrap(),"new");
    let (code,p)=prepare();assert_eq!(code,200);let record_path=s.root.join(".isa/web-undo").join(id);
    let mut record:serde_json::Value=serde_json::from_slice(&fs::read(&record_path).unwrap()).unwrap();record["undo"]["before"]=json!("unexpected");fs::write(&record_path,serde_json::to_vec(&record).unwrap()).unwrap();
    assert_eq!(s.req("/api/actions/run",json!({"chat":"a","id":p["id"],"approved":true})).0,409);assert_eq!(fs::read_to_string(&path).unwrap(),"new");
}
