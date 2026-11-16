use std::env;
mod http;
#[allow(dead_code)]
mod cli;

use std::io;
use std::net::{TcpListener, TcpStream};

// Embedded assets are a fallback when running outside the project directory.
const INTERFACE: &str = include_str!("../index.html");
const LOGO: &[u8] = include_bytes!("../assets/isa-logo.png");
const TRANSPARENT_LOGO: &[u8] = include_bytes!("../assets/isa-logo-transparent.png");

// Decode query parameters without treating user input as a URL or a filesystem path.
fn decode_query(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => output.push(b' '),
            b'%' => {
                if i + 2 >= bytes.len() { return None; }
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
                output.push(u8::from_str_radix(hex, 16).ok()?);
                i += 2;
            }
            byte => output.push(byte),
        }
        i += 1;
    }
    String::from_utf8(output).ok()
}

fn search_extensions(query: &str) -> Result<Vec<u8>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("ISA-preview/0.1")
        .build().map_err(|e| e.to_string())?;
    let response = client.get("https://open-vsx.org/api/-/search")
        .query(&[("query", query), ("size", "20")])
        .send().and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
    let data: serde_json::Value = response.json().map_err(|e| e.to_string())?;
    serde_json::to_vec(&data).map_err(|e| e.to_string())
}

fn web_chat(body: &[u8], model: &str) -> Result<Vec<u8>, &'static str> {
    let value: serde_json::Value = serde_json::from_slice(body).map_err(|_| "Invalid request")?;
    let entries = value.get("messages").and_then(|v| v.as_array()).ok_or("Messages are required")?;
    if entries.is_empty() || entries.len() > 20 { return Err("Expected 1 to 20 messages"); }
    let mut messages = Vec::new();
    let mut total = 0usize;
    for entry in entries {
        let role = entry.get("role").and_then(|v| v.as_str()).ok_or("Invalid role")?;
        let text = entry.get("text").and_then(|v| v.as_str()).ok_or("Invalid message")?;
        if !matches!(role, "user" | "assistant") || text.trim().is_empty() || text.len() > 64 * 1024 { return Err("Invalid message"); }
        total += text.len();
        if total > 96 * 1024 { return Err("Conversation exceeds context limit"); }
        messages.push((role.to_owned(), text.to_owned()));
    }
    if messages.last().map(|m| m.0.as_str()) != Some("user") { return Err("Last message must be from the user"); }
    let root = std::env::current_dir().map_err(|_| "Workspace unavailable")?;
    let reply = cli::web_response(&root, model, &messages).map_err(|error| { eprintln!("Local model request failed: {error}"); "Local model or harness request failed. Check server logs and configuration." })?;
    if reply.len() > 65536 { return Err("Model response exceeded the limit"); }
    serde_json::to_vec(&serde_json::json!({"provider":"ollama","role":"assistant","content":reply})).map_err(|_| "Could not encode response")
}

fn handle_request(mut stream: TcpStream) -> io::Result<()> {
    let request = match http::read_request(&mut stream) {
        Ok(request) => request,
        Err(_) => return http::respond(&mut stream, "400 Bad Request", "application/json", br#"{"error":"Invalid request"}"#, false),
    };
    let method = request.method.as_str();
    let target = request.target.as_str();
    let (path, query_string) = target.split_once('?').unwrap_or((target, ""));

    // Read the development assets on each request so the browser can detect edits.
    // Only explicit paths are served; arbitrary filesystem access is not allowed.
    let (status, content_type, body): (&str, &str, Vec<u8>) = match (method, path) {
        ("POST", "/api/chat") if env::var("ISA_WEB_MODEL").ok().is_some_and(|s| !s.trim().is_empty()) => {
            if !request.is_same_origin_json() {
                ("403 Forbidden", "application/json", b"{\"error\":\"Same-origin JSON request required\"}".to_vec())
            } else {
                let model = env::var("ISA_WEB_MODEL").unwrap_or_default();
                match web_chat(&request.body, &model) {
                    Ok(body) => ("200 OK", "application/json; charset=utf-8", body),
                    Err(message) => ("502 Bad Gateway", "application/json; charset=utf-8", serde_json::to_vec(&serde_json::json!({"provider":"ollama","role":"assistant","error":{"message":message}})).unwrap_or_default()),
                }
            }
        },
        ("POST", "/api/chat" | "/api/decoy") => {
            if !request.is_same_origin_json() {
                ("403 Forbidden", "application/json", b"{\"error\":\"Same-origin JSON request required\"}".to_vec())
            } else {
                let client = reqwest::blocking::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(std::time::Duration::from_secs(5)).build();
                let result = client.and_then(|client| {
                    let response = client.post("http://127.0.0.1:4180/v1/chat").header("Content-Type", "application/json").body(request.body.clone()).send()?;
                    let status = response.status().as_u16();
                    Ok((status, response.bytes()?))
                });
                match result {
                    Ok((code, body)) if body.len() <= 128 * 1024 => {
                        let status = match code { 200 => "200 OK", 400 => "400 Bad Request", 503 => "503 Service Unavailable", _ => "502 Bad Gateway" };
                        (status, "application/json; charset=utf-8", body.to_vec())
                    }
                    _ => ("502 Bad Gateway", "application/json", b"{\"error\":\"Decoy provider unavailable\"}".to_vec()),
                }
            }
        },
        ("GET", "/api/extensions") => {
            let raw = query_string.split('&').find_map(|part| part.strip_prefix("q=")).unwrap_or("");
            match decode_query(raw) {
                Some(query) if query.len() <= 200 => match search_extensions(&query) {
                    Ok(data) => ("200 OK", "application/json; charset=utf-8", data),
                    Err(error) => {
                        eprintln!("Open VSX request failed: {error}");
                        ("502 Bad Gateway", "application/json; charset=utf-8", b"{\"error\":\"Extension catalog unavailable\"}".to_vec())
                    }
                },
                _ => ("400 Bad Request", "application/json; charset=utf-8", b"{\"error\":\"Invalid query\"}".to_vec()),
            }
        },
        ("GET" | "HEAD", "/" | "/index.html") => (
            "200 OK", "text/html; charset=utf-8",
            std::fs::read("index.html").unwrap_or_else(|_| INTERFACE.as_bytes().to_vec()),
        ),
        ("GET" | "HEAD", "/assets/providers/github.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/github.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/google-drive.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/google-drive.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/google-docs.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/google-docs.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/google-sheets.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/google-sheets.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/google-slides.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/google-slides.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/notion.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/notion.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/linear.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/linear.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/gmail.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/gmail.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/supabase.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/supabase.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/vercel.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/vercel.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/dropbox.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/dropbox.svg").to_vec()),
        ("GET" | "HEAD", "/assets/providers/google-calendar.svg") => ("200 OK", "image/svg+xml", include_bytes!("../assets/providers/google-calendar.svg").to_vec()),
        ("GET" | "HEAD", "/assets/isa-logo-transparent.png") => (
            "200 OK", "image/png", std::fs::read("assets/isa-logo-transparent.png").unwrap_or_else(|_| TRANSPARENT_LOGO.to_vec()),
        ),
        ("GET" | "HEAD", "/assets/isa-logo.png") => (
            "200 OK", "image/png",
            std::fs::read("assets/isa-logo.png").unwrap_or_else(|_| LOGO.to_vec()),
        ),
        ("GET" | "HEAD", _) => ("404 Not Found", "text/plain; charset=utf-8", b"Not found".to_vec()),
        _ => ("405 Method Not Allowed", "text/plain; charset=utf-8", b"Method not allowed".to_vec()),
    };
    http::respond(&mut stream, status, content_type, &body, method == "HEAD")
}

fn main() -> io::Result<()> {
    let address = env::var("ISA_ADDRESS").unwrap_or_else(|_| "127.0.0.1:4174".to_owned());
    let listener = TcpListener::bind(&address)?;
    println!("ISA preview listening on http://{address}");
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                std::thread::spawn(move || {
                    if let Err(error) = handle_request(stream) {
                        eprintln!("Request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("Connection failed: {error}"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod web_chat_tests {
    use super::*;
    #[test]
    fn rejects_invalid_conversation_before_model_call() {
        assert!(web_chat(br#"{"messages":[{"role":"system","text":"test"}]}"#, "model").is_err());
        assert!(web_chat(br#"{"messages":[{"role":"assistant","text":"test"}]}"#, "model").is_err());
        assert!(web_chat(br#"{"messages":[]}"#, "model").is_err());
        assert!(web_chat(b"invalid", "model").is_err());
    }
}
