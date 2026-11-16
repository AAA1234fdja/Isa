use super::Result;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::{Component, Path, PathBuf};

const LIMIT: u64 = 1024 * 1024;

fn safe_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.as_os_str().is_empty() { return Err("path cannot be empty".into()); }
    let mut path = root.to_path_buf();
    for component in relative.components() {
        let name = match component {
            Component::Normal(name) => name,
            _ => return Err("path must be relative and cannot contain parent traversal".into()),
        };
        let name_text = name.to_string_lossy();
        if name_text == ".git" || name_text == ".isa" || name_text == ".env" || name_text.starts_with(".env.") || name_text == ".git-credentials" || name_text == ".netrc" {
            return Err("protected paths cannot be edited".into());
        }
        path.push(name);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() { return Err("symlink paths are not supported".into()); }
    }
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() || metadata.len() > LIMIT { return Err("target must be a regular file smaller than 1 MiB".into()); }
    Ok(path)
}

fn confirm(question: &str) -> Result<bool> {
    if !io::stdin().is_terminal() { return Err("interactive approval is required; refusing unattended edit".into()); }
    eprint!("{question} [y/N] ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}

fn visible(text: &str) -> String {
    text.chars().take(4000).map(|ch| if ch == '\n' || ch == '\t' || !ch.is_control() { ch } else { '�' }).collect()
}

fn backup_dir(root: &Path) -> Result<PathBuf> {
    let dir = root.join(".isa");
    match fs::symlink_metadata(&dir) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => return Err(".isa must be a regular directory".into()),
        Ok(_) => (),
        Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&dir)?,
        Err(e) => return Err(e.into()),
    }
    Ok(dir)
}

fn atomic_write(path: &Path, data: &[u8], copy_permissions: bool) -> Result<()> {
    let parent = path.parent().ok_or("file has no parent")?;
    let temp = parent.join(format!(".isa-edit-{}-{}.tmp", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(data)?;
        if copy_permissions { file.set_permissions(fs::metadata(path)?.permissions())?; }
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(temp); }
    result
}

pub fn replace(root: &Path, relative: &Path, old: &str, new: &str) -> Result<()> {
    if old.is_empty() { return Err("search text cannot be empty".into()); }
    if old.chars().count() > 4000 || new.chars().count() > 4000 { return Err("replacement is too large to review safely (maximum 4000 characters per side)".into()); }
    let path = safe_path(root, relative)?;
    let before = fs::read_to_string(&path)?;
    if before.matches(old).count() != 1 { return Err("search text must match exactly once".into()); }
    let after = before.replacen(old, new, 1);
    if after == before { return Err("replacement would not change the file".into()); }
    if after.len() as u64 > LIMIT { return Err("result exceeds the 1 MiB limit".into()); }
    println!("File: {}\n--- remove ---\n{}\n--- add ---\n{}", relative.display(), visible(old), visible(new));
    if !confirm("Apply this replacement?")? { return Err("edit cancelled".into()); }
    if safe_path(root, relative)? != path || fs::read_to_string(&path)? != before { return Err("file changed while awaiting approval".into()); }
    let dir = backup_dir(root)?;
    let backup = serde_json::to_vec(&json!({"version": 1, "path": relative.to_str().ok_or("path is not UTF-8")?, "before": before, "after": after}))?;
    atomic_write(&dir.join("undo.json"), &backup, false)?;
    atomic_write(&path, after.as_bytes(), true)?;
    println!("Edit applied. Use 'isa-cli undo' to revert it if the file remains unchanged.");
    Ok(())
}

pub fn undo(root: &Path) -> Result<()> {
    let dir = backup_dir(root)?;
    let path = dir.join("undo.json");
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > LIMIT * 3 { return Err("invalid undo record".into()); }
    let record: Value = serde_json::from_slice(&fs::read(&path)?)?;
    if record.get("version").and_then(Value::as_u64) != Some(1) { return Err("unsupported undo record".into()); }
    let relative = Path::new(record.get("path").and_then(Value::as_str).ok_or("invalid undo path")?);
    let before = record.get("before").and_then(Value::as_str).ok_or("invalid undo content")?;
    let after = record.get("after").and_then(Value::as_str).ok_or("invalid undo content")?;
    if before.len() as u64 > LIMIT || after.len() as u64 > LIMIT { return Err("undo content exceeds limit".into()); }
    let target = safe_path(root, relative)?;
    if fs::read_to_string(&target)? != after { return Err("file has changed since the edit; refusing to overwrite it".into()); }
    println!("Undo the last ISA edit to {}", relative.display());
    if !confirm("Restore the previous file contents?")? { return Err("undo cancelled".into()); }
    if safe_path(root, relative)? != target || fs::read_to_string(&target)? != after { return Err("file changed while awaiting approval".into()); }
    atomic_write(&target, before.as_bytes(), true)?;
    fs::remove_file(path)?;
    println!("Edit undone.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_is_rejected() {
        assert!(safe_path(Path::new("."), Path::new("../outside")).is_err());
    }

    #[test]
    fn strips_terminal_controls() {
        assert!(!visible("\u{1b}[31m").contains('\u{1b}'));
    }
}
