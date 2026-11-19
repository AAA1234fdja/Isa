use super::Result;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_RECORD_BYTES: u64 = 256 * 1024;
const MAX_TEXT_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy)]
pub enum Kind {
    Contract,
    Goal,
}

impl Kind {
    fn folder(self) -> &'static str {
        match self {
            Self::Contract => "contracts",
            Self::Goal => "goals",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Contract => "contract",
            Self::Goal => "goal",
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn timestamp() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

fn checked_text(text: &str) -> Result<&str> {
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_TEXT_BYTES {
        return Err("text must be nonempty and at most 16 KiB".into());
    }
    Ok(text)
}

fn dir(root: &Path, kind: Kind, create: bool) -> Result<PathBuf> {
    let base = root.join(".isa");
    let child = base.join(kind.folder());
    for path in [&base, &child] {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => (),
            Ok(_) => {
                return Err("registry directory must be a real directory, not a symlink".into())
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound && create => {
                fs::create_dir(path)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(child)
}

fn record_path(root: &Path, kind: Kind, id: &str, create_dir: bool) -> Result<PathBuf> {
    if !valid_id(id) {
        return Err(
            "ID must contain only letters, numbers, '-' or '_' (maximum 80 characters)".into(),
        );
    }
    Ok(dir(root, kind, create_dir)?.join(format!("{id}.json")))
}

fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

fn encoded(record: &Value) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec_pretty(record)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err("record exceeds size limit".into());
    }
    Ok(bytes)
}

fn create(root: &Path, kind: Kind, id: &str, record: &Value) -> Result<()> {
    let path = record_path(root, kind, id, true)?;
    let data = encoded(record)?;
    let mut file = private_options().open(&path)?;
    if let Err(error) = file.write_all(&data).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&path);
        return Err(error.into());
    }
    Ok(())
}

fn update(root: &Path, kind: Kind, id: &str, record: &Value) -> Result<()> {
    let path = record_path(root, kind, id, false)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("record must be a regular file".into());
    }
    let data = encoded(record)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temp = path.with_file_name(format!(".{id}-{}-{nonce}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = private_options().open(&temp)?;
        file.write_all(&data)?;
        file.sync_all()?;
        drop(file); // Close the temporary file before replacing the destination.
        fs::rename(&temp, &path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

pub fn read(root: &Path, kind: Kind, id: &str) -> Result<Value> {
    let path = record_path(root, kind, id, false)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_RECORD_BYTES
    {
        return Err("record must be a regular file within the size limit".into());
    }
    let record: Value = serde_json::from_slice(&fs::read(path)?)?;
    if record.get("version").and_then(Value::as_u64) != Some(1)
        || record.get("id").and_then(Value::as_str) != Some(id)
        || record.get("kind").and_then(Value::as_str) != Some(kind.label())
    {
        return Err("invalid or unsupported registry record".into());
    }
    Ok(record)
}

pub fn list(root: &Path, kind: Kind) -> Result<Vec<String>> {
    let dir = dir(root, kind, false)?;
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut ids = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
            if valid_id(id) {
                ids.push(id.to_owned());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

pub fn create_contract(root: &Path, id: &str, objective: &str) -> Result<()> {
    let objective = checked_text(objective)?;
    let now = timestamp()?;
    create(
        root,
        Kind::Contract,
        id,
        &json!({
            "version": 1,
            "kind": "contract",
            "id": id,
            "objective": objective,
            "instructions": "",
            "deliverables": "",
            "allowed_tools": [],
            "status": "draft",
            "created_at": now,
            "updated_at": now,
        }),
    )
}

pub fn set_contract(root: &Path, id: &str, field: &str, value: &str) -> Result<()> {
    let mut record = read(root, Kind::Contract, id)?;
    match field {
        "objective" | "instructions" | "deliverables" => {
            record[field] = json!(checked_text(value)?);
        }
        "status" if matches!(value, "draft" | "ready" | "archived") => {
            record[field] = json!(value);
        }
        "status" => return Err("contract status must be draft, ready, or archived".into()),
        _ => {
            return Err(
                "editable contract fields: objective, instructions, deliverables, status".into(),
            )
        }
    }
    record["updated_at"] = json!(timestamp()?);
    update(root, Kind::Contract, id, &record)
}

pub fn create_goal(root: &Path, id: &str, objective: &str) -> Result<()> {
    let objective = checked_text(objective)?;
    let now = timestamp()?;
    create(
        root,
        Kind::Goal,
        id,
        &json!({
            "version": 1,
            "kind": "goal",
            "id": id,
            "objective": objective,
            "status": "open",
            "contract_id": null,
            "created_at": now,
            "updated_at": now,
        }),
    )
}

pub fn set_goal_status(root: &Path, id: &str, status: &str) -> Result<()> {
    if !matches!(status, "open" | "paused" | "completed") {
        return Err("goal status must be open, paused, or completed".into());
    }
    let mut record = read(root, Kind::Goal, id)?;
    record["status"] = json!(status);
    record["updated_at"] = json!(timestamp()?);
    update(root, Kind::Goal, id, &record)
}

pub fn bind_goal(root: &Path, goal_id: &str, contract_id: &str) -> Result<()> {
    let contract = read(root, Kind::Contract, contract_id)?;
    if contract.get("status").and_then(Value::as_str) == Some("archived") {
        return Err("cannot bind an archived contract".into());
    }
    let mut goal = read(root, Kind::Goal, goal_id)?;
    goal["contract_id"] = json!(contract_id);
    goal["updated_at"] = json!(timestamp()?);
    update(root, Kind::Goal, goal_id, &goal)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_reject_traversal_and_empty_values() {
        assert!(!valid_id("../bad"));
        assert!(!valid_id(""));
        assert!(valid_id("good-id_42"));
    }
    #[test]
    fn goals_reject_invalid_status() {
        assert!(set_goal_status(Path::new("."), "missing", "running").is_err());
    }
}

#[cfg(test)]
mod windows_file_replacement_tests {
    use super::*;
    #[test]
    fn windows_file_replacement_registry_updates_existing_record() {
        let root = std::env::temp_dir().join(format!(
            "isa-registry-replace-{}-{}-é",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let mut record = json!({"version":1,"id":"test","kind":"goal","text":"first"});
        create(&root, Kind::Goal, "test", &record).unwrap();
        record["text"] = json!("second");
        update(&root, Kind::Goal, "test", &record).unwrap();
        assert_eq!(read(&root, Kind::Goal, "test").unwrap()["text"], "second");
        fs::remove_dir_all(root).unwrap();
    }
}
