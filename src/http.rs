use std::io::{self, Read, Write};
use std::net::TcpStream;

const MAX_HEADERS: usize = 16 * 1024;
const MAX_BODY: usize = 128 * 1024;

pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }
    pub fn is_same_origin_json(&self) -> bool {
        let content_type = self.header("content-type").unwrap_or("").split(';').next().unwrap_or("").trim();
        let host = self.header("host").unwrap_or("");
        let origin = self.header("origin").unwrap_or("");
        !host.is_empty() && content_type.eq_ignore_ascii_case("application/json")
            && (origin == format!("http://{host}") || origin == format!("https://{host}"))
    }
}

pub fn read_request(stream: &mut TcpStream) -> io::Result<Request> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    let mut buffer = Vec::new();
    let end = loop {
        if let Some(pos) = buffer.windows(4).position(|bytes| bytes == b"\r\n\r\n") { break pos + 4; }
        if buffer.len() >= MAX_HEADERS { return Err(io::Error::new(io::ErrorKind::InvalidData, "headers too large")); }
        let mut chunk = [0; 4096];
        let read = stream.read(&mut chunk)?;
        if read == 0 { return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "incomplete request")); }
        buffer.extend_from_slice(&chunk[..read]);
    };
    if end > MAX_HEADERS { return Err(io::Error::new(io::ErrorKind::InvalidData, "headers too large")); }
    let header = std::str::from_utf8(&buffer[..end]).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid headers"))?;
    let mut lines = header.split("\r\n");
    let mut request_line = lines.next().unwrap_or("").split_whitespace();
    let method = request_line.next().unwrap_or("").to_owned();
    let target = request_line.next().unwrap_or("").to_owned();
    if method.is_empty() || target.is_empty() { return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid request")); }
    let mut headers = Vec::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid header"))?;
        if headers.iter().any(|(key, _): &(String, String)| key.eq_ignore_ascii_case(name)) { return Err(io::Error::new(io::ErrorKind::InvalidData, "duplicate header")); }
        headers.push((name.trim().to_owned(), value.trim().to_owned()));
    }
    if headers.iter().any(|(key, _)| key.eq_ignore_ascii_case("transfer-encoding")) { return Err(io::Error::new(io::ErrorKind::InvalidData, "transfer encoding is not supported")); }
    let length = match headers.iter().find(|(key, _)| key.eq_ignore_ascii_case("content-length")) {
        Some((_, value)) => value.parse::<usize>().map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid content length"))?,
        None => 0,
    };
    if length > MAX_BODY { return Err(io::Error::new(io::ErrorKind::InvalidData, "body too large")); }
    let mut body = buffer[end..].to_vec();
    if body.len() > length { body.truncate(length); }
    while body.len() < length {
        let mut chunk = [0; 4096];
        let needed = (length - body.len()).min(chunk.len());
        let read = stream.read(&mut chunk[..needed])?;
        if read == 0 { return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "incomplete body")); }
        body.extend_from_slice(&chunk[..read]);
    }
    Ok(Request { method, target, headers, body })
}

pub fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8], head: bool) -> io::Result<()> {
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\n\r\n", body.len())?;
    if !head { stream.write_all(body)?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_matching_origin_and_json() {
        let mut request = Request { method: "POST".into(), target: "/".into(), body: vec![], headers: vec![("host".into(), "example.test".into()), ("origin".into(), "https://example.test".into()), ("content-type".into(), "application/json".into())] };
        assert!(request.is_same_origin_json());
        request.headers[1].1 = "https://evil.test".into();
        assert!(!request.is_same_origin_json());
    }
}
