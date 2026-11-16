use super::Result;
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

const MAX_SESSION_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CONTEXT_CHARS: usize = 100_000;

#[derive(Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn new(role: &str, content: &str) -> Self {
        Self { role: role.to_string(), content: content.to_string() }
    }
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > 80 || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
        return Err("session ID must contain only letters, numbers, '-' or '_' (maximum 80 characters)".into());
    }
    Ok(())
}

fn session_dir(root: &Path) -> PathBuf {
    root.join(".isa").join("sessions")
}

fn check_session_dirs(root: &Path) -> Result<()> {
    for path in [root.join(".isa"), session_dir(root)] {
        match fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => return Err("session directory must not be a symlink".into()),
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn session_path(root: &Path, id: &str) -> Result<PathBuf> {
    validate_id(id)?;
    check_session_dirs(root)?;
    Ok(session_dir(root).join(format!("{id}.json")))
}

pub fn list(root: &Path) -> Result<Vec<String>> {
    check_session_dirs(root)?;
    let mut result = Vec::new();
    let entries = match fs::read_dir(session_dir(root)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(result),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() { continue; }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
        if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
            if validate_id(id).is_ok() { result.push(id.to_string()); }
        }
    }
    result.sort();
    Ok(result)
}

pub fn load(root: &Path, id: &str) -> Result<Vec<Message>> {
    let path = session_path(root, id)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_file() || metadata.len() > MAX_SESSION_BYTES {
        return Err("session is not a regular file or exceeds the size limit".into());
    }
    let value: Value = serde_json::from_slice(&fs::read(path)?)?;
    let entries = value.get("messages").and_then(Value::as_array).ok_or("invalid session format")?;
    let mut messages = Vec::new();
    for entry in entries {
        let role = entry.get("role").and_then(Value::as_str).ok_or("invalid message role")?;
        let content = entry.get("content").and_then(Value::as_str).ok_or("invalid message content")?;
        if role != "user" && role != "assistant" { return Err("unsupported message role".into()); }
        messages.push(Message::new(role, content));
    }
    Ok(messages)
}

pub fn save(root: &Path, id: &str, messages: &[Message]) -> Result<()> {
    let path = session_path(root, id)?;
    let dir = session_dir(root);
    fs::create_dir_all(&dir)?;
    let entries: Vec<Value> = messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect();
    let data = serde_json::to_vec_pretty(&json!({"version": 1, "messages": entries}))?;
    if data.len() as u64 > MAX_SESSION_BYTES { return Err("session exceeds the 8 MiB size limit".into()); }
    let temporary = dir.join(format!(".{id}.{}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&data)?;
        file.sync_all()?;
        fs::rename(&temporary, &path)?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temporary); }
    result
}

fn select_context(messages: &[Message]) -> Vec<Value> {
    let mut selected = Vec::new();
    let mut size = 0usize;
    for message in messages.iter().rev() {
        let next = message.content.chars().count();
        if size + next > MAX_CONTEXT_CHARS { break; }
        size += next;
        selected.push(json!({"role": message.role, "content": message.content}));
    }
    selected.reverse();
    selected
}

const MAX_EVENT_BYTES: usize = 1024 * 1024;
const MAX_REPLY_BYTES: usize = 4 * 1024 * 1024;

fn read_event<R: std::io::BufRead>(reader: &mut R) -> Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return if line.is_empty() { Ok(None) } else { Ok(Some(line)) };
        }
        let size = buffer.iter().position(|byte| *byte == b'\n').map(|i| i + 1).unwrap_or(buffer.len());
        if line.len() + size > MAX_EVENT_BYTES { return Err("model stream event exceeds size limit".into()); }
        line.extend_from_slice(&buffer[..size]);
        let complete = line.last() == Some(&b'\n');
        reader.consume(size);
        if complete { return Ok(Some(line)); }
    }
}

fn consume_stream<R: std::io::BufRead, F: FnMut(&str) -> Result<()>>(mut reader: R, mut on_chunk: F) -> Result<String> {
    let mut output = String::new();
    let mut completed = false;
    while let Some(line) = read_event(&mut reader)? {
        if line.iter().all(u8::is_ascii_whitespace) { continue; }
        let event: Value = serde_json::from_slice(&line)?;
        if let Some(error) = event.get("error").and_then(Value::as_str) {
            return Err(format!("model error: {error}").into());
        }
        if let Some(content) = event.pointer("/message/content").and_then(Value::as_str) {
            if output.len() + content.len() > MAX_REPLY_BYTES { return Err("model response exceeds size limit".into()); }
            on_chunk(content)?;
            output.push_str(content);
        }
        if event.get("done").and_then(Value::as_bool) == Some(true) {
            completed = true;
            break;
        }
    }
    if !completed { return Err("model stream ended before completion".into()); }
    if output.is_empty() { return Err("model returned an empty response".into()); }
    Ok(output)
}

pub fn ollama_stream<F: FnMut(&str) -> Result<()>>(model: &str, messages: &[Message], on_chunk: F) -> Result<String> {
    let selected = select_context(messages);
    if selected.is_empty() { return Err("no messages fit in the model context".into()); }
    let client = reqwest::blocking::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(120)).build()?;
    let response = client.post("http://127.0.0.1:11434/api/chat")
        .json(&json!({"model": model, "messages": selected, "stream": true}))
        .send()?.error_for_status()?;
    consume_stream(std::io::BufReader::new(response), on_chunk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_combines_chunks_and_requires_done() {
        let events = b"{\"message\":{\"content\":\"Hello \"},\"done\":false}\n{\"message\":{\"content\":\"world\"},\"done\":true}\n";
        let mut seen = String::new();
        let response = consume_stream(&events[..], |chunk| { seen.push_str(chunk); Ok(()) }).unwrap();
        assert_eq!(response, "Hello world");
        assert_eq!(seen, response);
        assert!(consume_stream(&events[..events.iter().position(|b| *b == b'\n').unwrap() + 1], |_| Ok(())).is_err());
    }

    #[test]
    fn stream_reports_provider_errors() {
        let event = b"{\"error\":\"model missing\"}\n";
        assert!(consume_stream(&event[..], |_| Ok(())).is_err());
    }

    #[test]
    fn session_id_rejects_traversal() {
        assert!(validate_id("../secret").is_err());
        assert!(validate_id("good-session_1").is_ok());
        assert!(validate_id("").is_err());
    }

    #[test]
    fn missing_sessions_return_empty() {
        let root = std::env::temp_dir().join(format!("isa-missing-{}", std::process::id()));
        assert!(load(&root, "new").unwrap().is_empty());
    }
}
