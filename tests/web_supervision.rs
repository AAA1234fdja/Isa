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
    fn start(enabled: bool) -> Self {
        let _guard=START_LOCK.lock().unwrap();
        let root=std::env::temp_dir().join(format!("isa-supervision-{}-{enabled}",std::process::id())); fs::create_dir_all(&root).unwrap();
        let socket=TcpListener::bind("127.0.0.1:0").unwrap(); let address=socket.local_addr().unwrap().to_string(); drop(socket);
        let mut command=Command::new(env!("CARGO_BIN_EXE_isa")); command.current_dir(&root).env("ISA_ADDRESS",&address).env_remove("ISA_WEB_EXEC_TOKEN").env_remove("ISA_WEB_EXEC_WORKSPACE").stdout(Stdio::null()).stderr(Stdio::null());
        if enabled { command.env("ISA_WEB_EXEC_TOKEN",TOKEN).env("ISA_WEB_EXEC_WORKSPACE",&root); }
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
