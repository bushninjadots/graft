//! Extract Studio: read-only repository analysis and deliberately validated
//! extraction. Nothing here writes until a plan has been checked and the
//! caller has confirmed; every destructive step is preceded by a backup.

use crate::db::{now, Db};
use serde::Serialize;
use std::cmp::Ordering as CmpOrdering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use tauri::{Manager, State};

const MAX_FILES: usize = 12_000;
const MAX_RETURNED_FILES: usize = 2_000;
const MAX_PARSE_BYTES: u64 = 512 * 1024;
const MAX_LINE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_SELECTED: usize = 5_000;
const MAX_EDGES: usize = 60_000;
const MAX_ANALYSIS_FILES: usize = 500;
const MAX_REFS: usize = 150;
const MAX_ACTIONS: usize = 160;

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "dist",
    "build",
    "vendor",
    ".venv",
    "venv",
    "env",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    ".turbo",
    ".next",
    ".nuxt",
    "coverage",
    "out",
    ".gradle",
    ".idea",
    ".cache",
    "Pods",
    "DerivedData",
];

const MARKERS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "setup.py",
    "go.mod",
    "CMakeLists.txt",
    "Makefile",
    "build.gradle",
    "pom.xml",
    "Gemfile",
    "composer.json",
    "mix.exs",
    "flake.nix",
    "Dockerfile",
    "rust-toolchain.toml",
    "tsconfig.json",
    "pnpm-workspace.yaml",
    "deno.json",
    "meson.build",
];

// ---------------------------------------------------------------------------
// Public shapes
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtractFile {
    pub path: String,
    pub language: String,
    pub bytes: u64,
    pub lines: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LanguageStat {
    pub language: String,
    pub files: usize,
    pub bytes: u64,
    pub lines: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractScan {
    pub source: String,
    pub is_git_repo: bool,
    pub branch: Option<String>,
    pub total_files: usize,
    pub total_bytes: u64,
    pub total_lines: u64,
    pub files: Vec<ExtractFile>,
    pub truncated: bool,
    pub languages: Vec<LanguageStat>,
    pub markers: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractComponent {
    pub path: String,
    pub files: usize,
    pub bytes: u64,
    pub lines: u64,
    pub languages: Vec<String>,
    pub internal_references: usize,
    pub incoming_references: usize,
    pub outgoing_references: usize,
    pub external_deps: usize,
    pub cohesion: f64,
    pub score: f64,
    pub has_manifest: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtractRef {
    pub from: String,
    pub to: String,
    pub specifier: String,
    pub line: usize,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExternalDep {
    pub name: String,
    pub files: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractAnalysis {
    pub source: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub total_lines: u64,
    pub languages: Vec<LanguageStat>,
    pub internal_references: usize,
    pub incoming_references: usize,
    pub outgoing_references: usize,
    pub external_deps: Vec<ExternalDep>,
    pub cohesion: f64,
    pub files: Vec<ExtractFile>,
    pub incoming: Vec<ExtractRef>,
    pub warnings: Vec<String>,
    pub scan_warnings: usize,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtractAction {
    pub kind: String,
    pub path: String,
    pub detail: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtractCheck {
    pub id: String,
    pub label: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractPlan {
    pub source: String,
    pub target: String,
    pub name: String,
    pub strategy: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub selection: Vec<String>,
    pub actions: Vec<ExtractAction>,
    pub warnings: Vec<String>,
    pub checks: Vec<ExtractCheck>,
    pub requires_backup: bool,
    pub backup_path: Option<String>,
    pub history_prefix: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractResult {
    pub ok: bool,
    pub strategy: String,
    pub target: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub backup_path: Option<String>,
    pub checks: Vec<ExtractCheck>,
    pub message: String,
    pub run_id: Option<i64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionRun {
    pub id: i64,
    pub source: String,
    pub target: String,
    pub name: String,
    pub strategy: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub backup_path: Option<String>,
    pub status: String,
    pub message: String,
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// Internal shapes
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ScannedFile {
    rel: String,
    language: String,
    bytes: u64,
    lines: u64,
}

struct ScanData {
    files: Vec<ScannedFile>,
    markers: Vec<String>,
    warnings: Vec<String>,
    truncated: bool,
}

#[derive(Clone)]
struct Edge {
    from: String,
    to: String,
    specifier: String,
    line: usize,
}

struct RepoGraph {
    root: PathBuf,
    files: Vec<ScannedFile>,
    index: HashMap<String, usize>,
    edges: Vec<Edge>,
    externals: Vec<(String, String)>,
    warnings: Vec<String>,
    markers: Vec<String>,
    truncated: bool,
    is_git: bool,
    branch: Option<String>,
    dirty: bool,
    toplevel: Option<PathBuf>,
    rust_roots: Vec<(String, String)>,
    python_modules: HashMap<String, String>,
    go_module: Option<String>,
    go_packages: HashMap<String, Vec<String>>,
}

struct ParsedImport {
    spec: String,
    line: usize,
}

struct Resolved {
    targets: Vec<String>,
    external: Option<String>,
    unresolved: bool,
}

impl Resolved {
    fn targets(targets: Vec<String>) -> Self {
        Self {
            targets,
            external: None,
            unresolved: false,
        }
    }

    fn external(name: &str) -> Self {
        Self {
            targets: Vec::new(),
            external: Some(name.to_string()),
            unresolved: false,
        }
    }

    fn unresolved() -> Self {
        Self {
            targets: Vec::new(),
            external: None,
            unresolved: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn extract_scan(source: String) -> Result<ExtractScan, String> {
    let root = resolve_source(&source)?;
    let scan = scan_files(&root);
    let (is_git, branch, _, _) = git_info(&root);

    let mut languages: BTreeMap<String, (usize, u64, u64)> = BTreeMap::new();
    let mut total_bytes = 0u64;
    let mut total_lines = 0u64;
    for file in &scan.files {
        let entry = languages.entry(file.language.clone()).or_insert((0, 0, 0));
        entry.0 += 1;
        entry.1 += file.bytes;
        entry.2 += file.lines;
        total_bytes += file.bytes;
        total_lines += file.lines;
    }

    let mut files: Vec<ExtractFile> = scan
        .files
        .iter()
        .map(|file| ExtractFile {
            path: file.rel.clone(),
            language: file.language.clone(),
            bytes: file.bytes,
            lines: file.lines,
        })
        .collect();
    let truncated = scan.truncated || files.len() > MAX_RETURNED_FILES;
    files.truncate(MAX_RETURNED_FILES);

    let mut languages: Vec<LanguageStat> = languages
        .into_iter()
        .map(|(language, (files, bytes, lines))| LanguageStat {
            language,
            files,
            bytes,
            lines,
        })
        .collect();
    languages.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.language.cmp(&b.language)));

    Ok(ExtractScan {
        source: root.to_string_lossy().to_string(),
        is_git_repo: is_git,
        branch,
        total_files: scan.files.len(),
        total_bytes,
        total_lines,
        files,
        truncated,
        languages,
        markers: scan.markers,
        warnings: scan.warnings,
    })
}

#[tauri::command]
pub fn extract_suggest(source: String) -> Result<Vec<ExtractComponent>, String> {
    let graph = build_graph(&source)?;
    Ok(suggest_components(&graph))
}

#[tauri::command]
pub fn extract_analyze(source: String, selection: Vec<String>) -> Result<ExtractAnalysis, String> {
    let graph = build_graph(&source)?;
    analyze_graph(&graph, &selection)
}

#[tauri::command]
pub fn extract_plan(
    app: tauri::AppHandle,
    source: String,
    selection: Vec<String>,
    strategy: String,
    target: String,
    name: String,
) -> Result<ExtractPlan, String> {
    let backup_root = backup_root(&app)?;
    let graph = build_graph(&source)?;
    plan_for_graph(&graph, &backup_root, &selection, &strategy, &target, &name)
}

#[tauri::command]
pub fn extract_execute(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    source: String,
    selection: Vec<String>,
    strategy: String,
    target: String,
    name: String,
    confirmed: bool,
) -> Result<ExtractResult, String> {
    let backup_root = backup_root(&app)?;
    execute_with(
        &db,
        &backup_root,
        &source,
        &selection,
        &strategy,
        &target,
        &name,
        confirmed,
    )
}

#[tauri::command]
pub fn extract_runs(db: State<'_, Db>) -> Result<Vec<ExtractionRun>, String> {
    let conn =
        db.0.lock()
            .map_err(|_| "The local database is unavailable.".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, source, target, name, strategy, file_count, total_bytes, \
             backup_path, status, message, created_at \
             FROM extraction_runs ORDER BY created_at DESC, id DESC LIMIT 100",
        )
        .map_err(|e| format!("Cannot read extraction history: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ExtractionRun {
                id: row.get(0)?,
                source: row.get(1)?,
                target: row.get(2)?,
                name: row.get(3)?,
                strategy: row.get(4)?,
                file_count: row.get::<_, i64>(5)?.max(0) as usize,
                total_bytes: row.get::<_, i64>(6)?.max(0) as u64,
                backup_path: row.get(7)?,
                status: row.get(8)?,
                message: row.get(9)?,
                created_at: row.get(10)?,
            })
        })
        .map_err(|e| format!("Cannot read extraction history: {e}"))?;
    let mut runs = Vec::new();
    for row in rows {
        runs.push(row.map_err(|e| format!("Cannot read extraction history: {e}"))?);
    }
    Ok(runs)
}

// ---------------------------------------------------------------------------
// Source scanning
// ---------------------------------------------------------------------------

fn resolve_source(source: &str) -> Result<PathBuf, String> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Err("Choose a source repository.".to_string());
    }
    let root =
        fs::canonicalize(trimmed).map_err(|e| format!("Cannot open source {trimmed}: {e}"))?;
    if !root.is_dir() {
        return Err(format!("Source is not a directory: {trimmed}"));
    }
    Ok(root)
}

fn language_for(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "rs" => "Rust",
        "ts" | "mts" | "cts" | "tsx" => "TypeScript",
        "js" | "mjs" | "cjs" | "jsx" => "JavaScript",
        "svelte" => "Svelte",
        "py" => "Python",
        "go" => "Go",
        "c" | "h" => "C",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "C++",
        "sh" | "bash" | "zsh" => "Shell",
        _ => "Other",
    }
}

fn is_parseable(language: &str) -> bool {
    matches!(
        language,
        "Rust" | "TypeScript" | "JavaScript" | "Svelte" | "Python" | "Go" | "C" | "C++" | "Shell"
    )
}

fn is_marker(name: &str) -> bool {
    MARKERS.contains(&name)
}

fn scan_files(root: &Path) -> ScanData {
    let mut files: Vec<ScannedFile> = Vec::new();
    let mut markers: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut truncated = false;
    let mut stack: Vec<String> = vec![String::new()];

    while let Some(dir) = stack.pop() {
        if files.len() >= MAX_FILES {
            truncated = true;
            break;
        }
        let abs = if dir.is_empty() {
            root.to_path_buf()
        } else {
            root.join(&dir)
        };
        let entries = match fs::read_dir(&abs) {
            Ok(entries) => entries,
            Err(e) => {
                warnings.push(format!("{dir}: cannot read directory ({e})"));
                continue;
            }
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if file_type.is_symlink() {
                continue;
            }
            let rel = if dir.is_empty() {
                name.clone()
            } else {
                format!("{dir}/{name}")
            };
            if file_type.is_dir() {
                if SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                stack.push(rel);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            if files.len() >= MAX_FILES {
                truncated = true;
                break;
            }
            let bytes = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            let (lines, line_warning) = count_lines(&root.join(&rel), bytes);
            if let Some(warning) = line_warning {
                warnings.push(format!("{rel}: {warning}"));
            }
            if is_marker(&name) {
                markers.push(rel.clone());
            }
            files.push(ScannedFile {
                language: language_for(&name).to_string(),
                rel,
                bytes,
                lines,
            });
        }
    }

    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    markers.sort();
    markers.dedup();
    markers.truncate(80);
    if truncated {
        warnings.push(format!(
            "The scan stopped at {MAX_FILES} files; analysis covers the files listed."
        ));
    }
    ScanData {
        files,
        markers,
        warnings,
        truncated,
    }
}

fn count_lines(path: &Path, bytes: u64) -> (u64, Option<String>) {
    if bytes > MAX_LINE_BYTES {
        return (0, Some("line count skipped (larger than 4 MB)".to_string()));
    }
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(e) => return (0, Some(format!("cannot read file ({e})"))),
    };
    if is_binary(&data) {
        return (0, None);
    }
    let mut lines = data.iter().filter(|byte| **byte == b'\n').count() as u64;
    if !data.is_empty() && !data.ends_with(b"\n") {
        lines += 1;
    }
    (lines, None)
}

fn is_binary(data: &[u8]) -> bool {
    data.iter().take(8192).any(|byte| *byte == 0)
}

// ---------------------------------------------------------------------------
// Import parsing
// ---------------------------------------------------------------------------

fn parse_imports(language: &str, text: &str) -> Vec<ParsedImport> {
    match language {
        "Rust" => parse_rust(text),
        "TypeScript" | "JavaScript" | "Svelte" => parse_js(text),
        "Python" => parse_python(text),
        "Go" => parse_go(text),
        "C" | "C++" => parse_c(text),
        "Shell" => parse_shell(text),
        _ => Vec::new(),
    }
}

fn parse_rust(text: &str) -> Vec<ParsedImport> {
    let mut out = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let mut line = raw.trim();
        for prefix in ["pub(crate) ", "pub(super) ", "pub "] {
            if let Some(rest) = line.strip_prefix(prefix) {
                line = rest;
            }
        }
        if let Some(rest) = line.strip_prefix("use ") {
            let spec = rest
                .split(|c: char| c == '{' || c == ';' || c == ' ' || c == '\t')
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(':');
            if !spec.is_empty() {
                out.push(ParsedImport {
                    spec: format!("u:{spec}"),
                    line: index + 1,
                });
            }
        } else if let Some(rest) = line.strip_prefix("mod ") {
            let name = rest.trim().trim_end_matches(';').trim();
            if !name.is_empty() && !name.contains('{') && !name.contains(' ') {
                out.push(ParsedImport {
                    spec: format!("m:{name}"),
                    line: index + 1,
                });
            }
        } else if let Some(rest) = line.strip_prefix("extern crate ") {
            let name = rest.split(';').next().unwrap_or("").trim();
            if !name.is_empty() {
                out.push(ParsedImport {
                    spec: format!("x:{name}"),
                    line: index + 1,
                });
            }
        }
    }
    out
}

fn first_quoted(text: &str) -> Option<&str> {
    for (index, ch) in text.char_indices() {
        if ch == '\'' || ch == '"' || ch == '`' {
            let rest = &text[index + ch.len_utf8()..];
            if let Some(end) = rest.find(ch) {
                return Some(&rest[..end]);
            }
            return None;
        }
    }
    None
}

fn parse_js(text: &str) -> Vec<ParsedImport> {
    let mut out = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let looks_import = line.starts_with("import ")
            || line.starts_with("export ")
            || line.contains("require(")
            || (line.contains("import") && line.contains(" from "));
        if !looks_import {
            continue;
        }
        if let Some(spec) = first_quoted(line) {
            if !spec.is_empty() {
                out.push(ParsedImport {
                    spec: spec.to_string(),
                    line: index + 1,
                });
            }
        }
    }
    out
}

fn parse_python(text: &str) -> Vec<ParsedImport> {
    let mut out = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("from ") {
            let mut parts = rest.splitn(2, " import ");
            let module = parts.next().unwrap_or("").trim();
            if module.is_empty() {
                continue;
            }
            if module.chars().all(|c| c == '.') {
                if let Some(names) = parts.next() {
                    for name in names.split(',') {
                        let name = name
                            .split(" as ")
                            .next()
                            .unwrap_or("")
                            .trim()
                            .trim_matches(|c| c == '(' || c == ')' || c == '\\');
                        if is_identifier(name) {
                            out.push(ParsedImport {
                                spec: format!("{module}{name}"),
                                line: index + 1,
                            });
                        }
                    }
                } else {
                    out.push(ParsedImport {
                        spec: module.to_string(),
                        line: index + 1,
                    });
                }
            } else {
                out.push(ParsedImport {
                    spec: module.to_string(),
                    line: index + 1,
                });
            }
        } else if let Some(rest) = line.strip_prefix("import ") {
            for part in rest.split(',') {
                let module = part.split(" as ").next().unwrap_or("").trim();
                if is_identifier_path(module) {
                    out.push(ParsedImport {
                        spec: module.to_string(),
                        line: index + 1,
                    });
                }
            }
        }
    }
    out
}

fn is_identifier(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(true)
}

fn is_identifier_path(name: &str) -> bool {
    !name.is_empty() && name.split('.').all(is_identifier)
}

fn parse_go(text: &str) -> Vec<ParsedImport> {
    let mut out = Vec::new();
    let mut block = false;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if block {
            if let Some(spec) = first_quoted(line) {
                out.push(ParsedImport {
                    spec: format!("g:{spec}"),
                    line: index + 1,
                });
            }
            if line.contains(')') {
                block = false;
            }
            continue;
        }
        if line.starts_with("import (") {
            block = true;
            if let Some(spec) = first_quoted(line) {
                out.push(ParsedImport {
                    spec: format!("g:{spec}"),
                    line: index + 1,
                });
            }
        } else if let Some(rest) = line.strip_prefix("import ") {
            if let Some(spec) = first_quoted(rest) {
                out.push(ParsedImport {
                    spec: format!("g:{spec}"),
                    line: index + 1,
                });
            }
        }
    }
    out
}

fn parse_c(text: &str) -> Vec<ParsedImport> {
    let mut out = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let Some(position) = line.find("#include") else {
            continue;
        };
        let rest = &line[position + "#include".len()..];
        let Some(start) = rest.find(|c| c == '"' || c == '<') else {
            continue;
        };
        let opener = rest.as_bytes()[start] as char;
        let closer = if opener == '<' { '>' } else { '"' };
        let after = &rest[start + 1..];
        let Some(end) = after.find(closer) else {
            continue;
        };
        let name = after[..end].trim();
        if name.is_empty() {
            continue;
        }
        let prefix = if opener == '<' { "a:" } else { "q:" };
        out.push(ParsedImport {
            spec: format!("{prefix}{name}"),
            line: index + 1,
        });
    }
    out
}

fn parse_shell(text: &str) -> Vec<ParsedImport> {
    let mut out = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        for prefix in [". ", "source "] {
            let Some(rest) = line.strip_prefix(prefix) else {
                continue;
            };
            if let Some(name) = rest.split_whitespace().next() {
                if !name.is_empty() && !name.starts_with('-') {
                    out.push(ParsedImport {
                        spec: format!("s:{name}"),
                        line: index + 1,
                    });
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Import resolution
// ---------------------------------------------------------------------------

fn parent(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(parent, _)| parent)
}

fn join_rel(base: &str, spec: &str) -> Option<String> {
    if spec.starts_with('/') || spec.starts_with('\\') {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    let base = base.trim_matches('/');
    if !base.is_empty() {
        for part in base.split('/') {
            parts.push(part.to_string());
        }
    }
    for part in spec.replace('\\', "/").split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            if parts.pop().is_none() {
                return None;
            }
        } else {
            parts.push(part.to_string());
        }
    }
    Some(parts.join("/"))
}

fn resolve_import(
    rel: &str,
    language: &str,
    spec: &str,
    index: &HashMap<String, usize>,
    rust_roots: &[(String, String)],
    python_modules: &HashMap<String, String>,
    go_module: Option<&str>,
    go_packages: &HashMap<String, Vec<String>>,
) -> Resolved {
    match language {
        "TypeScript" | "JavaScript" | "Svelte" => resolve_js(rel, spec, index),
        "Rust" => resolve_rust(rel, spec, index, rust_roots),
        "Python" => resolve_python(rel, spec, index, python_modules),
        "Go" => resolve_go(spec, go_module, go_packages),
        "C" | "C++" => resolve_c(rel, spec, index),
        "Shell" => resolve_shell(rel, spec, index),
        _ => Resolved::unresolved(),
    }
}

const JS_EXTENSIONS: &[&str] = &[
    ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".svelte", ".json", ".vue", ".d.ts",
];

fn resolve_js(rel: &str, spec: &str, index: &HashMap<String, usize>) -> Resolved {
    if spec.starts_with('.') {
        let Some(base) = join_rel(parent(rel).unwrap_or(""), spec) else {
            return Resolved::unresolved();
        };
        if index.contains_key(&base) {
            return Resolved::targets(vec![base]);
        }
        for extension in JS_EXTENSIONS {
            let candidate = format!("{base}{extension}");
            if index.contains_key(&candidate) {
                return Resolved::targets(vec![candidate]);
            }
        }
        for extension in JS_EXTENSIONS {
            let candidate = format!("{base}/index{extension}");
            if index.contains_key(&candidate) {
                return Resolved::targets(vec![candidate]);
            }
        }
        return Resolved::unresolved();
    }
    if spec.starts_with('/') || spec.starts_with('#') {
        return Resolved::unresolved();
    }
    Resolved::external(&js_package(spec))
}

fn js_package(spec: &str) -> String {
    let spec = spec.split('?').next().unwrap_or(spec);
    let parts: Vec<&str> = spec.split('/').collect();
    if spec.starts_with('@') && parts.len() >= 2 {
        format!("{}/{}", parts[0], parts[1])
    } else {
        parts.first().copied().unwrap_or(spec).to_string()
    }
}

fn rust_root_for(rel: &str, roots: &[(String, String)]) -> String {
    let mut best: Option<(&str, &str)> = None;
    for (dir, module_root) in roots {
        let matches = dir.is_empty() || rel == dir || rel.starts_with(&format!("{dir}/"));
        if !matches {
            continue;
        }
        if best
            .map(|(current, _)| dir.len() > current.len())
            .unwrap_or(true)
        {
            best = Some((dir, module_root));
        }
    }
    best.map(|(_, module_root)| module_root.to_string())
        .unwrap_or_default()
}

fn rust_module_dir(rel: &str) -> String {
    if rel == "mod.rs" {
        return String::new();
    }
    if let Some(without) = rel.strip_suffix("/mod.rs") {
        return parent(without).unwrap_or("").to_string();
    }
    parent(rel).unwrap_or("").to_string()
}

fn rust_candidates(base: &str, segments: &[&str], index: &HashMap<String, usize>) -> Vec<String> {
    if segments.is_empty() {
        if base.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for candidate in [format!("{base}.rs"), format!("{base}/mod.rs")] {
            if index.contains_key(&candidate) {
                out.push(candidate);
            }
        }
        return out;
    }
    for take in (1..=segments.len()).rev() {
        let Some(path) = join_rel(base, &segments[..take].join("/")) else {
            continue;
        };
        let mut out = Vec::new();
        for candidate in [format!("{path}.rs"), format!("{path}/mod.rs")] {
            if index.contains_key(&candidate) {
                out.push(candidate);
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
    Vec::new()
}

fn resolve_rust(
    rel: &str,
    spec: &str,
    index: &HashMap<String, usize>,
    rust_roots: &[(String, String)],
) -> Resolved {
    if let Some(name) = spec.strip_prefix("x:") {
        return Resolved::external(name);
    }
    if let Some(name) = spec.strip_prefix("m:") {
        let base = rust_module_dir(rel);
        let Some(path) = join_rel(&base, name) else {
            return Resolved::unresolved();
        };
        for candidate in [format!("{path}.rs"), format!("{path}/mod.rs")] {
            if index.contains_key(&candidate) {
                return Resolved::targets(vec![candidate]);
            }
        }
        return Resolved::unresolved();
    }

    let path = spec
        .strip_prefix("u:")
        .unwrap_or(spec)
        .trim_start_matches("::");
    let segments: Vec<&str> = path.split("::").filter(|part| !part.is_empty()).collect();
    if segments.is_empty() {
        return Resolved::unresolved();
    }
    let crate_root = rust_root_for(rel, rust_roots);
    let module_dir = rust_module_dir(rel);

    let candidates = match segments[0] {
        "crate" => rust_candidates(&crate_root, &segments[1..], index),
        "self" => rust_candidates(&module_dir, &segments[1..], index),
        "super" => {
            let mut base = module_dir;
            let mut index_in_segments = 0;
            while index_in_segments < segments.len() && segments[index_in_segments] == "super" {
                base = parent(&base).unwrap_or("").to_string();
                index_in_segments += 1;
            }
            rust_candidates(&base, &segments[index_in_segments..], index)
        }
        _ => {
            let mut found = rust_candidates(&crate_root, &segments, index);
            if found.is_empty() {
                found = rust_candidates(&module_dir, &segments, index);
            }
            found
        }
    };

    if !candidates.is_empty() {
        return Resolved::targets(candidates);
    }
    if matches!(segments[0], "crate" | "self" | "super") {
        Resolved::unresolved()
    } else {
        Resolved::external(segments[0])
    }
}

fn resolve_python(
    rel: &str,
    spec: &str,
    index: &HashMap<String, usize>,
    python_modules: &HashMap<String, String>,
) -> Resolved {
    if spec.starts_with('.') {
        let dots = spec.chars().take_while(|c| *c == '.').count();
        let rest = &spec[dots..];
        let mut base = parent(rel).unwrap_or("").to_string();
        for _ in 1..dots {
            match parent(&base) {
                Some(parent) => base = parent.to_string(),
                None => return Resolved::unresolved(),
            }
        }
        let candidates = if rest.is_empty() {
            vec![format!("{base}/__init__.py")]
        } else {
            let Some(path) = join_rel(&base, &rest.replace('.', "/")) else {
                return Resolved::unresolved();
            };
            vec![format!("{path}.py"), format!("{path}/__init__.py")]
        };
        for candidate in candidates {
            if index.contains_key(&candidate) {
                return Resolved::targets(vec![candidate]);
            }
        }
        return Resolved::unresolved();
    }

    let path = spec.replace('.', "/");
    for base in ["", "src"] {
        let Some(candidate_base) = join_rel(base, &path) else {
            continue;
        };
        for candidate in [
            format!("{candidate_base}.py"),
            format!("{candidate_base}/__init__.py"),
        ] {
            if index.contains_key(&candidate) {
                return Resolved::targets(vec![candidate]);
            }
        }
    }
    if let Some(module) = python_modules.get(spec) {
        return Resolved::targets(vec![module.clone()]);
    }
    let segments: Vec<&str> = spec.split('.').collect();
    for skip in 1..segments.len() {
        let suffix = segments[skip..].join(".");
        if let Some(module) = python_modules.get(&suffix) {
            return Resolved::targets(vec![module.clone()]);
        }
    }
    let first = segments.first().copied().unwrap_or(spec);
    Resolved::external(first)
}

fn resolve_go(
    spec: &str,
    go_module: Option<&str>,
    go_packages: &HashMap<String, Vec<String>>,
) -> Resolved {
    let Some(spec) = spec.strip_prefix("g:") else {
        return Resolved::unresolved();
    };
    if let Some(module) = go_module {
        let module = module.trim();
        if !module.is_empty() && (spec == module || spec.starts_with(&format!("{module}/"))) {
            let dir = spec
                .strip_prefix(module)
                .unwrap_or("")
                .trim_start_matches('/');
            if let Some(files) = go_packages.get(dir) {
                if !files.is_empty() {
                    return Resolved::targets(files.clone());
                }
            }
            return Resolved::unresolved();
        }
    }
    Resolved::external(spec)
}

fn resolve_c(rel: &str, spec: &str, index: &HashMap<String, usize>) -> Resolved {
    if let Some(header) = spec.strip_prefix("a:") {
        return Resolved::external(header);
    }
    let Some(header) = spec.strip_prefix("q:") else {
        return Resolved::unresolved();
    };
    let Some(candidate) = join_rel(parent(rel).unwrap_or(""), header) else {
        return Resolved::external(header);
    };
    if index.contains_key(&candidate) {
        return Resolved::targets(vec![candidate]);
    }
    Resolved::external(header)
}

fn resolve_shell(rel: &str, spec: &str, index: &HashMap<String, usize>) -> Resolved {
    let Some(name) = spec.strip_prefix("s:") else {
        return Resolved::unresolved();
    };
    if name.starts_with('/') || name.starts_with('~') {
        return Resolved::external(name);
    }
    let Some(base) = join_rel(parent(rel).unwrap_or(""), name) else {
        return Resolved::external(name);
    };
    for candidate in [base.clone(), format!("{base}.sh")] {
        if index.contains_key(&candidate) {
            return Resolved::targets(vec![candidate]);
        }
    }
    Resolved::external(name)
}

// ---------------------------------------------------------------------------
// Graph construction
// ---------------------------------------------------------------------------

fn git_info(root: &Path) -> (bool, Option<String>, bool, Option<PathBuf>) {
    let inside = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
        })
        .unwrap_or(false);
    if !inside {
        return (false, None, false, None);
    }
    let branch = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|branch| !branch.is_empty() && branch != "HEAD");
    let dirty = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false);
    let toplevel = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                None
            } else {
                fs::canonicalize(path).ok()
            }
        });
    (true, branch, dirty, toplevel)
}

fn build_graph(source: &str) -> Result<RepoGraph, String> {
    let root = resolve_source(source)?;
    let scan = scan_files(&root);
    let (is_git, branch, dirty, toplevel) = git_info(&root);

    let mut index = HashMap::new();
    for (position, file) in scan.files.iter().enumerate() {
        index.insert(file.rel.clone(), position);
    }

    let mut rust_roots: Vec<(String, String)> = Vec::new();
    for marker in &scan.markers {
        if marker != "Cargo.toml" && !marker.ends_with("/Cargo.toml") {
            continue;
        }
        let dir = parent(marker).unwrap_or("");
        let src_prefix = if dir.is_empty() {
            "src/".to_string()
        } else {
            format!("{dir}/src/")
        };
        let has_src = scan
            .files
            .iter()
            .any(|file| file.rel.starts_with(&src_prefix));
        let module_root = if has_src {
            src_prefix.trim_end_matches('/').to_string()
        } else {
            dir.to_string()
        };
        rust_roots.push((dir.to_string(), module_root));
    }

    let mut python_modules: HashMap<String, String> = HashMap::new();
    for file in &scan.files {
        if file.language != "Python" {
            continue;
        }
        let mut module = file.rel.trim_end_matches(".py").to_string();
        if module == "__init__" {
            module.clear();
        } else if let Some(stripped) = module.strip_suffix("/__init__") {
            module = stripped.to_string();
        }
        if module.is_empty() {
            continue;
        }
        let dotted = module.replace('/', ".");
        python_modules
            .entry(dotted.clone())
            .or_insert_with(|| file.rel.clone());
        let segments: Vec<&str> = dotted.split('.').collect();
        for take in [3usize, 2] {
            if segments.len() > take {
                let suffix = segments[segments.len() - take..].join(".");
                python_modules
                    .entry(suffix)
                    .or_insert_with(|| file.rel.clone());
            }
        }
    }

    let mut go_module: Option<String> = None;
    let mut go_packages: HashMap<String, Vec<String>> = HashMap::new();
    for file in &scan.files {
        if file.language == "Go" {
            let dir = parent(&file.rel).unwrap_or("").to_string();
            go_packages.entry(dir).or_default().push(file.rel.clone());
        }
    }
    if index.contains_key("go.mod") {
        if let Ok(text) = fs::read_to_string(root.join("go.mod")) {
            for line in text.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("module ") {
                    let module = rest.trim().trim_matches('"');
                    if !module.is_empty() {
                        go_module = Some(module.to_string());
                    }
                    break;
                }
            }
        }
    }

    let mut graph = RepoGraph {
        root: root.clone(),
        files: scan.files,
        index,
        edges: Vec::new(),
        externals: Vec::new(),
        warnings: scan.warnings,
        markers: scan.markers,
        truncated: scan.truncated,
        is_git,
        branch,
        dirty,
        toplevel,
        rust_roots,
        python_modules,
        go_module,
        go_packages,
    };

    for position in 0..graph.files.len() {
        if graph.edges.len() >= MAX_EDGES {
            graph
                .warnings
                .push("Reference graph stopped at the analysis limit.".to_string());
            break;
        }
        let file = graph.files[position].clone();
        if !is_parseable(&file.language) || file.bytes > MAX_PARSE_BYTES {
            continue;
        }
        let data = match fs::read(root.join(&file.rel)) {
            Ok(data) => data,
            Err(_) => continue,
        };
        if is_binary(&data) {
            continue;
        }
        let text = String::from_utf8_lossy(&data);
        for parsed in parse_imports(&file.language, &text) {
            let resolved = resolve_import(
                &file.rel,
                &file.language,
                &parsed.spec,
                &graph.index,
                &graph.rust_roots,
                &graph.python_modules,
                graph.go_module.as_deref(),
                &graph.go_packages,
            );
            if !resolved.targets.is_empty() {
                for target in resolved.targets {
                    if graph.edges.len() >= MAX_EDGES {
                        break;
                    }
                    graph.edges.push(Edge {
                        from: file.rel.clone(),
                        to: target,
                        specifier: parsed.spec.clone(),
                        line: parsed.line,
                    });
                }
            } else if let Some(dependency) = resolved.external {
                graph.externals.push((file.rel.clone(), dependency));
            } else if resolved.unresolved && graph.warnings.len() < 120 {
                graph.warnings.push(format!(
                    "{}:{} unresolved local import {}",
                    file.rel,
                    parsed.line,
                    parsed
                        .spec
                        .trim_start_matches("u:")
                        .trim_start_matches("g:")
                ));
            }
        }
    }

    Ok(graph)
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

fn normalize_selection(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "." {
        return Ok(String::new());
    }
    if trimmed.starts_with('/') {
        return Err(format!("Selection must be relative: {trimmed}"));
    }
    join_rel("", trimmed)
        .ok_or_else(|| format!("Selection escapes the source repository: {trimmed}"))
}

fn expand_selection(files: &[ScannedFile], selection: &[String]) -> Result<Vec<String>, String> {
    let mut selected: BTreeSet<String> = BTreeSet::new();
    for raw in selection {
        let rel = normalize_selection(raw)?;
        let mut found = false;
        if rel.is_empty() {
            for file in files {
                selected.insert(file.rel.clone());
                found = true;
            }
        } else {
            let prefix = format!("{rel}/");
            for file in files {
                if file.rel == rel || file.rel.starts_with(&prefix) {
                    selected.insert(file.rel.clone());
                    found = true;
                }
            }
        }
        if !found {
            return Err(format!("No scanned file matches: {raw}"));
        }
        if selected.len() > MAX_SELECTED {
            return Err(format!(
                "Select at most {MAX_SELECTED} files in one extraction."
            ));
        }
    }
    if selected.is_empty() {
        return Err("Select at least one file or folder.".to_string());
    }
    Ok(selected.into_iter().collect())
}

fn file_info(file: &ScannedFile) -> ExtractFile {
    ExtractFile {
        path: file.rel.clone(),
        language: file.language.clone(),
        bytes: file.bytes,
        lines: file.lines,
    }
}

fn analyze_graph(graph: &RepoGraph, selection: &[String]) -> Result<ExtractAnalysis, String> {
    let selected = expand_selection(&graph.files, selection)?;
    let set: HashSet<&str> = selected.iter().map(String::as_str).collect();

    let mut bytes = 0u64;
    let mut lines = 0u64;
    let mut languages: BTreeMap<String, (usize, u64, u64)> = BTreeMap::new();
    let mut files = Vec::new();
    for file in &graph.files {
        if !set.contains(file.rel.as_str()) {
            continue;
        }
        bytes += file.bytes;
        lines += file.lines;
        let entry = languages.entry(file.language.clone()).or_insert((0, 0, 0));
        entry.0 += 1;
        entry.1 += file.bytes;
        entry.2 += file.lines;
        if files.len() < MAX_ANALYSIS_FILES {
            files.push(file_info(file));
        }
    }

    let mut internal_pairs: HashSet<(String, String)> = HashSet::new();
    let mut outgoing_pairs: HashSet<(String, String)> = HashSet::new();
    let mut incoming_pairs: HashMap<(String, String), Edge> = HashMap::new();
    for edge in &graph.edges {
        let from_inside = set.contains(edge.from.as_str());
        let to_inside = set.contains(edge.to.as_str());
        if from_inside && to_inside {
            internal_pairs.insert((edge.from.clone(), edge.to.clone()));
        } else if from_inside {
            outgoing_pairs.insert((edge.from.clone(), edge.to.clone()));
        } else if to_inside {
            incoming_pairs
                .entry((edge.from.clone(), edge.to.clone()))
                .or_insert_with(|| edge.clone());
        }
    }

    let mut external_map: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    for (rel, dependency) in &graph.externals {
        if set.contains(rel.as_str()) {
            external_map
                .entry(dependency.clone())
                .or_default()
                .insert(rel.clone());
        }
    }
    let mut external_deps: Vec<ExternalDep> = external_map
        .into_iter()
        .map(|(name, files)| ExternalDep {
            name,
            files: files.len(),
        })
        .collect();
    external_deps.sort_by(|a, b| b.files.cmp(&a.files).then(a.name.cmp(&b.name)));

    let internal = internal_pairs.len();
    let outgoing = outgoing_pairs.len();
    let denominator = internal + outgoing;
    let cohesion = if denominator == 0 {
        0.5
    } else {
        internal as f64 / denominator as f64
    };

    let incoming_count = incoming_pairs.len();
    let mut incoming: Vec<ExtractRef> = incoming_pairs
        .into_values()
        .map(|edge| ExtractRef {
            from: edge.from,
            to: edge.to,
            specifier: edge.specifier,
            line: edge.line,
        })
        .collect();
    incoming.sort_by(|a, b| a.from.cmp(&b.from).then(a.to.cmp(&b.to)));
    incoming.truncate(MAX_REFS);

    let mut languages: Vec<LanguageStat> = languages
        .into_iter()
        .map(|(language, (files, bytes, lines))| LanguageStat {
            language,
            files,
            bytes,
            lines,
        })
        .collect();
    languages.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.language.cmp(&b.language)));

    Ok(ExtractAnalysis {
        source: graph.root.to_string_lossy().to_string(),
        file_count: selected.len(),
        total_bytes: bytes,
        total_lines: lines,
        languages,
        internal_references: internal,
        incoming_references: incoming_count,
        outgoing_references: outgoing,
        external_deps,
        cohesion: (cohesion * 1000.0).round() / 1000.0,
        files,
        incoming,
        warnings: graph.warnings.clone(),
        scan_warnings: graph.warnings.len(),
    })
}

fn suggest_components(graph: &RepoGraph) -> Vec<ExtractComponent> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for file in &graph.files {
        let mut parts: Vec<&str> = file.rel.split('/').collect();
        parts.pop();
        let mut accumulated = String::new();
        for part in parts {
            if !accumulated.is_empty() {
                accumulated.push('/');
            }
            accumulated.push_str(part);
            *counts.entry(accumulated.clone()).or_insert(0) += 1;
        }
    }

    let mut candidates: Vec<(String, usize)> = counts
        .into_iter()
        .filter(|(dir, count)| !dir.is_empty() && *count >= 2 && *count <= 800)
        .collect();
    candidates.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    candidates.truncate(700);

    let mut outgoing_edges: HashMap<&str, Vec<&Edge>> = HashMap::new();
    let mut incoming_edges: HashMap<&str, Vec<&Edge>> = HashMap::new();
    for edge in &graph.edges {
        outgoing_edges
            .entry(edge.from.as_str())
            .or_default()
            .push(edge);
        incoming_edges
            .entry(edge.to.as_str())
            .or_default()
            .push(edge);
    }

    let mut components = Vec::new();
    for (dir, _) in candidates {
        let prefix = format!("{dir}/");
        let set: HashSet<&str> = graph
            .files
            .iter()
            .filter(|file| file.rel.starts_with(&prefix))
            .map(|file| file.rel.as_str())
            .collect();
        if set.len() < 2 {
            continue;
        }

        let mut bytes = 0u64;
        let mut lines = 0u64;
        let mut languages: BTreeSet<String> = BTreeSet::new();
        for file in &graph.files {
            if set.contains(file.rel.as_str()) {
                bytes += file.bytes;
                lines += file.lines;
                languages.insert(file.language.clone());
            }
        }

        let mut internal_pairs: HashSet<(&str, &str)> = HashSet::new();
        let mut outgoing_pairs: HashSet<(&str, &str)> = HashSet::new();
        let mut incoming_pairs: HashSet<(&str, &str)> = HashSet::new();
        for rel in &set {
            if let Some(edges) = outgoing_edges.get(rel) {
                for edge in edges {
                    let pair = (edge.from.as_str(), edge.to.as_str());
                    if set.contains(edge.to.as_str()) {
                        internal_pairs.insert(pair);
                    } else {
                        outgoing_pairs.insert(pair);
                    }
                }
            }
            if let Some(edges) = incoming_edges.get(rel) {
                for edge in edges {
                    if !set.contains(edge.from.as_str()) {
                        incoming_pairs.insert((edge.from.as_str(), edge.to.as_str()));
                    }
                }
            }
        }

        let mut external: HashSet<&str> = HashSet::new();
        for (rel, dependency) in &graph.externals {
            if set.contains(rel.as_str()) {
                external.insert(dependency.as_str());
            }
        }

        let internal = internal_pairs.len();
        let outgoing = outgoing_pairs.len();
        let denominator = internal + outgoing;
        let cohesion = if denominator == 0 {
            0.5
        } else {
            internal as f64 / denominator as f64
        };
        let has_manifest = graph
            .markers
            .iter()
            .any(|marker| parent(marker).unwrap_or("") == dir.as_str());
        let manifest_bonus = if has_manifest { 2.0 } else { 0.0 };
        let size_factor =
            ((set.len() as f64) + 1.0).ln() * (1.0 + (bytes as f64 / 32_000.0).ln().max(0.0));
        let score = cohesion * size_factor + manifest_bonus;

        components.push(ExtractComponent {
            path: dir,
            files: set.len(),
            bytes,
            lines,
            languages: languages.into_iter().collect(),
            internal_references: internal,
            incoming_references: incoming_pairs.len(),
            outgoing_references: outgoing,
            external_deps: external.len(),
            cohesion: (cohesion * 1000.0).round() / 1000.0,
            score: (score * 1000.0).round() / 1000.0,
            has_manifest,
        });
    }

    components.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(CmpOrdering::Equal)
            .then(b.files.cmp(&a.files))
            .then(a.path.cmp(&b.path))
    });
    components.truncate(12);
    components
}

// ---------------------------------------------------------------------------
// Planning
// ---------------------------------------------------------------------------

fn strategy_id(raw: &str) -> Result<&'static str, String> {
    match raw.trim() {
        "copy" => Ok("copy"),
        "move" => Ok("move"),
        "history" => Ok("history"),
        other => Err(format!("Unknown extraction strategy: {other}")),
    }
}

fn bytes_human(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn check(id: &str, label: &str, ok: bool, detail: impl Into<String>) -> ExtractCheck {
    ExtractCheck {
        id: id.to_string(),
        label: label.to_string(),
        ok,
        detail: detail.into(),
    }
}

fn expand_home(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(trimmed)
}

fn realish(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    let mut suffix = Vec::new();
    let mut current = path.to_path_buf();
    loop {
        let Some(name) = current.file_name().map(|name| name.to_os_string()) else {
            break;
        };
        suffix.push(name);
        let Some(parent) = current.parent() else {
            break;
        };
        if parent.as_os_str().is_empty() {
            break;
        }
        current = parent.to_path_buf();
        if let Ok(canonical) = current.canonicalize() {
            let mut result = canonical;
            for part in suffix.iter().rev() {
                result.push(part);
            }
            return result;
        }
    }
    path.to_path_buf()
}

fn validate_target(root: &Path, raw: &str) -> Result<PathBuf, String> {
    if raw.trim().is_empty() {
        return Err("Choose a destination folder.".to_string());
    }
    let target = expand_home(raw);
    if !target.is_absolute() {
        return Err(
            "Use an absolute destination path, for example ~/Projects/component.".to_string(),
        );
    }
    if target
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("Destination cannot contain '..'.".to_string());
    }
    let real = realish(&target);
    if real.starts_with(root) {
        return Err("Destination cannot be inside the source repository.".to_string());
    }
    if root.starts_with(&real) {
        return Err("Source cannot be inside the destination folder.".to_string());
    }
    Ok(target)
}

fn sanitize_name(raw: &str, target: &Path) -> String {
    let base = if raw.trim().is_empty() {
        target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("extract")
            .to_string()
    } else {
        raw.trim().to_string()
    };
    let mut cleaned = String::new();
    for ch in base.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            cleaned.push(ch);
        } else if ch == ' ' {
            cleaned.push('-');
        }
    }
    let cleaned = cleaned
        .trim_matches(|ch| ch == '.' || ch == '-')
        .to_string();
    let cleaned = if cleaned.is_empty() {
        "extract".to_string()
    } else {
        cleaned
    };
    cleaned.chars().take(60).collect()
}

fn common_dir(paths: &[String]) -> String {
    let mut common: Option<Vec<String>> = None;
    for path in paths {
        let mut parts: Vec<String> = path.split('/').map(str::to_string).collect();
        parts.pop();
        match &common {
            None => common = Some(parts),
            Some(current) => {
                let shared = current
                    .iter()
                    .zip(parts.iter())
                    .take_while(|(a, b)| a == b)
                    .count();
                common = Some(current[..shared].to_vec());
            }
        }
    }
    common.unwrap_or_default().join("/")
}

fn unique_backup_path(root: &Path, name: &str) -> PathBuf {
    let stamp = now();
    let base = root.join(format!("extract-{stamp}-{name}"));
    if !base.exists() {
        return base;
    }
    for index in 2..1000 {
        let candidate = root.join(format!("extract-{stamp}-{name}-{index}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    root.join(format!("extract-{stamp}-{name}-overflow"))
}

fn plan_for_graph(
    graph: &RepoGraph,
    backup_base: &Path,
    selection: &[String],
    strategy_raw: &str,
    target_raw: &str,
    name_raw: &str,
) -> Result<ExtractPlan, String> {
    let strategy = strategy_id(strategy_raw)?;
    let analysis = analyze_graph(graph, selection)?;
    let selected = expand_selection(&graph.files, selection)?;
    let target = validate_target(&graph.root, target_raw)?;
    let name = sanitize_name(name_raw, &target);
    let target_display = target.to_string_lossy().to_string();

    let target_check = if !target.exists() {
        (true, "Destination is new.".to_string())
    } else if !target.is_dir() {
        (
            false,
            "Destination exists and is not a directory.".to_string(),
        )
    } else {
        match fs::read_dir(&target) {
            Ok(mut entries) => {
                if entries.next().is_none() {
                    (true, "Destination is empty.".to_string())
                } else {
                    (
                        false,
                        "Destination already contains files. Choose an empty folder.".to_string(),
                    )
                }
            }
            Err(e) => (false, format!("Cannot read the destination: {e}")),
        }
    };

    let is_repo_root = graph.is_git
        && graph
            .toplevel
            .as_ref()
            .map(|toplevel| toplevel == &graph.root)
            .unwrap_or(false);
    let history_prefix = if strategy == "history" {
        Some(common_dir(&selected)).filter(|prefix| !prefix.is_empty())
    } else {
        None
    };
    let backup_path = if strategy == "move" {
        Some(unique_backup_path(backup_base, &name))
    } else {
        None
    };

    let mut checks = Vec::new();
    checks.push(check(
        "source",
        "Source repository",
        true,
        format!(
            "{} files · {} · {}",
            graph.files.len(),
            bytes_human(analysis.total_bytes),
            graph.branch.as_deref().unwrap_or("no git branch")
        ),
    ));
    checks.push(check(
        "selection",
        "Selection resolves to files",
        true,
        format!(
            "{} files · {} · cohesion {:.2}",
            analysis.file_count,
            bytes_human(analysis.total_bytes),
            analysis.cohesion
        ),
    ));
    checks.push(check(
        "target",
        "Destination is new or empty",
        target_check.0,
        target_check.1,
    ));
    if strategy == "history" {
        checks.push(check(
            "git",
            "History source",
            is_repo_root,
            if is_repo_root {
                format!(
                    "Git repository on {} with a complete working tree.",
                    graph.branch.as_deref().unwrap_or("HEAD")
                )
            } else if graph.is_git {
                "Open the repository root, not a folder inside a larger repository.".to_string()
            } else {
                "History extraction requires a Git repository at the source.".to_string()
            },
        ));
        checks.push(check(
            "history-prefix",
            "Component directory",
            history_prefix.is_some(),
            history_prefix
                .as_ref()
                .map(|prefix| format!("History will be split from {prefix}/."))
                .unwrap_or_else(|| {
                    "The selection has no shared directory; select one folder for history."
                        .to_string()
                }),
        ));
    }

    let mut warnings = Vec::new();
    if analysis.incoming_references > 0 {
        let sample = analysis
            .incoming
            .iter()
            .take(3)
            .map(|reference| reference.from.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        warnings.push(format!(
            "{} incoming references from outside the selection will stop resolving (for example {sample}).",
            analysis.incoming_references
        ));
    }
    if analysis.outgoing_references > 0 {
        warnings.push(format!(
            "{} references leave the selection and stay external dependencies.",
            analysis.outgoing_references
        ));
    }
    if !analysis.external_deps.is_empty() {
        let sample = analysis
            .external_deps
            .iter()
            .take(6)
            .map(|dependency| dependency.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        warnings.push(format!(
            "External packages remain needed: {sample}{}.",
            if analysis.external_deps.len() > 6 {
                ", …"
            } else {
                ""
            }
        ));
    }
    if strategy == "move" {
        warnings.push(
            "Originals are removed only after the copy validates. A backup is written first."
                .to_string(),
        );
    }
    if strategy == "history" {
        if graph.dirty {
            warnings.push(
                "The repository has uncommitted changes; only committed history is split."
                    .to_string(),
            );
        }
        if let Some(prefix) = &history_prefix {
            let prefix_count = graph
                .files
                .iter()
                .filter(|file| file.rel.starts_with(&format!("{prefix}/")))
                .count();
            if prefix_count > analysis.file_count {
                warnings.push(format!(
                    "The history split includes {} unselected files under {prefix}/ because Git history is directory-scoped.",
                    prefix_count - analysis.file_count
                ));
            }
        }
    }
    if analysis.file_count > 300 {
        warnings.push(format!(
            "Large selection ({} files). Review the plan before writing.",
            analysis.file_count
        ));
    }
    if graph.truncated {
        warnings.push("The scan hit its file limit; some files are not represented.".to_string());
    }

    let mut actions = Vec::new();
    for rel in selected.iter().take(MAX_ACTIONS) {
        actions.push(ExtractAction {
            kind: "write".to_string(),
            path: rel.clone(),
            detail: format!("{rel} → {target_display}/{rel}"),
        });
    }
    if selected.len() > MAX_ACTIONS {
        actions.push(ExtractAction {
            kind: "note".to_string(),
            path: String::new(),
            detail: format!("… and {} more files", selected.len() - MAX_ACTIONS),
        });
    }
    match strategy {
        "move" => {
            if let Some(backup) = &backup_path {
                actions.push(ExtractAction {
                    kind: "backup".to_string(),
                    path: backup.to_string_lossy().to_string(),
                    detail: "Full copy of the selection is stored before removal.".to_string(),
                });
            }
            actions.push(ExtractAction {
                kind: "remove".to_string(),
                path: selected
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                detail: format!(
                    "Remove {} original files and any empty folders left behind.",
                    selected.len()
                ),
            });
        }
        "history" => {
            actions.push(ExtractAction {
                kind: "git".to_string(),
                path: history_prefix.clone().unwrap_or_default(),
                detail: "Clone the repository, then git subtree split the component directory so its commits survive.".to_string(),
            });
        }
        _ => {}
    }

    Ok(ExtractPlan {
        source: graph.root.to_string_lossy().to_string(),
        target: target_display,
        name,
        strategy: strategy.to_string(),
        file_count: analysis.file_count,
        total_bytes: analysis.total_bytes,
        selection: selected,
        actions,
        warnings,
        checks,
        requires_backup: strategy == "move",
        backup_path: backup_path.map(|path| path.to_string_lossy().to_string()),
        history_prefix,
    })
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

fn backup_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Cannot locate the GRAFT data directory: {e}"))?;
    Ok(data_dir.join("backups"))
}

fn write_selection(root: &Path, target: &Path, files: &[String]) -> Result<u64, String> {
    let mut copied = 0u64;
    for rel in files {
        let from = root.join(rel);
        let to = target.join(rel);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
        }
        let bytes = fs::copy(&from, &to).map_err(|e| format!("Cannot copy {rel}: {e}"))?;
        copied += bytes;
    }
    Ok(copied)
}

fn backup_selection(
    root: &Path,
    backup: &Path,
    plan: &ExtractPlan,
    graph_source: &str,
) -> Result<(), String> {
    write_selection(root, backup, &plan.selection)?;
    let manifest = serde_json::json!({
        "source": graph_source,
        "target": plan.target,
        "name": plan.name,
        "strategy": plan.strategy,
        "files": plan.selection.len(),
        "createdAt": now(),
    });
    fs::write(
        backup.join("graft-backup.json"),
        serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
    )
    .map_err(|e| format!("Cannot write the backup manifest: {e}"))?;
    Ok(())
}

fn remove_selection(root: &Path, files: &[String]) -> Result<(), String> {
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    for rel in files {
        fs::remove_file(root.join(rel)).map_err(|e| format!("Cannot remove {rel}: {e}"))?;
        if let Some(parent) = parent(rel) {
            if !parent.is_empty() {
                dirs.insert(parent.to_string());
            }
        }
    }
    let mut dirs: Vec<String> = dirs.into_iter().collect();
    dirs.sort_by_key(|dir| std::cmp::Reverse(dir.matches('/').count()));
    for dir in dirs {
        let _ = fs::remove_dir(root.join(dir));
    }
    Ok(())
}

fn run_git_history(root: &Path, target: &Path, prefix: &str) -> Result<(), String> {
    if target.exists() {
        return Err("Destination already exists.".to_string());
    }
    let clone = Command::new("git")
        .args(["clone", "--no-local"])
        .arg(root)
        .arg(target)
        .output()
        .map_err(|e| format!("Cannot run git clone: {e}"))?;
    if !clone.status.success() {
        let message = String::from_utf8_lossy(&clone.stderr).trim().to_string();
        return Err(format!("git clone failed: {message}"));
    }

    let split = Command::new("git")
        .arg("-C")
        .arg(target)
        .args([
            "subtree",
            "split",
            "--prefix",
            prefix,
            "-b",
            "graft-extract",
        ])
        .output()
        .map_err(|e| {
            let _ = fs::remove_dir_all(target);
            format!("Cannot run git subtree: {e}")
        })?;
    if !split.status.success() {
        let message = String::from_utf8_lossy(&split.stderr).trim().to_string();
        let _ = fs::remove_dir_all(target);
        if message.contains("not a git command") {
            return Err(
                "git subtree is not installed. Install it or use Copy or Move instead.".to_string(),
            );
        }
        return Err(format!("git subtree split failed: {message}"));
    }

    let checkout = Command::new("git")
        .arg("-C")
        .arg(target)
        .args(["checkout", "graft-extract"])
        .output()
        .map_err(|e| {
            let _ = fs::remove_dir_all(target);
            format!("Cannot check out the split history: {e}")
        })?;
    if !checkout.status.success() {
        let message = String::from_utf8_lossy(&checkout.stderr).trim().to_string();
        let _ = fs::remove_dir_all(target);
        return Err(format!("Cannot check out the split history: {message}"));
    }

    let _ = Command::new("git")
        .arg("-C")
        .arg(target)
        .args(["remote", "remove", "origin"])
        .output();

    if let Ok(branches) = Command::new("git")
        .arg("-C")
        .arg(target)
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads"])
        .output()
    {
        for branch in String::from_utf8_lossy(&branches.stdout).lines() {
            let branch = branch.trim();
            if !branch.is_empty() && branch != "graft-extract" {
                let _ = Command::new("git")
                    .arg("-C")
                    .arg(target)
                    .args(["branch", "-D", branch])
                    .output();
            }
        }
    }
    let _ = Command::new("git")
        .arg("-C")
        .arg(target)
        .args(["branch", "-m", "graft-extract", "main"])
        .output();
    Ok(())
}

fn files_match(reference: &Path, target: &Path, selection: &[String]) -> bool {
    selection.iter().all(|rel| {
        match (
            fs::metadata(reference.join(rel)),
            fs::metadata(target.join(rel)),
        ) {
            (Ok(expected), Ok(written)) => expected.len() == written.len() && !written.is_dir(),
            _ => false,
        }
    })
}

fn validate_written(root: &Path, target: &Path, plan: &ExtractPlan) -> Vec<ExtractCheck> {
    match plan.strategy.as_str() {
        "move" => {
            let target_ok = {
                let reference = plan
                    .backup_path
                    .as_deref()
                    .map(Path::new)
                    .filter(|path| path.exists())
                    .unwrap_or(root);
                files_match(reference, target, &plan.selection)
            };
            let mut source_ok = true;
            let mut backup_ok = plan.backup_path.is_some();
            for rel in &plan.selection {
                let from = root.join(rel);
                if from.exists() {
                    source_ok = false;
                }
                if let Some(backup) = &plan.backup_path {
                    if !Path::new(backup).join(rel).exists() {
                        backup_ok = false;
                    }
                }
            }
            vec![
                check(
                    "target-files",
                    "Destination files match the source",
                    target_ok,
                    if target_ok {
                        format!("{} files verified", plan.selection.len())
                    } else {
                        "Some destination files are missing or changed.".to_string()
                    },
                ),
                check(
                    "source-removed",
                    "Originals removed",
                    source_ok,
                    if source_ok {
                        "The original selection is gone.".to_string()
                    } else {
                        "Some original files remain in the source.".to_string()
                    },
                ),
                check(
                    "backup",
                    "Backup verified",
                    backup_ok,
                    plan.backup_path
                        .as_deref()
                        .unwrap_or("No backup path recorded."),
                ),
            ]
        }
        "history" => {
            let prefix = plan.history_prefix.as_deref().unwrap_or("");
            let mut missing = 0usize;
            for rel in &plan.selection {
                let relative = rel
                    .strip_prefix(prefix)
                    .unwrap_or(rel)
                    .trim_start_matches('/');
                if !target.join(relative).exists() {
                    missing += 1;
                }
            }
            let git_ok = target.join(".git").exists();
            vec![
                check(
                    "target-files",
                    "Split working tree matches the selection",
                    missing == 0,
                    if missing == 0 {
                        format!("{} files verified", plan.selection.len())
                    } else {
                        format!("{missing} files are missing from the split.")
                    },
                ),
                check(
                    "git-history",
                    "Independent Git repository",
                    git_ok,
                    if git_ok {
                        "A new .git directory holds the split history.".to_string()
                    } else {
                        "The destination has no Git history.".to_string()
                    },
                ),
            ]
        }
        _ => {
            let mut missing = 0usize;
            for rel in &plan.selection {
                let from = root.join(rel);
                let to = target.join(rel);
                match (fs::metadata(&from), fs::metadata(&to)) {
                    (Ok(source), Ok(written)) if source.len() == written.len() => {}
                    _ => missing += 1,
                }
            }
            vec![
                check(
                    "target-files",
                    "Destination files match the source",
                    missing == 0,
                    if missing == 0 {
                        format!("{} files verified", plan.selection.len())
                    } else {
                        format!("{missing} files are missing or changed.")
                    },
                ),
                check(
                    "source-intact",
                    "Source repository untouched",
                    true,
                    "Copy never modifies the source.".to_string(),
                ),
            ]
        }
    }
}

fn record_run(db: &Db, plan: &ExtractPlan, total_bytes: u64, message: &str) -> Result<i64, String> {
    let conn =
        db.0.lock()
            .map_err(|_| "The local database is unavailable.".to_string())?;
    conn.execute(
        "INSERT INTO extraction_runs \
         (source, target, name, strategy, file_count, total_bytes, backup_path, status, message, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'completed', ?8, ?9)",
        rusqlite::params![
            plan.source,
            plan.target,
            plan.name,
            plan.strategy,
            plan.file_count as i64,
            total_bytes as i64,
            plan.backup_path.as_deref(),
            message,
            now()
        ],
    )
    .map_err(|e| format!("Extraction finished but the audit record failed: {e}"))?;
    Ok(conn.last_insert_rowid())
}

fn execute_with(
    db: &Db,
    backup_base: &Path,
    source: &str,
    selection: &[String],
    strategy: &str,
    target_raw: &str,
    name_raw: &str,
    confirmed: bool,
) -> Result<ExtractResult, String> {
    if !confirmed {
        return Err("Extraction requires explicit confirmation.".to_string());
    }
    let graph = build_graph(source)?;
    let plan = plan_for_graph(
        &graph,
        backup_base,
        selection,
        strategy,
        target_raw,
        name_raw,
    )?;
    if let Some(failed) = plan.checks.iter().find(|check| !check.ok) {
        return Err(format!("Pre-flight check failed: {}", failed.detail));
    }

    let target = PathBuf::from(&plan.target);
    let target_existed = target.exists();
    let backup_path = plan.backup_path.as_ref().map(PathBuf::from);
    let total_bytes;

    match plan.strategy.as_str() {
        "move" => {
            let Some(backup) = &backup_path else {
                return Err("Move requires a backup destination.".to_string());
            };
            total_bytes = write_selection(&graph.root, &target, &plan.selection).map_err(|e| {
                if !target_existed {
                    let _ = fs::remove_dir_all(&target);
                }
                e
            })?;
            let pre_ok = files_match(&graph.root, &target, &plan.selection);
            if !pre_ok {
                if !target_existed {
                    let _ = fs::remove_dir_all(&target);
                }
                return Err(
                    "Copy did not validate, originals kept: destination files are missing or changed."
                        .to_string(),
                );
            }
            backup_selection(&graph.root, backup, &plan, &plan.source)?;
            remove_selection(&graph.root, &plan.selection)?;
        }
        "history" => {
            let Some(prefix) = plan.history_prefix.as_deref() else {
                return Err("History extraction needs a shared component directory.".to_string());
            };
            run_git_history(&graph.root, &target, prefix)?;
            total_bytes = plan.total_bytes;
        }
        _ => {
            total_bytes = write_selection(&graph.root, &target, &plan.selection).map_err(|e| {
                if !target_existed {
                    let _ = fs::remove_dir_all(&target);
                }
                e
            })?;
        }
    }

    let checks = validate_written(&graph.root, &target, &plan);
    let ok = checks.iter().all(|check| check.ok);
    if !ok {
        if plan.strategy == "copy" && !target_existed {
            let _ = fs::remove_dir_all(&target);
        }
        return Ok(ExtractResult {
            ok: false,
            strategy: plan.strategy,
            target: plan.target,
            file_count: plan.file_count,
            total_bytes,
            backup_path: plan.backup_path,
            checks,
            message: "Validation failed. Review the checks before using the destination."
                .to_string(),
            run_id: None,
        });
    }

    let message = match plan.strategy.as_str() {
        "move" => format!(
            "Moved {} files ({}). Backup at {}.",
            plan.file_count,
            bytes_human(total_bytes),
            plan.backup_path.as_deref().unwrap_or("—")
        ),
        "history" => format!(
            "Created an independent Git repository at {} from {}/ with {} files.",
            plan.target,
            plan.history_prefix.as_deref().unwrap_or(""),
            plan.file_count
        ),
        _ => format!(
            "Copied {} files ({}) into {}.",
            plan.file_count,
            bytes_human(total_bytes),
            plan.target
        ),
    };
    let run_id = record_run(db, &plan, total_bytes, &message).ok();

    Ok(ExtractResult {
        ok: true,
        strategy: plan.strategy,
        target: plan.target,
        file_count: plan.file_count,
        total_bytes,
        backup_path: plan.backup_path,
        checks,
        message,
        run_id,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("graft-{label}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write(root: &Path, rel: &str, contents: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn sample_repo(label: &str) -> PathBuf {
        let root = temp_path(label);
        write(&root, "Cargo.toml", "[package]\nname = \"demo\"\n");
        write(
            &root,
            "src/lib.rs",
            "mod util;\nuse crate::model::Thing;\nuse serde::Serialize;\n",
        );
        write(&root, "src/model.rs", "pub struct Thing;\n");
        write(&root, "src/util.rs", "use crate::model;\n");
        write(
            &root,
            "web/app.ts",
            "import { value } from './util';\nimport x from 'svelte';\n",
        );
        write(&root, "web/util.ts", "export const value = 1;\n");
        write(&root, "py/pkg/__init__.py", "");
        write(&root, "py/pkg/main.py", "from . import util\n");
        write(&root, "py/pkg/util.py", "VALUE = 1\n");
        write(&root, "scripts/run.sh", "source ./helper.sh\n");
        write(&root, "scripts/helper.sh", "echo hello\n");
        root
    }

    #[test]
    fn scans_files_languages_and_markers() {
        let root = sample_repo("scan");
        let scan = extract_scan(root.to_string_lossy().to_string()).unwrap();
        assert!(scan.total_files >= 9);
        assert!(scan.markers.iter().any(|marker| marker == "Cargo.toml"));
        assert!(scan
            .languages
            .iter()
            .any(|language| language.language == "Rust" && language.files == 3));
        assert!(scan.total_lines > 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn analyzes_internal_and_external_edges() {
        let root = sample_repo("analyze");
        let analysis =
            extract_analyze(root.to_string_lossy().to_string(), vec!["src".to_string()]).unwrap();
        assert_eq!(analysis.file_count, 3);
        assert!(analysis.internal_references >= 1);
        assert!(analysis
            .external_deps
            .iter()
            .any(|dependency| dependency.name == "serde"));
        assert!(analysis.cohesion > 0.0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn suggests_component_directories() {
        let root = sample_repo("suggest");
        let components = extract_suggest(root.to_string_lossy().to_string()).unwrap();
        assert!(components.iter().any(|component| component.path == "src"));
        assert!(components.iter().any(|component| component.path == "web"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_traversal_and_nested_targets() {
        let root = sample_repo("safety");
        let backups = temp_path("safety-backups");
        let graph = build_graph(&root.to_string_lossy()).unwrap();
        let inside = root.join("out");
        let inside = inside.to_string_lossy();

        assert!(plan_for_graph(
            &graph,
            &backups,
            &["../outside".to_string()],
            "copy",
            &inside,
            "demo",
        )
        .is_err());
        assert!(plan_for_graph(
            &graph,
            &backups,
            &["src".to_string()],
            "copy",
            &inside,
            "demo",
        )
        .is_err());

        let outside = temp_path("safety-target");
        fs::remove_dir_all(&outside).unwrap();
        let outside = outside.to_string_lossy();
        let plan = plan_for_graph(
            &graph,
            &backups,
            &["src".to_string()],
            "copy",
            &outside,
            "demo",
        )
        .unwrap();
        assert_eq!(plan.file_count, 3);
        assert!(plan.checks.iter().all(|check| check.ok));

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(backups).unwrap();
    }

    #[test]
    fn copy_requires_confirmation_and_validates() {
        let root = sample_repo("copy");
        let backups = temp_path("backups");
        let target = temp_path("copy-target");
        fs::remove_dir_all(&target).unwrap();
        let db = Db::open(&root.join("audit.db")).unwrap();
        let args = |confirmed: bool| {
            (
                root.to_string_lossy().to_string(),
                vec!["src".to_string()],
                "copy".to_string(),
                target.to_string_lossy().to_string(),
                "src".to_string(),
                confirmed,
            )
        };

        let (source, selection, strategy, dest, name, confirmed) = args(false);
        let err = execute_with(
            &db, &backups, &source, &selection, &strategy, &dest, &name, confirmed,
        );
        assert!(err.is_err());
        assert!(!target.exists());

        let (source, selection, strategy, dest, name, confirmed) = args(true);
        let result = execute_with(
            &db, &backups, &source, &selection, &strategy, &dest, &name, confirmed,
        )
        .unwrap();
        assert!(result.ok);
        assert!(target.join("src/lib.rs").exists());
        assert!(result.run_id.is_some());
        assert_eq!(extract_runs_count(&db), 1);

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(backups).unwrap();
        fs::remove_dir_all(target).unwrap();
    }

    fn extract_runs_count(db: &Db) -> i64 {
        let conn = db.0.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM extraction_runs", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn move_backs_up_before_removing() {
        let root = sample_repo("move");
        let backups = temp_path("move-backups");
        let target = temp_path("move-target");
        fs::remove_dir_all(&target).unwrap();
        let db = Db::open(&root.join("audit.db")).unwrap();
        let result = execute_with(
            &db,
            &backups,
            &root.to_string_lossy(),
            &["web".to_string()],
            "move",
            &target.to_string_lossy(),
            "web",
            true,
        )
        .unwrap();
        assert!(result.ok);
        assert!(target.join("web/app.ts").exists());
        assert!(!root.join("web/app.ts").exists());
        assert!(backups
            .read_dir()
            .unwrap()
            .flatten()
            .any(|entry| entry.path().join("web/app.ts").exists()));
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(backups).unwrap();
        fs::remove_dir_all(target).unwrap();
    }

    #[test]
    fn history_split_preserves_commits() {
        let root = temp_path("history");
        write(&root, "src/a.txt", "hello\n");
        write(&root, "src/b.txt", "world\n");
        let init = Command::new("git")
            .arg("-C")
            .arg(&root)
            .arg("init")
            .output();
        if init.map(|output| !output.status.success()).unwrap_or(true) {
            fs::remove_dir_all(root).unwrap();
            return;
        }
        for args in [
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
            vec!["add", "."],
            vec!["commit", "-m", "initial"],
        ] {
            let _ = Command::new("git").arg("-C").arg(&root).args(args).output();
        }
        let backups = temp_path("history-backups");
        let target = temp_path("history-target");
        fs::remove_dir_all(&target).unwrap();
        let db = Db::open(&root.join("audit.db")).unwrap();
        let result = execute_with(
            &db,
            &backups,
            &root.to_string_lossy(),
            &["src".to_string()],
            "history",
            &target.to_string_lossy(),
            "component",
            true,
        )
        .unwrap();
        assert!(result.ok, "{}", result.message);
        assert!(target.join("a.txt").exists());
        assert!(target.join(".git").exists());
        let log = Command::new("git")
            .arg("-C")
            .arg(&target)
            .args(["log", "--oneline"])
            .output()
            .unwrap();
        assert!(log.status.success());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(backups).unwrap();
        fs::remove_dir_all(target).unwrap();
    }
}
