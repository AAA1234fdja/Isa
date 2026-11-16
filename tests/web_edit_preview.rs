use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct Server { child: Child, address: String, workspace: PathBuf }
impl Server {
    fn start(configure_workspace: bool) -> Self {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let workspace = std::env::temp_dir().join(format!("isa-web-review-{}-{id}", std::process::id()));
        fs::create_dir(&workspace).unwrap();
        let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap().to_string();
        drop(reservation);
        let mut command = Command::new(env!("CARGO_BIN_EXE_isa"));
        command.current_dir(&workspace).env("ISA_ADDRESS", &address).env_remove("ISA_WEB_AGENT_WORKSPACE").env_remove("ISA_WEB_MODEL").stdout(Stdio::null()).stderr(Stdio::null());
        if configure_workspace { command.env("ISA_WEB_AGENT_WORKSPACE", &workspace); }
        let child = command.spawn().expect("start ISA server");
        let mut server = Self { child, address, workspace };
        for _ in 0..200 {
            if server.child.try_wait().unwrap().is_some() { panic!("server exited before accepting connections"); }
            if TcpStream::connect(&server.address).is_ok() { return server; }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("server did not start");
    }
    fn request(&self, method: &str, origin: Option<&str>, content_type: &str, body: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(&self.address).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let origin_header = origin.map(|s| format!("Origin: {s}\r\n")).unwrap_or_default();
        write!(stream, "{method} /api/edit-preview HTTP/1.1\r\nHost: {}\r\n{origin_header}Content-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", self.address, body.len()).unwrap();
        let mut response = String::new(); stream.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        let status = headers.lines().next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, body.to_owned())
    }
    fn json(&self, value: serde_json::Value) -> (u16, String) {
        self.request("POST", Some(&format!("http://{}", self.address)), "application/json", &value.to_string())
    }
}
impl Drop for Server {
    fn drop(&mut self) { let _ = self.child.kill(); let _ = self.child.wait(); let _ = fs::remove_dir_all(&self.workspace); }
}
fn proposal(path: &str, search: &str, replacement: &str) -> String {
    format!("File: {path}\n<<<<<<< SEARCH\n{search}\n=======\n{replacement}\n>>>>>>> REPLACE")
}

#[test]
fn endpoint_checks_snapshot_without_writing_or_disclosing_other_content() {
    let server = Server::start(true);
    fs::write(server.workspace.join("source.txt"), "private context\nold value\nlast line\n").unwrap();
    let original = fs::read(server.workspace.join("source.txt")).unwrap();
    let patch = proposal("source.txt", "old", "new");
    let (status, body) = server.json(serde_json::json!({"proposal":patch}));
    assert_eq!(status, 200);
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["file"], "source.txt"); assert_eq!(value["line"], 2); assert_eq!(value["readOnly"], true);
    assert_eq!(value["search"], "old"); assert_eq!(value["replacement"], "new");
    assert!(!body.contains("private context"));
    assert_eq!(fs::read(server.workspace.join("source.txt")).unwrap(), original);
    assert!(!server.workspace.join(".isa").exists());
    fs::write(server.workspace.join("source.txt"), "old old").unwrap();
    let (status, _) = server.json(serde_json::json!({"proposal":patch}));
    assert_eq!(status, 422); assert_eq!(fs::read_to_string(server.workspace.join("source.txt")).unwrap(), "old old");
    assert!(!server.workspace.join(".isa").exists());
}

#[test]
fn endpoint_rejects_cross_origin_bad_requests_and_unsafe_paths() {
    let server = Server::start(true);
    fs::write(server.workspace.join("source.txt"), "old").unwrap();
    fs::write(server.workspace.join(".env"), "old secret").unwrap();
    let patch = proposal("source.txt", "old", "new");
    let body = serde_json::json!({"proposal":patch}).to_string();
    let origin = format!("http://{}", server.address);
    assert_eq!(server.request("POST", Some("https://evil.example"), "application/json", &body).0, 403);
    assert_eq!(server.request("POST", None, "application/json", &body).0, 403);
    assert_eq!(server.request("POST", Some(&origin), "text/plain", &body).0, 403);
    assert_eq!(server.request("GET", Some(&origin), "application/json", &body).0, 404);
    assert_eq!(server.request("POST", Some(&origin), "application/json", "{").0, 400);
    assert_eq!(server.json(serde_json::json!({"proposal":patch,"workspace":"/"})).0, 400);
    assert_eq!(server.json(serde_json::json!({"proposal":"x".repeat(32769)})).0, 400);
    for path in [".env", "../outside", "/etc/passwd", "missing.txt"] {
        let (status, body) = server.json(serde_json::json!({"proposal":proposal(path,"old","new")}));
        assert_eq!(status, 422); assert!(!body.contains("old secret"));
    }
    #[cfg(unix)] {
        std::os::unix::fs::symlink("source.txt", server.workspace.join("link.txt")).unwrap();
        assert_eq!(server.json(serde_json::json!({"proposal":proposal("link.txt","old","new")})).0, 422);
    }
    assert_eq!(fs::read_to_string(server.workspace.join("source.txt")).unwrap(), "old");
    assert!(!server.workspace.join(".isa").exists());
}

#[test]
fn endpoint_fails_closed_without_configured_workspace() {
    let server = Server::start(false);
    fs::write(server.workspace.join("source.txt"), "old").unwrap();
    let (status, _) = server.json(serde_json::json!({"proposal":proposal("source.txt", "old", "new")}));
    assert_eq!(status, 503);
    assert_eq!(fs::read_to_string(server.workspace.join("source.txt")).unwrap(), "old");
    assert!(!server.workspace.join(".isa").exists());
}
