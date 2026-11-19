//! Bounded syntax-derived declarations and call sites. No semantic name resolution.
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
        "ts" | "mts" | "cts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        _ => None,
    }
}

// Advisory only: a bounded parse of each complete snapshot, without returning
// source text or claiming that the grammar reports all compiler errors.
pub fn compare_syntax(path: &Path, before: &str, after: &str) -> Value {
    const LIMIT: usize = 64 * 1024;
    let Some(lang) = language(path) else {
        return json!({"status":"unsupported"});
    };
    if before.len() > LIMIT || after.len() > LIMIT {
        return json!({"status":"too_large"});
    }
    fn inspect(lang: &tree_sitter::Language, source: &str) -> Option<(bool, Vec<Value>, bool)> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(lang).ok()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
        let mut progress = |_: &tree_sitter::ParseState| std::time::Instant::now() >= deadline;
        let bytes = source.as_bytes();
        let mut read = |offset: usize, _: tree_sitter::Point| &bytes[offset.min(bytes.len())..];
        let options = tree_sitter::ParseOptions::new().progress_callback(&mut progress);
        let tree = parser.parse_with_options(&mut read, None, Some(options))?;
        let root = tree.root_node();
        let has_errors = root.has_error();
        let mut issues = Vec::new();
        let mut pending = vec![root];
        let mut visited = 0usize;
        let mut truncated = false;
        'walk: while let Some(node) = pending.pop() {
            if visited >= 20_000 || std::time::Instant::now() >= deadline {
                truncated = true;
                break;
            }
            visited += 1;
            if node.is_error() || node.is_missing() {
                if issues.len() >= 8 {
                    truncated = true;
                    break;
                }
                let pos = node.start_position();
                issues.push(json!({"kind": if node.is_missing() { "missing" } else { "error" }, "line":pos.row + 1,"byteColumn":pos.column + 1}));
            }
            // Only branches carrying errors need inspection, plus missing nodes.
            let mut cursor = node.walk();
            let mut children = Vec::new();
            for child in node.children(&mut cursor) {
                if visited >= 20_000 || std::time::Instant::now() >= deadline {
                    truncated = true;
                    break 'walk;
                }
                visited += 1;
                if child.has_error() || child.is_error() || child.is_missing() {
                    children.push(child);
                }
            }
            pending.extend(children.into_iter().rev());
        }
        Some((has_errors, issues, truncated))
    }
    match (inspect(&lang, before), inspect(&lang, after)) {
        (
            Some((before_error, before_issues, before_truncated)),
            Some((after_error, after_issues, after_truncated)),
        ) => {
            json!({"status":"checked","method":"tree-sitter","beforeHasErrors":before_error,"afterHasErrors":after_error,"beforeIssues":before_issues,"afterIssues":after_issues,"beforeIssuesTruncated":before_truncated,"afterIssuesTruncated":after_truncated})
        }
        _ => json!({"status":"unavailable"}),
    }
}

fn declaration(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "type_item"
            | "mod_item"
            | "const_item"
            | "static_item"
            | "function_definition"
            | "class_definition"
            | "function_declaration"
            | "generator_function_declaration"
            | "class_declaration"
            | "method_definition"
            | "interface_declaration"
            | "type_alias_declaration"
            | "enum_declaration"
    )
}

// Only simple bindings of functions are named; destructuring and wrappers are not resolved.
fn assigned_function_name<'a>(
    node: tree_sitter::Node<'a>,
    content: &'a [u8],
) -> Option<(&'a str, &'static str)> {
    if node.kind() != "variable_declarator" {
        return None;
    }
    let name = node.child_by_field_name("name")?;
    if name.kind() != "identifier" {
        return None;
    }
    let value = node.child_by_field_name("value")?;
    let kind = match value.kind() {
        "arrow_function" => "arrow_function",
        "function_expression" => "function_expression",
        "generator_function" => "generator_function",
        _ => return None,
    };
    Some((name.utf8_text(content).ok()?, kind))
}

#[derive(Clone, Copy)]
enum Lookup {
    Declarations,
    Calls,
}

fn callable_name<'a>(mut node: tree_sitter::Node<'a>, content: &'a [u8]) -> Option<&'a str> {
    // Only statically spelled names or member names; computed calls are omitted.
    for _ in 0..16 {
        match node.kind() {
            "identifier" | "field_identifier" | "property_identifier" => {
                return node.utf8_text(content).ok()
            }
            "attribute" => node = node.child_by_field_name("attribute")?,
            "member_expression" => node = node.child_by_field_name("property")?,
            "field_expression" => node = node.child_by_field_name("field")?,
            "scoped_identifier" => node = node.child_by_field_name("name")?,
            "generic_function" => node = node.child_by_field_name("function")?,
            _ => return None,
        }
    }
    None
}

fn enclosing_function<'a>(mut node: tree_sitter::Node<'a>, content: &'a [u8]) -> Option<&'a str> {
    for _ in 0..128 {
        let parent = node.parent()?;
        if matches!(
            parent.kind(),
            "function_item"
                | "function_definition"
                | "function_declaration"
                | "generator_function_declaration"
                | "method_definition"
        ) {
            return parent
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(content).ok())
                .filter(|s| s.len() <= 160 && !s.chars().any(char::is_control));
        }
        if matches!(
            parent.kind(),
            "arrow_function" | "function_expression" | "generator_function"
        ) {
            let directly_named = parent
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(content).ok());
            let assigned = parent
                .parent()
                .and_then(|binding| assigned_function_name(binding, content).map(|(name, _)| name));
            return directly_named
                .or(assigned)
                .filter(|s| s.len() <= 160 && !s.chars().any(char::is_control));
        }
        // Do not attribute calls in anonymous lambdas/closures to an outer function.
        if matches!(parent.kind(), "lambda" | "closure_expression") {
            return None;
        }
        node = parent;
    }
    None
}

pub fn lookup(root: &Path, query: &str) -> Result<Value> {
    scan(root, query, Lookup::Declarations)
}
pub fn callers(root: &Path, query: &str) -> Result<Value> {
    scan(root, query, Lookup::Calls)
}

fn scan(root: &Path, query: &str, mode: Lookup) -> Result<Value> {
    let query = query.trim();
    if query.is_empty() || query.len() > 100 || query.chars().any(char::is_control) {
        return Err("symbol query must contain 1 to 100 bytes without control characters".into());
    }
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
        let Some(lang) = language(&path) else {
            continue;
        };
        if files_scanned >= MAX_FILES || nodes_seen >= MAX_NODES {
            truncated = true;
            break;
        }
        let absolute = root.join(&path);
        let metadata = match fs::symlink_metadata(&absolute) {
            Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() <= MAX_FILE_BYTES => m,
            _ => {
                skipped_files += 1;
                continue;
            }
        };
        if bytes_scanned.saturating_add(metadata.len()) > MAX_TOTAL_BYTES {
            truncated = true;
            break;
        }
        let mut content = Vec::new();
        let mut file = match File::open(&absolute) {
            Ok(file) => file,
            Err(_) => {
                skipped_files += 1;
                continue;
            }
        };
        if Read::by_ref(&mut file)
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut content)
            .is_err()
            || content.len() as u64 > MAX_FILE_BYTES
            || std::str::from_utf8(&content).is_err()
            || content.contains(&0)
        {
            skipped_files += 1;
            continue;
        }
        if bytes_scanned.saturating_add(content.len() as u64) > MAX_TOTAL_BYTES {
            truncated = true;
            break;
        }
        files_scanned += 1;
        bytes_scanned += content.len() as u64;
        parser.set_language(&lang)?;
        let Some(tree) = parser.parse(&content, None) else {
            parse_errors += 1;
            continue;
        };
        if tree.root_node().has_error() {
            parse_errors += 1;
        }
        let mut pending = vec![tree.root_node()];
        while let Some(node) = pending.pop() {
            if nodes_seen >= MAX_NODES {
                truncated = true;
                break;
            }
            nodes_seen += 1;
            match mode {
                Lookup::Declarations if declaration(node.kind()) => {
                    if let Some(name) = node
                        .child_by_field_name("name")
                        .and_then(|n| n.utf8_text(&content).ok())
                    {
                        if name.len() <= 160
                            && !name.chars().any(char::is_control)
                            && name.to_lowercase().contains(&needle)
                        {
                            results.push(json!({"path":path.to_string_lossy(),"name":name,"kind":node.kind(),"line":node.start_position().row + 1}));
                        }
                    }
                }
                Lookup::Declarations if node.kind() == "variable_declarator" => {
                    if let Some((name, kind)) = assigned_function_name(node, &content) {
                        if name.len() <= 160
                            && !name.chars().any(char::is_control)
                            && name.to_lowercase().contains(&needle)
                        {
                            results.push(json!({"path":path.to_string_lossy(),"name":name,"kind":kind,"line":node.start_position().row + 1}));
                        }
                    }
                }
                Lookup::Calls if matches!(node.kind(), "call" | "call_expression") => {
                    if let Some(name) = node
                        .child_by_field_name("function")
                        .and_then(|callee| callable_name(callee, &content))
                    {
                        if name == query && name.len() <= 160 && !name.chars().any(char::is_control)
                        {
                            results.push(json!({"path":path.to_string_lossy(),"callee":name,"caller":enclosing_function(node, &content),"line":node.start_position().row + 1}));
                        }
                    }
                }
                _ => (),
            }
            if results.len() >= MAX_MATCHES {
                truncated = true;
                break;
            }
            let mut cursor = node.walk();
            let children: Vec<_> = node.named_children(&mut cursor).collect();
            pending.extend(children.into_iter().rev());
        }
        if truncated {
            break;
        }
    }
    let mut value = json!({"version":1,"method":match mode { Lookup::Declarations => "tree-sitter declarations", Lookup::Calls => "tree-sitter call sites" },"query":query,"symbols_are_untrusted":true,"files_scanned":files_scanned,"parse_error_files":parse_errors,"skipped_files":skipped_files,"truncated":truncated});
    value[match mode {
        Lookup::Declarations => "symbols",
        Lookup::Calls => "calls",
    }] = json!(results);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn syntax_comparison_is_advisory_and_bounded() {
        let rust = Path::new("main.rs");
        let clean = "fn main() {}";
        let broken = "fn main( {}";
        let introduced = compare_syntax(rust, clean, broken);
        assert_eq!(introduced["status"], "checked");
        assert_eq!(introduced["beforeHasErrors"], false);
        assert_eq!(introduced["afterHasErrors"], true);
        assert!(introduced["beforeIssues"].as_array().unwrap().is_empty());
        let issues = introduced["afterIssues"].as_array().unwrap();
        assert!(!issues.is_empty() && issues.len() <= 8);
        assert!(issues
            .iter()
            .all(|issue| issue["line"].as_u64().unwrap() >= 1
                && issue["byteColumn"].as_u64().unwrap() >= 1));
        assert!(!introduced.to_string().contains("fn main"));
        let fixed = compare_syntax(rust, broken, clean);
        assert_eq!(fixed["beforeHasErrors"], true);
        assert_eq!(fixed["afterHasErrors"], false);
        assert_eq!(
            compare_syntax(Path::new("note.txt"), "a", "b")["status"],
            "unsupported"
        );
        assert_eq!(
            compare_syntax(rust, &"x".repeat(65537), clean)["status"],
            "too_large"
        );
        for path in ["a.py", "a.js", "a.ts", "a.tsx"] {
            assert_eq!(compare_syntax(Path::new(path), "", "")["status"], "checked");
        }
    }

    #[test]
    fn recognizes_declarations_without_comment_false_positives() {
        let root = std::env::temp_dir().join(format!("isa-symbols-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("source.rs"),
            "// fn fake_parse() {}\nfn parse_input() {}\nstruct Parser {}\n",
        )
        .unwrap();
        fs::write(
            root.join("source.py"),
            "class Parser:\n    def parse_file(self):\n        pass\n",
        )
        .unwrap();
        fs::write(
            root.join("source.js"),
            "function parseRequest() {}\nclass Parser {}\n",
        )
        .unwrap();
        fs::write(root.join(".env"), "fn parse_secret() {}").unwrap();
        let value = lookup(&root, "parse").unwrap();
        let items = value["symbols"].as_array().unwrap();
        assert!(items
            .iter()
            .any(|s| s["name"] == "parse_input" && s["line"] == 2));
        assert!(items
            .iter()
            .any(|s| s["name"] == "parse_file" && s["line"] == 2));
        assert!(items.iter().any(|s| s["name"] == "parseRequest"));
        assert!(!items
            .iter()
            .any(|s| s["name"] == "fake_parse" || s["name"] == "parse_secret"));
        assert!(lookup(&root, "").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn finds_syntactic_call_sites_and_enclosing_functions() {
        let root = std::env::temp_dir().join(format!("isa-callers-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("a.rs"),
            "// parse();\nfn work() { parse(); obj.parse(); }\n",
        )
        .unwrap();
        fs::write(
            root.join("b.py"),
            "def work():\n    parse()\n    obj.parse()\n    data['parse']()\n",
        )
        .unwrap();
        fs::write(
            root.join("c.js"),
            "function work() { parse(); obj.parse(); obj['parse'](); }\n",
        )
        .unwrap();
        let data = callers(&root, "parse").unwrap();
        let calls = data["calls"].as_array().unwrap();
        assert_eq!(calls.len(), 6);
        assert!(calls
            .iter()
            .all(|item| item["callee"] == "parse" && item["caller"] == "work"));
        assert_eq!(
            calls.iter().filter(|item| item["path"] == "b.py").count(),
            2
        );
        assert!(callers(&root, "PARSE").unwrap()["calls"]
            .as_array()
            .unwrap()
            .is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn indexes_typescript_tsx_and_named_function_expressions() {
        let root = std::env::temp_dir().join(format!("isa-ts-symbols-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("parser.ts"), "interface ParseConfig { strict: boolean }\ntype ParseResult = string;\nconst parseFile = (x: string): string => normalize(x);\nconst wrap = function (x: string) { return normalize(x); };\nconst value = normalize('x');\n").unwrap();
        fs::write(
            root.join("view.tsx"),
            "const View = () => <div>{normalize('x')}</div>;\n",
        )
        .unwrap();
        fs::write(root.join("other.js"), "function outer() { items.map(() => normalize('x')); }\nconst run = () => normalize('x');\n").unwrap();
        let result = lookup(&root, "parse").unwrap();
        let items = result["symbols"].as_array().unwrap();
        assert_eq!(result["parse_error_files"], 0);
        assert!(items
            .iter()
            .any(|s| s["name"] == "ParseConfig" && s["kind"] == "interface_declaration"));
        assert!(items
            .iter()
            .any(|s| s["name"] == "ParseResult" && s["kind"] == "type_alias_declaration"));
        assert!(items
            .iter()
            .any(|s| s["name"] == "parseFile" && s["kind"] == "arrow_function"));
        let calls = callers(&root, "normalize").unwrap();
        let calls = calls["calls"].as_array().unwrap();
        assert!(calls.iter().any(|c| c["caller"] == "parseFile"));
        assert!(calls.iter().any(|c| c["caller"] == "wrap"));
        assert!(calls.iter().any(|c| c["caller"] == "View"));
        assert!(calls.iter().any(|c| c["caller"] == "run"));
        assert!(calls
            .iter()
            .any(|c| c["path"] == "other.js" && c["caller"].is_null()));
        assert!(!calls
            .iter()
            .any(|c| c["path"] == "other.js" && c["caller"] == "outer"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn call_lookup_marks_truncation_and_unknown_caller() {
        let root = std::env::temp_dir().join(format!("isa-caller-limits-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("calls.py"), "parse()\n".repeat(50)).unwrap();
        let data = callers(&root, "parse").unwrap();
        assert_eq!(data["calls"].as_array().unwrap().len(), MAX_MATCHES);
        assert_eq!(data["truncated"], true);
        assert!(data["calls"][0]["caller"].is_null());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn limits_matches_and_ignores_unsupported_extensions() {
        let root = std::env::temp_dir().join(format!("isa-symbol-limits-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("source.rs"),
            (0..50)
                .map(|i| format!("fn match_{i}() {{}}\n"))
                .collect::<String>(),
        )
        .unwrap();
        fs::write(root.join("ignored.txt"), "fn match_text() {}\n").unwrap();
        let result = lookup(&root, "match_").unwrap();
        assert_eq!(result["symbols"].as_array().unwrap().len(), MAX_MATCHES);
        assert_eq!(result["truncated"], true);
        fs::remove_dir_all(root).unwrap();
    }
}
