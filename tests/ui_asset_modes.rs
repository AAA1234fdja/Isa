//! Exercise both asset modes over HTTP using the real server binary.
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};

struct Server {
    child: Child,
    root: PathBuf,
    address: SocketAddr,
}

impl Server {
    fn start(mode: Option<&str>) -> Self {
        let root = std::env::temp_dir().join(format!(
            "isa-asset-http-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("assets")).unwrap();
        fs::write(root.join("index.html"), "development-only html").unwrap();
        fs::write(root.join("assets/layout.js"), "development-only script").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_isa"));
        command
            .current_dir(&root)
            .env("ISA_ADDRESS", "127.0.0.1:0")
            .env_remove("ISA_ASSET_MODE")
            .env_remove("ISA_WEB_EXEC_TOKEN")
            .env_remove("ISA_WEB_EXEC_WORKSPACE")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(mode) = mode {
            command.env("ISA_ASSET_MODE", mode);
        }
        let mut child = command.spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
            let _ = sender.send(result);
        });
        let startup = receiver.recv_timeout(Duration::from_secs(15));
        let line = match startup {
            Ok(Ok(line)) => line,
            other => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_dir_all(&root);
                panic!("server did not announce a listening address: {other:?}");
            }
        };
        let address = line
            .trim()
            .strip_prefix("ISA preview listening on http://")
            .expect("unexpected startup message")
            .parse::<SocketAddr>()
            .expect("invalid listening address");
        assert!(address.ip().is_loopback());
        assert_ne!(address.port(), 0);
        Self {
            child,
            root,
            address,
        }
    }

    fn get(&self, path: &str) -> String {
        let mut stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(5)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            self.address
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        response.split_once("\r\n\r\n").unwrap().1.to_owned()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn development_server_keeps_serving_live_files() {
    let server = Server::start(None);
    assert_eq!(server.get("/"), "development-only html");
    assert_eq!(server.get("/assets/layout.js"), "development-only script");
    fs::write(server.root.join("assets/layout.js"), "updated live script").unwrap();
    assert_eq!(server.get("/assets/layout.js"), "updated live script");
}

#[test]
fn embedded_mode_ignores_files_in_the_working_directory() {
    let server = Server::start(Some("embedded"));
    assert_eq!(server.get("/"), include_str!("../index.html"));
    assert_eq!(
        server.get("/assets/layout.js"),
        include_str!("../assets/layout.js")
    );
}
