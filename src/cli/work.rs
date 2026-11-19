use super::{repository, Result};
use serde_json::{json, Map, Value};
use similar::{ChangeTag, TextDiff};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_FILES: usize = 20_000;

#[derive(Default)]
pub struct Status {
    pub added: usize,
    pub removed: usize,
    pub changed_files: usize,
    pub skipped_files: usize,
}

fn work_dir(root: &Path, create: bool) -> Result<PathBuf> {
    let isa = root.join(".isa");
    let work = isa.join("work");
    for path in [&isa, &work] {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => (),
            Ok(_) => return Err("work storage must be a real directory, not a symlink".into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound && create => fs::create_dir(path)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(work)
}

fn snapshot_path(root: &Path, create: bool) -> Result<PathBuf> {
    Ok(work_dir(root, create)?.join("baseline.json"))
}

fn files(root: &Path) -> Result<BTreeMap<String, Option<String>>> {
    let paths = repository::files(root)?;
    if paths.len() >= MAX_FILES { return Err("workspace reaches the file limit; cannot create a reliable work count".into()); }
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for relative in paths {
        let name = relative.to_str().ok_or("file path is not UTF-8")?.to_owned();
        let path = root.join(&relative);
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() { return Err("workspace changed while scanning files; retry".into()); }
        let value = if metadata.len() > MAX_FILE_BYTES {
            None
        } else {
            let bytes = fs::read(&path)?;
            if bytes.len() as u64 > MAX_FILE_BYTES { return Err("file changed while scanning; retry".into()); }
            if bytes.contains(&0) { None } else { String::from_utf8(bytes).ok().filter(|text| text.lines().take(10_001).count() <= 10_000) }
        };
        if let Some(text) = &value {
            total = total.checked_add(text.len()).ok_or("workspace size overflow")?;
            if total > MAX_TOTAL_BYTES { return Err("workspace text exceeds the 16 MiB work tracking limit".into()); }
        }
        files.insert(name, value);
    }
    Ok(files)
}

pub fn start(root: &Path) -> Result<()> {
    let snapshot = files(root)?;
    let path = snapshot_path(root, true)?;
    let metadata = json!({
        "version": 1,
        "created_at": SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        "files": snapshot,
    });
    let data = serde_json::to_vec(&metadata)?;
    if data.len() as u64 > MAX_SNAPSHOT_BYTES { return Err("work snapshot exceeds the size limit".into()); }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            io::Error::new(io::ErrorKind::AlreadyExists, "work is already active; end it before starting another")
        } else { error }
    })?;
    if let Err(error) = file.write_all(&data).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

fn read_baseline(root: &Path) -> Result<BTreeMap<String, Option<String>>> {
    let path = snapshot_path(root, false)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_SNAPSHOT_BYTES { return Err("invalid work snapshot".into()); }
    let snapshot: Value = serde_json::from_slice(&fs::read(path)?)?;
    if snapshot.get("version").and_then(Value::as_u64) != Some(1) { return Err("unsupported work snapshot".into()); }
    let entries: &Map<String, Value> = snapshot.get("files").and_then(Value::as_object).ok_or("invalid work snapshot files")?;
    if entries.len() >= MAX_FILES { return Err("work snapshot exceeds the file limit".into()); }
    let mut files = BTreeMap::new();
    for (path, value) in entries {
        let text = match value {
            Value::Null => None,
            Value::String(text) if text.len() as u64 <= MAX_FILE_BYTES => Some(text.clone()),
            _ => return Err("invalid work snapshot entry".into()),
        };
        files.insert(path.clone(), text);
    }
    Ok(files)
}

pub fn status(root: &Path) -> Result<Status> {
    let baseline = read_baseline(root)?;
    let current = files(root)?;
    let paths: BTreeSet<_> = baseline.keys().chain(current.keys()).collect();
    let mut result = Status::default();
    for path in paths {
        let before = baseline.get(path);
        let after = current.get(path);
        if matches!(before, Some(None)) || matches!(after, Some(None)) {
            result.skipped_files += 1;
            continue;
        }
        let old = before.and_then(Option::as_deref).unwrap_or("");
        let new = after.and_then(Option::as_deref).unwrap_or("");
        if old == new { continue; }
        result.changed_files += 1;
        for change in TextDiff::from_lines(old, new).iter_all_changes() {
            match change.tag() {
                ChangeTag::Insert => result.added += 1,
                ChangeTag::Delete => result.removed += 1,
                ChangeTag::Equal => (),
            }
        }
    }
    Ok(result)
}

pub fn end(root: &Path) -> Result<()> {
    let path = snapshot_path(root, false)?;
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() { return Err("invalid work snapshot".into()); }
    fs::remove_file(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn counts_changes_relative_to_work_start() {
        let root = std::env::temp_dir().join(format!("isa-work-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("existing.txt"), "old\nkeep\n").unwrap();
        start(&root).unwrap();
        fs::write(root.join("existing.txt"), "new\nkeep\n").unwrap();
        fs::write(root.join("new.txt"), "one\ntwo\n").unwrap();
        let result = status(&root).unwrap();
        assert_eq!((result.added, result.removed, result.changed_files), (3, 1, 2));
        end(&root).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

// Web actions serialize with each other, not with CLI processes or outside writers.
static WEB_WORK_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn baseline_digest(root:&Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let path=snapshot_path(root,false)?;let meta=fs::symlink_metadata(&path)?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len()>MAX_SNAPSHOT_BYTES {return Err("invalid work baseline".into())}
    let mut options=OpenOptions::new();options.read(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt;options.custom_flags(libc::O_NOFOLLOW); }
    let file=options.open(&path)?;if !file.metadata()?.is_file() {return Err("invalid work baseline".into())}
    let mut data=Vec::new();file.take(MAX_SNAPSHOT_BYTES+1).read_to_end(&mut data)?;
    if data.len() as u64>MAX_SNAPSHOT_BYTES {return Err("work baseline too large".into())}
    Ok(format!("{:x}",Sha256::digest(&data)))
}
pub(super) fn web_prepare_end(root:&Path)->Result<String> { baseline_digest(root) }
pub(super) fn web_start(root:&Path)->Result<Value> {
    let _guard=WEB_WORK_LOCK.lock().map_err(|_|"work lock unavailable")?;start(root)?;Ok(json!({"started":true}))
}
pub(super) fn web_status(root:&Path)->Result<Value> {
    let _guard=WEB_WORK_LOCK.lock().map_err(|_|"work lock unavailable")?;let s=status(root)?;
    Ok(json!({"added":s.added,"removed":s.removed,"changedFiles":s.changed_files,"skippedFiles":s.skipped_files}))
}
pub(super) fn web_end(root:&Path,expected:&str)->Result<Value> {
    let _guard=WEB_WORK_LOCK.lock().map_err(|_|"work lock unavailable")?;
    if baseline_digest(root)?!=expected {return Err("work baseline changed since preparation".into())}
    end(root)?;Ok(json!({"ended":true}))
}
