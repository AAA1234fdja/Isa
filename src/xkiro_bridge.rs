//! Narrow, loopback-only bridge for xKiro chat completions. This is not a
//! general-purpose proxy and does not provide authentication against local processes.
use crate::http::{self, Request};
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

const ENDPOINT: &str = "https://api.xkiro.com/v1/chat/completions";
const MAX_RESPONSE: u64 = 8 * 1024 * 1024;

fn public_address(addr: &SocketAddr) -> bool {
    match addr.ip() {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        // Reject mapped, local, multicast and non-global IPv6 addresses.
        IpAddr::V6(ip) => {
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && s[0] != 0x2002
                && !(s[0] == 0x2001 && (s[1] == 0x0db8 || s[1] < 0x0200))
        }
    }
}

fn allowed(request: &Request, stream: &TcpStream) -> bool {
    let local = match stream.local_addr() {
        Ok(a) if a.is_ipv4() && a.ip().is_loopback() => a,
        _ => return false,
    };
    if !stream.peer_addr().is_ok_and(|a| a.ip().is_loopback())
        || !request.is_same_origin_json()
        || request.header("host") != Some(local.to_string().as_str())
        || request.header("origin") != Some(format!("http://{local}").as_str())
        || request.header("forwarded").is_some()
        || request.header("x-forwarded-for").is_some()
        || request.header("x-forwarded-host").is_some()
        || request
            .header("sec-fetch-site")
            .is_some_and(|v| v != "same-origin")
    {
        return false;
    }
    true
}

fn error(stream: &mut TcpStream, status: &str, message: &'static [u8]) -> io::Result<()> {
    http::respond(stream, status, "application/json", message, false)
}

pub(crate) fn handle(request: &Request, stream: &mut TcpStream) -> io::Result<()> {
    if request.target != "/api/providers/xkiro/chat/completions" || !allowed(request, stream) {
        return error(
            stream,
            "403 Forbidden",
            br#"{"error":"Local same-origin request required"}"#,
        );
    }
    let payload: serde_json::Value = match serde_json::from_slice(&request.body) {
        Ok(value) => value,
        Err(_) => return error(stream, "400 Bad Request", br#"{"error":"Invalid JSON"}"#),
    };
    if !payload.is_object() {
        return error(stream, "400 Bad Request", br#"{"error":"Invalid payload"}"#);
    }
    let authorization = request.header("authorization");
    if authorization.is_some_and(|v| {
        !v.starts_with("Bearer ")
            || v.len() > 4096
            || v.bytes().any(|b| !b.is_ascii_graphic() && b != b' ')
    }) {
        return error(
            stream,
            "400 Bad Request",
            br#"{"error":"Invalid authorization"}"#,
        );
    }
    // Resolve once, reject private addresses and pin the checked results for the
    // connection. This avoids DNS rebinding between validation and the request.
    let addresses: Vec<SocketAddr> = match ("api.xkiro.com", 443).to_socket_addrs() {
        Ok(addrs) => addrs.filter(public_address).collect(),
        Err(_) => Vec::new(),
    };
    if addresses.is_empty() {
        return error(
            stream,
            "502 Bad Gateway",
            br#"{"error":"Provider DNS unavailable"}"#,
        );
    }
    // No inherited proxy, redirects, cookies, caller-supplied URLs or forwarded headers.
    let client = match reqwest::blocking::Client::builder()
        .no_proxy()
        .resolve_to_addrs("api.xkiro.com", &addresses)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            return error(
                stream,
                "502 Bad Gateway",
                br#"{"error":"Provider transport unavailable"}"#,
            )
        }
    };
    let mut outbound = client
        .post(ENDPOINT)
        .header("content-type", "application/json")
        .body(request.body.clone());
    if let Some(value) = authorization {
        outbound = outbound.header("authorization", value);
    }
    let mut response = match outbound.send() {
        Ok(response) => response,
        Err(_) => {
            return error(
                stream,
                "502 Bad Gateway",
                br#"{"error":"Could not reach xKiro"}"#,
            )
        }
    };
    let status = response.status().as_u16();
    // Do not forward untrusted response headers or error bodies. Preserve status
    // so the client can distinguish authorization, rate-limit and server errors.
    if !(200..300).contains(&status) {
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .filter(|v| v.len() <= 128 && v.bytes().all(|b| (0x20..=0x7e).contains(&b)));
        write!(stream, "HTTP/1.1 {status} Provider Error\r\nContent-Type: application/json\r\nContent-Length: 26\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n")?;
        if let Some(value) = retry_after {
            write!(stream, "Retry-After: {value}\r\n")?;
        }
        stream.write_all(b"\r\n{\"error\":\"Provider error\"}")?;
        return Ok(());
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let media_type = content_type.split(';').next().unwrap_or("").trim();
    let content_type = if media_type.eq_ignore_ascii_case("text/event-stream") {
        "text/event-stream"
    } else if media_type.eq_ignore_ascii_case("application/json") {
        "application/json"
    } else {
        return error(
            stream,
            "502 Bad Gateway",
            br#"{"error":"Unsupported provider response"}"#,
        );
    };
    stream.set_write_timeout(Some(Duration::from_secs(15)))?;
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\r\n")?;
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_RESPONSE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Provider response too large",
            ));
        }
        write!(stream, "{n:X}\r\n")?;
        stream.write_all(&buf[..n])?;
        stream.write_all(b"\r\n")?;
        stream.flush()?;
    }
    stream.write_all(b"0\r\n\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn rejects_non_public_addresses() {
        for ip in [
            "127.0.0.1:443",
            "10.0.0.1:443",
            "169.254.169.254:443",
            "192.168.1.1:443",
            "[::1]:443",
            "[fc00::1]:443",
            "[2001:db8::1]:443",
        ] {
            assert!(!public_address(&ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8:443", "[2606:4700:4700::1111]:443"] {
            assert!(public_address(&ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn requires_exact_loopback_origin_without_forwarding() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let _client = TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();
        let mut request = Request {
            method: "POST".into(),
            target: "/api/providers/xkiro/chat/completions".into(),
            body: b"{}".to_vec(),
            headers: vec![
                ("host".into(), addr.to_string()),
                ("origin".into(), format!("http://{addr}")),
                ("content-type".into(), "application/json".into()),
                ("sec-fetch-site".into(), "same-origin".into()),
            ],
        };
        assert!(allowed(&request, &server));
        request.headers[1].1 = "https://evil.example".into();
        assert!(!allowed(&request, &server));
        request.headers[1].1 = format!("http://{addr}");
        request
            .headers
            .push(("x-forwarded-for".into(), "127.0.0.1".into()));
        assert!(!allowed(&request, &server));
        request.headers.pop();
        request.headers[3].1 = "cross-site".into();
        assert!(!allowed(&request, &server));
    }
}
