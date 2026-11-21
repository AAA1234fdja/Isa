#![cfg(windows)]

use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const TOKEN: &str = "test-only-windows-capability-0123456789";
struct Server {
    child: Child,
    root: std::path::PathBuf,
    address: String,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn windows_supervisor_rejects_even_authorized_requests() {
    let root = std::env::temp_dir().join(format!("isa-windows-supervision-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap().to_string();
    drop(socket);
    let child = Command::new(env!("CARGO_BIN_EXE_isa"))
        .current_dir(&root)
        .env("ISA_ADDRESS", &address)
        .env("ISA_WEB_EXEC_TOKEN", TOKEN)
        .env("ISA_WEB_EXEC_WORKSPACE", &root)
        .env_remove("ISA_WEB_EXEC_PROFILE")
        .env_remove("ISA_WEB_AUDIT_ENCRYPTED")
        .env_remove("ISA_WEB_WINDOWS_READ_ONLY")
        .env_remove("ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Server {
        child,
        root,
        address,
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(
            server.child.try_wait().unwrap().is_none(),
            "server exited during startup"
        );
        if TcpStream::connect(&server.address).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "server did not start");
        std::thread::sleep(Duration::from_millis(25));
    }
    // The public hint must agree with the server-side platform gate, even when
    // a token and workspace are configured. It is not an authorization check.
    let mut options = TcpStream::connect(&server.address).unwrap();
    options
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(options, "POST /api/chat-options HTTP/1.1\r\nHost: {0}\r\nOrigin: http://{0}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}", server.address).unwrap();
    let mut response = String::new();
    options.read_to_string(&mut response).unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    assert_eq!(
        head.lines().next().unwrap().split_whitespace().nth(1),
        Some("200")
    );
    let capabilities: serde_json::Value = serde_json::from_str(body).unwrap();
    assert_eq!(capabilities["localActionsPlatformSupported"], false);
    for path in [
        "/api/exec/policy",
        "/api/exec/prepare",
        "/api/exec/run",
        "/api/actions/prepare",
        "/api/actions/run",
        "/api/actions/events",
        "/api/actions/usage",
        "/api/actions/sessions/list",
    ] {
        let mut stream = TcpStream::connect(&server.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let body = "{}";
        write!(stream, "POST {path} HTTP/1.1\r\nHost: {0}\r\nOrigin: http://{0}\r\nX-ISA-Exec-Token: {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {1}\r\nConnection: close\r\n\r\n{body}", server.address, body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert_eq!(
            response.lines().next().unwrap().split_whitespace().nth(1),
            Some("403"),
            "{path}: {response}"
        );
    }
    assert!(!server.root.join(".isa").exists());
}

impl Server {
    fn post(&self, path: &str, body: serde_json::Value, token: &str) -> (u16, serde_json::Value) {
        let mut stream = TcpStream::connect(&self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(40)))
            .unwrap();
        let body = body.to_string();
        write!(stream, "POST {path} HTTP/1.1\r\nHost: {0}\r\nOrigin: http://{0}\r\nX-ISA-Exec-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {1}\r\nConnection: close\r\n\r\n{body}", self.address, body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        (
            head.lines()
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap()
                .parse()
                .unwrap(),
            serde_json::from_str(body).unwrap(),
        )
    }
}

#[test]
fn windows_read_only_supervision_is_opt_in_and_rejects_other_actions() {
    use serde_json::json;
    let root = std::env::temp_dir().join(format!(
        "isa-windows-read-only-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap().to_string();
    drop(socket);
    let child = Command::new(env!("CARGO_BIN_EXE_isa"))
        .current_dir(&root)
        .env("ISA_ADDRESS", &address)
        .env("ISA_WEB_EXEC_TOKEN", TOKEN)
        .env("ISA_WEB_EXEC_WORKSPACE", &root)
        .env("ISA_WEB_WINDOWS_READ_ONLY", "1")
        .env_remove("ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL")
        .env_remove("ISA_WEB_EXEC_PROFILE")
        .env_remove("ISA_WEB_AUDIT_ENCRYPTED")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Server {
        child,
        root,
        address,
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(server.child.try_wait().unwrap().is_none());
        if TcpStream::connect(&server.address).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    let (code, options) = server.post("/api/chat-options", json!({}), "");
    assert_eq!(code, 200);
    assert_eq!(options["localActionsWindowsReadOnly"], true);
    assert_eq!(options["localActionsPlatformSupported"], false);
    let policy = json!({"chat":"a","mode":"always"});
    assert_eq!(
        server.post("/api/exec/policy", policy.clone(), "wrong").0,
        403
    );
    assert_eq!(server.post("/api/exec/policy", policy, TOKEN).0, 200);
    for action in [
        json!({"type":"shell","command":"echo unsafe"}),
        json!({"type":"edit","proposal":"x"}),
        json!({"type":"edit_undo","undo_id":"a".repeat(48)}),
        json!({"type":"mcp_start"}),
        json!({"type":"work_start"}),
    ] {
        assert_eq!(
            server
                .post(
                    "/api/actions/prepare",
                    json!({"chat":"a","action":action}),
                    TOKEN
                )
                .0,
            403
        );
    }
    assert_eq!(
        server
            .post(
                "/api/exec/prepare",
                json!({"chat":"a","command":"echo unsafe"}),
                TOKEN
            )
            .0,
        403
    );
    let prepare = || {
        server.post(
            "/api/actions/prepare",
            json!({"chat":"a","action":{"type":"workspace_info"}}),
            TOKEN,
        )
    };
    let (code, pending) = prepare();
    assert_eq!(code, 200);
    assert_eq!(pending["requiresApproval"], true);
    let id = pending["id"].as_str().unwrap();
    assert_eq!(
        server
            .post(
                "/api/actions/run",
                json!({"chat":"a","id":id,"approved":false}),
                TOKEN
            )
            .0,
        403
    );
    assert_eq!(
        server
            .post(
                "/api/actions/run",
                json!({"chat":"a","id":id,"approved":true}),
                TOKEN
            )
            .0,
        409
    );
    let (_, pending) = prepare();
    let id = pending["id"].as_str().unwrap();
    let (code, result) = server.post(
        "/api/actions/run",
        json!({"chat":"a","id":id,"approved":true}),
        TOKEN,
    );
    assert_eq!(code, 200);
    assert_eq!(result["result"]["isDirectory"], true);
    assert_eq!(
        server
            .post(
                "/api/actions/run",
                json!({"chat":"a","id":id,"approved":true}),
                TOKEN
            )
            .0,
        409
    );
    let (code, events) = server.post("/api/actions/events", json!({"chat":"a"}), TOKEN);
    assert_eq!(code, 200);
    assert_eq!(events["volatile"], true);
    assert_eq!(events["persistence"], "degraded");
    assert!(!server.root.join(".isa").exists());
}

#[test]
fn windows_experimental_shell_respects_policy_denial_and_replay() {
    use serde_json::json;
    let root = std::env::temp_dir().join(format!(
        "isa-windows-shell-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap().to_string();
    drop(socket);
    let child = Command::new(env!("CARGO_BIN_EXE_isa"))
        .current_dir(&root)
        .env("ISA_ADDRESS", &address)
        .env("ISA_WEB_EXEC_TOKEN", TOKEN)
        .env("ISA_WEB_EXEC_WORKSPACE", &root)
        .env("ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL", "1")
        .env_remove("ISA_WEB_WINDOWS_READ_ONLY")
        .env_remove("ISA_WEB_EXEC_PROFILE")
        .env_remove("ISA_WEB_AUDIT_ENCRYPTED")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Server {
        child,
        root,
        address,
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(server.child.try_wait().unwrap().is_none());
        if TcpStream::connect(&server.address).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    let (_, options) = server.post("/api/chat-options", json!({}), "");
    assert_eq!(options["localActionsWindowsShellExperimental"], true);
    assert_eq!(
        server
            .post(
                "/api/exec/policy",
                json!({"chat":"a","mode":"always"}),
                "wrong"
            )
            .0,
        403
    );
    assert_eq!(
        server
            .post(
                "/api/exec/policy",
                json!({"chat":"a","mode":"always"}),
                TOKEN
            )
            .0,
        200
    );
    let command =
        "[Console]::Out.WriteLine('OK'); Set-Content -LiteralPath 'marker.txt' -Value done";
    assert_eq!(
        server
            .post(
                "/api/actions/prepare",
                json!({"chat":"a","action":{"type":"shell","command":command}}),
                TOKEN
            )
            .0,
        403
    );
    assert_eq!(
        server
            .post(
                "/api/actions/prepare",
                json!({"chat":"a","action":{"type":"edit","proposal":"x"}}),
                TOKEN
            )
            .0,
        403
    );
    let prepare = || {
        server.post(
            "/api/exec/prepare",
            json!({"chat":"a","command":command}),
            TOKEN,
        )
    };
    let (status, p) = prepare();
    assert_eq!(status, 200);
    assert_eq!(p["shellDialect"], "powershell");
    assert_eq!(p["requiresApproval"], true);
    let id = p["id"].as_str().unwrap();
    assert_eq!(
        server
            .post(
                "/api/exec/run",
                json!({"chat":"a","id":id,"approved":false}),
                TOKEN
            )
            .0,
        403
    );
    assert!(!server.root.join("marker.txt").exists());
    assert_eq!(
        server
            .post(
                "/api/exec/run",
                json!({"chat":"a","id":id,"approved":true}),
                TOKEN
            )
            .0,
        409
    );
    let (_, p) = prepare();
    let id = p["id"].as_str().unwrap();
    let (status, result) = server.post(
        "/api/exec/run",
        json!({"chat":"a","id":id,"approved":true}),
        TOKEN,
    );
    assert_eq!(status, 200);
    assert_eq!(result["result"]["exitCode"], 0);
    assert!(result["result"]["stdout"].as_str().unwrap().contains("OK"));
    assert!(server.root.join("marker.txt").exists());
    assert_eq!(
        server
            .post(
                "/api/exec/run",
                json!({"chat":"a","id":id,"approved":true}),
                TOKEN
            )
            .0,
        409
    );
    let (_, history) = server.post("/api/actions/events", json!({"chat":"a"}), TOKEN);
    assert_eq!(history["persistence"], "degraded");
    assert_eq!(history["volatile"], true);
}

#[test]
fn windows_experimental_shell_stop_is_scoped_and_reports_request() {
    use serde_json::json;
    let root = std::env::temp_dir().join(format!(
        "isa-win-stop-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap().to_string();
    drop(socket);
    let child = Command::new(env!("CARGO_BIN_EXE_isa"))
        .current_dir(&root)
        .env("ISA_ADDRESS", &address)
        .env("ISA_WEB_EXEC_TOKEN", TOKEN)
        .env("ISA_WEB_EXEC_WORKSPACE", &root)
        .env("ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL", "1")
        .env_remove("ISA_WEB_EXEC_PROFILE")
        .env_remove("ISA_WEB_AUDIT_ENCRYPTED")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Server {
        child,
        root,
        address,
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(server.child.try_wait().unwrap().is_none());
        if TcpStream::connect(&server.address).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        server
            .post(
                "/api/exec/policy",
                json!({"chat":"a","mode":"always"}),
                TOKEN
            )
            .0,
        200
    );
    let (_,prepared)=server.post("/api/exec/prepare",json!({"chat":"a","command":"Start-Sleep -Seconds 25; Set-Content -LiteralPath 'finished.txt' -Value done"}),TOKEN);
    let id = prepared["id"].as_str().unwrap().to_string();
    let (_, wrong) = server.post("/api/actions/stop", json!({"chat":"other","id":id}), TOKEN);
    assert_eq!(wrong["requested"], false);
    std::thread::scope(|scope| {
        let running = scope.spawn(|| {
            server.post(
                "/api/exec/run",
                json!({"chat":"a","id":id,"approved":true}),
                TOKEN,
            )
        });
        let until = Instant::now() + Duration::from_secs(5);
        let mut requested = false;
        while Instant::now() < until {
            let (status, response) =
                server.post("/api/actions/stop", json!({"chat":"a","id":id}), TOKEN);
            assert_eq!(status, 200);
            if response["requested"] == true {
                requested = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(requested, "stop was not registered");
        let (status, response) = running.join().unwrap();
        // Stop can consume a pending action, or cancel an already running one.
        assert!(
            status == 409
                || status == 500
                || (status == 200 && response["result"]["cancelled"] == true),
            "unexpected stop result: {status} {response}"
        );
    });
    assert!(!server.root.join("finished.txt").exists());
}

#[test]
fn windows_experimental_encrypted_audit_survives_restart_and_degrades_without_key() {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    let root = std::env::temp_dir().join(format!(
        "isa-win-audit-http-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let canonical = root.canonicalize().unwrap();
    let account = format!(
        "workspace-{:x}",
        Sha256::digest(canonical.to_str().unwrap().as_bytes())
    );
    let entry = keyring::Entry::new("dev.isa.web-audit", &account).unwrap();
    assert!(matches!(entry.get_password(), Err(keyring::Error::NoEntry)));
    struct Cleanup(keyring::Entry);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = self.0.delete_credential();
        }
    }
    let _credential_cleanup = Cleanup(keyring::Entry::new("dev.isa.web-audit", &account).unwrap());
    let mut key = [0u8; 32];
    getrandom::getrandom(&mut key).unwrap();
    entry.set_password(&STANDARD.encode(key)).unwrap();
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap().to_string();
    drop(socket);
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_isa"))
            .current_dir(&root)
            .env("ISA_ADDRESS", &address)
            .env("ISA_WEB_EXEC_TOKEN", TOKEN)
            .env("ISA_WEB_EXEC_WORKSPACE", &root)
            .env("ISA_WEB_WINDOWS_READ_ONLY", "1")
            .env("ISA_WEB_AUDIT_ENCRYPTED", "1")
            .env("ISA_WEB_WINDOWS_AUDIT_EXPERIMENTAL", "1")
            .env_remove("ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL")
            .env_remove("ISA_WEB_EXEC_PROFILE")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    };
    let child = spawn();
    let mut server = Server {
        child,
        root: root.clone(),
        address: address.clone(),
    };
    fn wait(server: &mut Server) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            assert!(server.child.try_wait().unwrap().is_none());
            if TcpStream::connect(&server.address).is_ok() {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    wait(&mut server);
    assert_eq!(
        server
            .post(
                "/api/exec/policy",
                json!({"chat":"a","mode":"always"}),
                TOKEN
            )
            .0,
        200
    );
    let (status, p) = server.post(
        "/api/actions/prepare",
        json!({"chat":"a","action":{"type":"workspace_info"}}),
        TOKEN,
    );
    assert_eq!(status, 200);
    assert_eq!(
        server
            .post(
                "/api/actions/run",
                json!({"chat":"a","id":p["id"],"approved":true}),
                TOKEN
            )
            .0,
        200
    );
    let (status,usage)=server.post("/api/actions/usage",json!({"chat":"a","calls":[{"input":12,"output":4,"total":16,"cached":null}],"truncated":false}),TOKEN);
    assert_eq!(status, 200);
    assert_eq!(usage["encryptedDetails"], "best_effort");
    assert_eq!(usage["encryptedDetailsExperimental"], true);
    let stored = fs::read(root.join(".isa/web-audit/sealed.jsonl")).unwrap();
    assert!(!stored
        .windows("workspace_info".len())
        .any(|v| v == b"workspace_info"));
    server.child.kill().unwrap();
    server.child.wait().unwrap();
    server.child = spawn();
    wait(&mut server);
    let (status, history) = server.post("/api/actions/events", json!({"chat":"a"}), TOKEN);
    assert_eq!(status, 200);
    assert_eq!(history["persistence"], "degraded");
    assert_eq!(history["encryptedDetails"], "best_effort");
    assert_eq!(history["encryptedDetailsExperimental"], true);
    assert!(history["details"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["kind"] == "usage"));
    let log = root.join(".isa/web-audit/sealed.jsonl");
    let contents = fs::read_to_string(&log).unwrap();
    let mut lines: Vec<String> = contents.lines().map(str::to_owned).collect();
    let mut envelope: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    let mut ciphertext = envelope["ciphertext"].as_str().unwrap().to_owned();
    let replacement = if ciphertext.starts_with('A') {
        "B"
    } else {
        "A"
    };
    ciphertext.replace_range(0..1, replacement);
    envelope["ciphertext"] = json!(ciphertext);
    lines[0] = envelope.to_string();
    fs::write(&log, format!("{}\n", lines.join("\n"))).unwrap();
    let damaged = fs::read(&log).unwrap();
    let (status,usage)=server.post("/api/actions/usage",json!({"chat":"a","calls":[{"input":1,"output":1,"total":2,"cached":null}],"truncated":false}),TOKEN);
    assert_eq!(status, 200);
    assert_eq!(usage["encryptedDetails"], "degraded");
    assert_eq!(fs::read(&log).unwrap(), damaged);
    entry.delete_credential().unwrap();
    let (status,usage)=server.post("/api/actions/usage",json!({"chat":"a","calls":[{"input":1,"output":1,"total":2,"cached":null}],"truncated":false}),TOKEN);
    assert_eq!(status, 200);
    assert_eq!(usage["encryptedDetails"], "degraded");
    assert_eq!(
        server
            .post(
                "/api/exec/policy",
                json!({"chat":"a","mode":"always"}),
                TOKEN
            )
            .0,
        200
    );
    let (status, p) = server.post(
        "/api/actions/prepare",
        json!({"chat":"a","action":{"type":"workspace_info"}}),
        TOKEN,
    );
    assert_eq!(status, 200);
    assert_eq!(p["auditWarning"], "both_incomplete");
    let (status, result) = server.post(
        "/api/actions/run",
        json!({"chat":"a","id":p["id"],"approved":true}),
        TOKEN,
    );
    assert_eq!(status, 200);
    assert_eq!(result["auditWarning"], "both_incomplete");
}

#[test]
fn windows_sandbox_profile_never_falls_back_to_host_shell() {
    use serde_json::json;
    let root = std::env::temp_dir().join(format!(
        "isa-windows-sandbox-denied-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    drop(listener);
    let child = Command::new(env!("CARGO_BIN_EXE_isa"))
        .current_dir(&root)
        .env("ISA_ADDRESS", &address)
        .env("ISA_WEB_EXEC_TOKEN", TOKEN)
        .env("ISA_WEB_EXEC_WORKSPACE", &root)
        .env("ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL", "1")
        .env("ISA_WEB_EXEC_PROFILE", "sandboxed")
        .env_remove("ISA_WEB_AUDIT_ENCRYPTED")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Server {
        child,
        root,
        address,
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(server.child.try_wait().unwrap().is_none());
        if TcpStream::connect(&server.address).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(25));
    }
    let (_, options) = server.post("/api/chat-options", json!({}), "");
    assert_eq!(options["windowsSandboxAvailable"], false);
    for (path, body) in [
        ("/api/exec/policy", json!({"chat":"a","mode":"never"})),
        (
            "/api/exec/prepare",
            json!({"chat":"a","command":"'unsafe' | Set-Content host-marker.txt"}),
        ),
        (
            "/api/exec/run",
            json!({"chat":"a","id":"invalid","approved":true}),
        ),
    ] {
        assert_eq!(server.post(path, body, TOKEN).0, 503, "{path}");
    }
    assert!(!server.root.join("host-marker.txt").exists());
}
