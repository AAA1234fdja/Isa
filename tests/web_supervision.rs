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
    fn configured(enabled: bool, mcp: bool) -> Self {
        let _guard=START_LOCK.lock().unwrap();
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root=std::env::temp_dir().join(format!("isa-supervision-{}-{}",std::process::id(), NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed))); fs::create_dir_all(&root).unwrap();
        let socket=TcpListener::bind("127.0.0.1:0").unwrap(); let address=socket.local_addr().unwrap().to_string(); drop(socket);
        let mut command=Command::new(env!("CARGO_BIN_EXE_isa")); command.current_dir(&root).env("ISA_ADDRESS",&address).env_remove("ISA_WEB_EXEC_TOKEN").env_remove("ISA_WEB_EXEC_WORKSPACE").stdout(Stdio::null()).stderr(Stdio::null());
        if enabled { command.env("ISA_WEB_EXEC_TOKEN",TOKEN).env("ISA_WEB_EXEC_WORKSPACE",&root); }
        command.env_remove("ISA_WEB_MCP_COMMAND");
        if mcp { let fixture=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp-server.cjs"); let marker=root.join("marker"); command.env("ISA_WEB_MCP_COMMAND",json!(["node",fixture.to_string_lossy(),"valid",marker.to_string_lossy()]).to_string()); }
        let child=command.spawn().unwrap(); let mut server=Self { child,address,root };
        for _ in 0..200 { if server.child.try_wait().unwrap().is_some() { panic!("server exited"); } if TcpStream::connect(&server.address).is_ok() { return server; } std::thread::sleep(Duration::from_millis(25)); } panic!("server failed to start");
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
    let (code,r)=s.req("/api/actions/pool/start",json!({"chat":"pool-chat","agents":["agent-a","agent-b"]}));assert_eq!(code,200);let run=r["run"].as_str().unwrap();assert_eq!(r["maxCalls"],3);
    let p=|agent:&str, action:Value| s.req("/api/actions/prepare",json!({"chat":"pool-chat","poolRun":run,"agent":agent,"action":action}));
    assert_eq!(p("other",json!({"type":"mcp_start"})).0,403);
    assert_eq!(p("agent-a",json!({"type":"workspace_info"})).0,400);
    for i in 0..3 {
        let agent=if i%2==0 {"agent-a"} else {"agent-b"};
        let (code,pending)=p(agent,json!({"type":"mcp_start"}));assert_eq!(code,200);assert_eq!(pending["requiresApproval"],true);assert_eq!(pending["agent"],agent);
        let (code,result)=s.req("/api/actions/run",json!({"chat":"pool-chat","id":pending["id"],"approved":true}));assert_eq!(code,200);let session=result["result"]["session"].as_str().unwrap();
        let call=json!({"type":"mcp_call","session":session,"name":"echo","arguments":{"i":i}});
        assert_eq!(s.req("/api/actions/prepare",json!({"chat":"pool-chat","action":call})).0,403);
        let (code,pending)=p(agent,call);assert_eq!(code,200);assert_eq!(pending["requiresApproval"],true);
        let (code,_)=s.req("/api/actions/run",json!({"chat":"pool-chat","id":pending["id"],"approved":true}));assert_eq!(code,200);
    }
    let (_,pending)=p("agent-b",json!({"type":"mcp_start"}));assert_eq!(s.req("/api/actions/run",json!({"chat":"pool-chat","id":pending["id"],"approved":true})).0,429);
    assert_eq!(s.req("/api/actions/pool/close",json!({"chat":"pool-chat","run":run})).0,200);
    assert_eq!(p("agent-a",json!({"type":"mcp_start"})).0,409);
}
