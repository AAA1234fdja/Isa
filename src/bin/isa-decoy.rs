#[path = "../http.rs"]
mod http;

use serde_json::{json, Value};
use std::io;
use std::net::{TcpListener, TcpStream};

fn handle(mut stream: TcpStream) -> io::Result<()> {
    let request = match http::read_request(&mut stream) {
        Ok(request) => request,
        Err(_) => return http::respond(&mut stream, "400 Bad Request", "application/json", br#"{"error":"Invalid request"}"#, false),
    };
    if request.method != "POST" || request.target != "/v1/chat" {
        return http::respond(&mut stream, "404 Not Found", "application/json", br#"{"error":"Not found"}"#, false);
    }
    if !request.header("content-type").unwrap_or("").starts_with("application/json") {
        return http::respond(&mut stream, "415 Unsupported Media Type", "application/json", br#"{"error":"Expected JSON"}"#, false);
    }
    let value: Value = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(_) => return http::respond(&mut stream, "400 Bad Request", "application/json", br#"{"error":"Invalid JSON"}"#, false),
    };
    let message = value.get("message").and_then(Value::as_str).unwrap_or("");
    if message.trim().is_empty() || message.len() > 64 * 1024 {
        return http::respond(&mut stream, "400 Bad Request", "application/json", br#"{"error":"Message must be 1 to 65536 bytes"}"#, false);
    }
    let response = json!({"provider":"decoy","role":"assistant","error":{"code":"no_provider","message":"No AI provider is configured. Connect a provider to continue."}});
    let body = serde_json::to_vec(&response).map_err(io::Error::other)?;
    http::respond(&mut stream, "503 Service Unavailable", "application/json; charset=utf-8", &body, false)
}

fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:4180")?;
    println!("Decoy provider listening on 127.0.0.1:4180");
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => { std::thread::spawn(move || { if let Err(error) = handle(stream) { eprintln!("Request failed: {error}"); } }); }
            Err(error) => eprintln!("Connection failed: {error}"),
        }
    }
    Ok(())
}
