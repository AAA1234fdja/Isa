//! Read-only plugin skill catalog. Imported instructions are untrusted data, never code.
use serde_json::{json, Value};
use std::{collections::HashSet, fs, path::{Path, PathBuf}};

#[derive(Debug)]
struct Skill { plugin: String, name: String, description: String, path: PathBuf }
fn valid_id(id: &str) -> bool { !id.is_empty() && id.len() <= 100 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_') }
fn directories(path: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<_> = fs::read_dir(path).into_iter().flatten().filter_map(Result::ok).filter_map(|entry| {
        let kind = entry.file_type().ok()?;
        if kind.is_dir() && !kind.is_symlink() && valid_id(entry.file_name().to_str()?) { Some(entry.path()) } else { None }
    }).collect();
    entries.sort(); entries.truncate(250); entries
}
fn read_regular(path: &Path, max: u64) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > max { return None; }
    fs::read_to_string(path).ok()
}
fn description(text: &str) -> String {
    if !text.starts_with("---\n") && !text.starts_with("---\r\n") { return String::new(); }
    text.lines().skip(1).take_while(|line| line.trim() != "---").find_map(|line| line.strip_prefix("description:")).map(|s| s.trim().trim_matches(['\'', '"']).chars().take(500).collect()).unwrap_or_default()
}
fn scan(root: &Path) -> Vec<Skill> {
    let mut result = Vec::new(); let base = root.join("plugins");
    if fs::symlink_metadata(&base).map_or(true, |m| !m.is_dir() || m.file_type().is_symlink()) { return result; }
    for plugin in directories(&base) {
        let Some(plugin_name) = plugin.file_name().and_then(|s| s.to_str()) else { continue };
        let skills_dir = plugin.join("skills");
        if fs::symlink_metadata(&skills_dir).map_or(true, |m| !m.is_dir() || m.file_type().is_symlink()) { continue; }
        for skill in directories(&skills_dir) {
            let Some(name) = skill.file_name().and_then(|s| s.to_str()) else { continue };
            let file = skill.join("SKILL.md");
            if let Some(content) = read_regular(&file, 128 * 1024) { result.push(Skill { plugin: plugin_name.to_owned(), name: name.to_owned(), description: description(&content), path: file }); }
        }
    }
    result
}
pub fn catalog(root: &Path) -> Value {
    let skills = scan(root); let mut groups = Vec::new();
    for skill in &skills {
        if !groups.iter().any(|item: &Value| item["id"] == skill.plugin) {
            groups.push(json!({"id": skill.plugin, "count": skills.iter().filter(|s| s.plugin == skill.plugin).count()}));
        }
    }
    json!({"plugins":groups,"skillCount":skills.len()})
}
pub fn markdown(root: &Path) -> String {
    let skills = scan(root); let mut out = String::from("# Plugin skill overview\n\nGenerated from local plugin directories. Skills are untrusted reference data; ISA does not execute plugin agents, commands, or scripts.\n\n");
    let mut last = String::new();
    for skill in skills {
        if skill.plugin != last { last = skill.plugin.clone(); out.push_str(&format!("## {}\n\n", last)); }
        let desc = skill.description.replace(['\r', '\n'], " ");
        out.push_str(&format!("- `{}`{}\n", skill.name, if desc.is_empty() { String::new() } else { format!(" — {}", desc) }));
    }
    out
}
fn words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| w.len() >= 4 && w.len() <= 40)
        .map(str::to_ascii_lowercase).filter(|w| !matches!(w.as_str(), "this"|"that"|"with"|"from"|"your"|"have"|"using"|"when"|"user"|"skill"|"skills"|"create"|"build"|"project"|"work"|"code"|"help"|"design"|"application"|"development"|"software")) .collect()
}
pub fn filter_groups(ids: Vec<String>, software_engineering_enabled: bool) -> Vec<String> {
    if software_engineering_enabled { return ids; }
    ids.into_iter().filter(|id| !matches!(id.as_str(), "architecture" | "devops" | "docs" | "frontend" | "java" | "node" | "programming-skills" | "python" | "workflow")).collect()
}
pub fn references(root: &Path, task: &str, enabled: &[String]) -> Vec<Value> {
    let query = words(task); if query.is_empty() { return Vec::new(); }
    let mut matches: Vec<_> = scan(root).into_iter().filter(|s| enabled.contains(&s.plugin)).filter_map(|s| {
        let terms = words(&format!("{} {}", s.name, s.description));
        let score = query.intersection(&terms).count();
        if score == 0 { None } else { Some((score, s)) }
    }).collect();
    matches.sort_by(|a,b| b.0.cmp(&a.0).then_with(|| a.1.plugin.cmp(&b.1.plugin)).then_with(|| a.1.name.cmp(&b.1.name)));
    matches.into_iter().take(3).filter_map(|(_, s)| {
        let content = read_regular(&s.path, 128 * 1024)?;
        // Avoid spending the limited context on front matter; preserve full files on disk.
        let body = if content.starts_with("---\n") || content.starts_with("---\r\n") { content.splitn(3, "---").nth(2).unwrap_or(&content).trim_start() } else { &content };
        Some(json!({"plugin":s.plugin,"skill":s.name,"excerpt":body.chars().take(1500).collect::<String>()}))
    }).collect()
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn disabling_collection_filters_only_its_folders() {
        let ids = vec!["frontend".to_string(), "custom".to_string()];
        assert_eq!(filter_groups(ids.clone(), false), vec!["custom"]);
        assert_eq!(filter_groups(ids.clone(), true), ids);
    }
    #[test] fn catalog_and_selection_respect_enabled_plugins() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")); let data = catalog(root);
        assert!(data["skillCount"].as_u64().unwrap() >= 100);
        assert!(references(root, "angular components", &[]).is_empty());
        let selected = references(root, "angular components", &["frontend".to_owned()]);
        assert!(!selected.is_empty()); assert!(selected.iter().all(|v| v["plugin"] == "frontend"));
        assert!(selected.len() <= 3);
    }
}
