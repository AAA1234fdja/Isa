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

pub fn ollama_reply(model: &str, messages: &[Message]) -> Result<String> {
    let mut selected = Vec::new();
    let mut size = 0usize;
    for message in messages.iter().rev() {
        let next = message.content.chars().count();
        if size + next > MAX_CONTEXT_CHARS { break; }
        size += next;
        selected.push(json!({"role": message.role, "content": message.content}));
    }
    selected.reverse();
    let client = reqwest::blocking::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(120)).build()?;
    let response: Value = client.post("http://127.0.0.1:11434/api/chat")
        .json(&json!({"model": model, "messages": selected, "stream": false}))
        .send()?.error_for_status()?.json()?;
    let content = response.pointer("/message/content").and_then(Value::as_str).ok_or("model returned no message")?;
    Ok(content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
