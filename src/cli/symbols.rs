//! A small, bounded syntax-derived symbol lookup. Not a name resolver or call graph.
use super::{repository, Result};
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

const MAX_FILES: usize = 1000;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024;
const MAX_NODES: usize = 40_000;
const MAX_MATCHES: usize = 40;

fn language(path: &Path) -> Option<tree_sitter::Language> {
    match path.extension()?.to_str()? {
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        "py" => Some(tree_sitter_python::LANGUAGE.into()),
        "js" | "mjs" | "cjs" | "jsx" => Some(tree_sitter_javascript::LANGUAGE.into()),
        _ => None,
    }
}

fn declaration(kind: &str) -> bool {
    matches!(kind, "function_item" | "struct_item" | "enum_item" | "trait_item" | "type_item" | "mod_item" | "const_item" | "static_item" | "function_definition" | "class_definition" | "function_declaration" | "generator_function_declaration" | "class_declaration" | "method_definition")
}

pub fn lookup(root: &Path, query: &str) -> Result<Value> {
    let query = query.trim();
    if query.is_empty() || query.len() > 100 || query.chars().any(char::is_control) { return Err("symbol query must contain 1 to 100 bytes without control characters".into()); }
    let needle = query.to_lowercase();
    let mut parser = tree_sitter::Parser::new();
    let mut results = Vec::new();
    let mut files_scanned = 0usize;
    let mut bytes_scanned = 0u64;
    let mut parse_errors = 0usize;
    let mut skipped_files = 0usize;
    let mut truncated = false;
    let mut nodes_seen = 0usize;
    for path in repository::files(root)? {
        let Some(lang) = language(&path) else { continue };
        if files_scanned >= MAX_FILES || nodes_seen >= MAX_NODES { truncated = true; break; }
        let absolute = root.join(&path);
        let metadata = match fs::symlink_metadata(&absolute) { Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() <= MAX_FILE_BYTES => m, _ => { skipped_files += 1; continue } };
        if bytes_scanned.saturating_add(metadata.len()) > MAX_TOTAL_BYTES { truncated = true; break; }
        let mut content = Vec::new();
        let mut file = match File::open(&absolute) { Ok(file) => file, Err(_) => { skipped_files += 1; continue } };
        if Read::by_ref(&mut file).take(MAX_FILE_BYTES + 1).read_to_end(&mut content).is_err() || content.len() as u64 > MAX_FILE_BYTES || std::str::from_utf8(&content).is_err() || content.contains(&0) { skipped_files += 1; continue; }
        if bytes_scanned.saturating_add(content.len() as u64) > MAX_TOTAL_BYTES { truncated = true; break; }
        files_scanned += 1;
        bytes_scanned += content.len() as u64;
        parser.set_language(&lang)?;
        let Some(tree) = parser.parse(&content, None) else { parse_errors += 1; continue };
        if tree.root_node().has_error() { parse_errors += 1; }
        let mut pending = vec![tree.root_node()];
        while let Some(node) = pending.pop() {
            if nodes_seen >= MAX_NODES { truncated = true; break; }
            nodes_seen += 1;
            if declaration(node.kind()) {
                if let Some(name) = node.child_by_field_name("name").and_then(|n| n.utf8_text(&content).ok()) {
                    if name.len() <= 160 && !name.chars().any(char::is_control) && name.to_lowercase().contains(&needle) {
                        results.push(json!({"path":path.to_string_lossy(),"name":name,"kind":node.kind(),"line":node.start_position().row + 1}));
                        if results.len() >= MAX_MATCHES { truncated = true; break; }
                    }
                }
            }
            let mut cursor = node.walk();
            let children: Vec<_> = node.named_children(&mut cursor).collect();
            pending.extend(children.into_iter().rev());
        }
        if truncated { break; }
    }
    Ok(json!({"version":1,"method":"tree-sitter declarations","query":query,"symbols_are_untrusted":true,"files_scanned":files_scanned,"parse_error_files":parse_errors,"skipped_files":skipped_files,"truncated":truncated,"symbols":results}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognizes_declarations_without_comment_false_positives() {
        let root = std::env::temp_dir().join(format!("isa-symbols-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("source.rs"), "// fn fake_parse() {}\nfn parse_input() {}\nstruct Parser {}\n").unwrap();
        fs::write(root.join("source.py"), "class Parser:\n    def parse_file(self):\n        pass\n").unwrap();
        fs::write(root.join("source.js"), "function parseRequest() {}\nclass Parser {}\n").unwrap();
        fs::write(root.join(".env"), "fn parse_secret() {}").unwrap();
        let value = lookup(&root, "parse").unwrap();
        let items = value["symbols"].as_array().unwrap();
        assert!(items.iter().any(|s| s["name"] == "parse_input" && s["line"] == 2));
        assert!(items.iter().any(|s| s["name"] == "parse_file" && s["line"] == 2));
        assert!(items.iter().any(|s| s["name"] == "parseRequest"));
        assert!(!items.iter().any(|s| s["name"] == "fake_parse" || s["name"] == "parse_secret"));
        assert!(lookup(&root, "").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn limits_matches_and_ignores_unsupported_extensions() {
        let root = std::env::temp_dir().join(format!("isa-symbol-limits-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("source.rs"), (0..50).map(|i| format!("fn match_{i}() {{}}\n")).collect::<String>()).unwrap();
        fs::write(root.join("ignored.txt"), "fn match_text() {}\n").unwrap();
        let result = lookup(&root, "match_").unwrap();
        assert_eq!(result["symbols"].as_array().unwrap().len(), MAX_MATCHES);
        assert_eq!(result["truncated"], true);
        fs::remove_dir_all(root).unwrap();
    }
}
