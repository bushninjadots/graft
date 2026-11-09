//! Project-wide relationships: remote sharing, package dependencies and the
//! extraction runs recorded by the local engine. Everything is derived from
//! data already on disk; nothing leaves the machine.

use crate::db::Db;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectNode {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub package: Option<String>,
    pub remote: Option<String>,
    pub github: Option<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEdge {
    pub source: i64,
    pub target: i64,
    pub kind: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGraph {
    pub nodes: Vec<ProjectNode>,
    pub edges: Vec<ProjectEdge>,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub fn project_graph(db: State<'_, Db>) -> Result<ProjectGraph, String> {
    let projects = db.list_projects()?;
    let mut warnings = Vec::new();
    let mut nodes = Vec::with_capacity(projects.len());

    for project in projects {
        if project.path.trim().is_empty() {
            warnings.push(format!("{} has no local path.", project.name));
            nodes.push(ProjectNode {
                id: project.id,
                name: project.name,
                path: project.path,
                package: None,
                remote: None,
                github: None,
                dependencies: Vec::new(),
            });
            continue;
        }
        let root = Path::new(&project.path);
        if !root.is_dir() {
            warnings.push(format!("{} is not available at {}.", project.name, project.path));
        }
        let (package, dependencies) = read_manifest(root);
        let status = crate::git::status(&project.path);
        nodes.push(ProjectNode {
            id: project.id,
            name: project.name,
            path: project.path,
            package,
            remote: status.remote,
            github: status.github,
            dependencies,
        });
    }

    let mut edges = Vec::new();

    // Extractions recorded by the audit log become directed relationships.
    let runs = extraction_runs(&db)?;
    for (source, target, name) in runs {
        if let (Some(from), Some(to)) = (find_node(&nodes, &source), find_node(&nodes, &target)) {
            if from != to {
                edges.push(ProjectEdge {
                    source: from,
                    target: to,
                    kind: "extracted".to_string(),
                    label: name,
                });
            }
        }
    }

    // Repositories sharing a remote are the same project seen from two paths.
    for (index, node) in nodes.iter().enumerate() {
        let Some(remote) = node.remote.as_deref().map(remote_key) else {
            continue;
        };
        if remote.is_empty() {
            continue;
        }
        for other in nodes.iter().skip(index + 1) {
            if let Some(other_remote) = other.remote.as_deref().map(remote_key) {
                if remote == other_remote {
                    edges.push(ProjectEdge {
                        source: node.id,
                        target: other.id,
                        kind: "shared-remote".to_string(),
                        label: node
                            .github
                            .clone()
                            .or_else(|| other.github.clone())
                            .unwrap_or(remote.clone()),
                    });
                }
            }
        }
    }

    // A package named in one project's manifest that another project declares
    // as its package name is a local dependency edge.
    for node in &nodes {
        let Some(package) = node.package.as_deref().map(normalize) else {
            continue;
        };
        if package.is_empty() {
            continue;
        }
        for target in &nodes {
            if target.id == node.id || target.package.as_deref().map(normalize).as_deref() != Some(&package) {
                continue;
            }
            edges.push(ProjectEdge {
                source: node.id,
                target: target.id,
                kind: "depends-on".to_string(),
                label: node.package.clone().unwrap_or(package.clone()),
            });
        }
    }

    let mut seen = BTreeSet::new();
    edges.retain(|edge| seen.insert((edge.source, edge.target, edge.kind.clone())));
    edges.sort_by_key(|edge| (edge.source, edge.target));

    nodes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    warnings.sort();
    warnings.dedup();

    Ok(ProjectGraph {
        nodes,
        edges,
        warnings,
    })
}

fn extraction_runs(db: &Db) -> Result<Vec<(String, String, String)>, String> {
    let conn = db
        .0
        .lock()
        .map_err(|_| "The local database is unavailable.".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT source, target, name FROM extraction_runs \
             WHERE status = 'completed' ORDER BY created_at DESC",
        )
        .map_err(|e| format!("cannot read extraction runs: {e}"))?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|e| format!("cannot read extraction runs: {e}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot decode extraction runs: {e}"))
}

fn find_node(nodes: &[ProjectNode], path: &str) -> Option<i64> {
    let wanted = path_key(path);
    nodes
        .iter()
        .find(|node| path_key(&node.path) == wanted)
        .map(|node| node.id)
}

fn path_key(path: &str) -> String {
    let trimmed = path.trim().trim_end_matches('/');
    fs::canonicalize(trimmed)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| trimmed.to_string())
}

fn remote_key(remote: &str) -> String {
    let mut value = remote.trim().to_lowercase();
    for prefix in ["https://", "http://", "ssh://", "git://"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest.to_string();
            break;
        }
    }
    if let Some(rest) = value.strip_prefix("git@") {
        value = rest.replacen(':', "/", 1);
    }
    if let Some((_, rest)) = value.split_once('@') {
        if !rest.is_empty() && !rest.starts_with('/') {
            value = rest.to_string();
        }
    }
    value = value
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_string();
    value
}

fn normalize(value: &str) -> String {
    value.trim().to_lowercase().replace('_', "-")
}

fn read_manifest(root: &Path) -> (Option<String>, Vec<String>) {
    let mut package = None;
    let mut dependencies = BTreeSet::new();

    if let Some(raw) = read_json(root, "package.json") {
        if let Some(value) = raw.get("name").and_then(|value| value.as_str()) {
            package = Some(value.to_string());
        }
        for section in [
            "dependencies",
            "devDependencies",
            "peerDependencies",
            "optionalDependencies",
        ] {
            if let Some(deps) = raw.get(section).and_then(|value| value.as_object()) {
                for name in deps.keys() {
                    insert_dependency(&mut dependencies, name);
                }
            }
        }
    }

    if let Some(text) = read_text(root, "Cargo.toml") {
        let mut section = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                section = trimmed.trim_matches(['[', ']']).to_string();
                continue;
            }
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((key, _)) = trimmed.split_once('=') else {
                continue;
            };
            let key = key.trim().trim_matches(['"', '\'']);
            if section == "package" && key == "name" && package.is_none() {
                package = value_of(trimmed);
            } else if matches!(
                section.as_str(),
                "dependencies" | "dev-dependencies" | "build-dependencies" | "workspace.dependencies"
            ) && !key.starts_with('[')
            {
                insert_dependency(&mut dependencies, key);
            }
        }
    }

    if let Some(text) = read_text(root, "pyproject.toml") {
        let mut section = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                section = trimmed.trim_matches(['[', ']']).to_string();
                continue;
            }
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if section == "project" && trimmed.starts_with("name") && package.is_none() {
                package = value_of(trimmed);
            }
            if section == "tool.poetry" && trimmed.starts_with("name") && package.is_none() {
                package = value_of(trimmed);
            }
            if section == "tool.poetry.dependencies"
                && !trimmed.starts_with("python")
            {
                if let Some((key, _)) = trimmed.split_once('=') {
                    insert_dependency(&mut dependencies, key.trim().trim_matches(['"', '\'']));
                }
            }
            if section == "project" && trimmed.starts_with("dependencies") {
                for item in quoted_items(trimmed) {
                    insert_dependency(&mut dependencies, &item);
                }
            }
        }
    }

    if let Some(text) = read_text(root, "go.mod") {
        let mut in_require = false;
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(module) = trimmed.strip_prefix("module ") {
                if package.is_none() {
                    package = Some(module.trim().to_string());
                }
            } else if trimmed.starts_with("require (") {
                in_require = true;
            } else if in_require && trimmed == ")" {
                in_require = false;
            } else if in_require {
                if let Some(name) = trimmed.split_whitespace().next() {
                    insert_dependency(&mut dependencies, name);
                }
            } else if let Some(rest) = trimmed.strip_prefix("require ") {
                if let Some(name) = rest.split_whitespace().next() {
                    insert_dependency(&mut dependencies, name);
                }
            }
        }
    }

    if let Some(raw) = read_json(root, "composer.json") {
        if let Some(value) = raw.get("name").and_then(|value| value.as_str()) {
            package = Some(value.to_string());
        }
        for section in ["require", "require-dev"] {
            if let Some(deps) = raw.get(section).and_then(|value| value.as_object()) {
                for name in deps.keys() {
                    insert_dependency(&mut dependencies, name);
                }
            }
        }
    }

    (package, dependencies.into_iter().collect())
}

fn read_text(root: &Path, name: &str) -> Option<String> {
    fs::read_to_string(root.join(name)).ok()
}

fn read_json(root: &Path, name: &str) -> Option<serde_json::Value> {
    let text = read_text(root, name)?;
    serde_json::from_str(&text).ok()
}

fn value_of(line: &str) -> Option<String> {
    let (_, value) = line.split_once('=')?;
    let value = value.trim().trim_matches(['"', '\'']).trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn quoted_items(line: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in line.chars() {
        match quote {
            Some(active) if ch == active => {
                items.push(current.trim().to_string());
                current.clear();
                quote = None;
            }
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None => {}
        }
    }
    items
}

fn insert_dependency(set: &mut BTreeSet<String>, raw: &str) {
    let cleaned = raw
        .trim()
        .trim_matches(['"', '\'', ',', ' '])
        .split(['[', ' ', '<', '>', '=', '^', '~', ';'])
        .next()
        .unwrap_or_default();
    let cleaned = cleaned.trim().trim_matches(['"', '\'', ',', ' ']);
    if cleaned.is_empty() {
        return;
    }
    set.insert(normalize(cleaned));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_remote_shapes() {
        for remote in [
            "https://github.com/bender/GRAFT.git",
            "git@github.com:bender/GRAFT.git",
            "ssh://git@github.com/bender/GRAFT",
        ] {
            assert_eq!(remote_key(remote), "github.com/bender/graft");
        }
    }

    #[test]
    fn reads_package_manifests() {
        let dir = std::env::temp_dir().join(format!(
            "graft-relations-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).expect("create");
        fs::write(
            dir.join("package.json"),
            r#"{"name":"@scope/app","dependencies":{"@scope/lib":"^1"},"devDependencies":{"vite":"^7"}}"#,
        )
        .expect("write");
        let (package, deps) = read_manifest(&dir);
        assert_eq!(package.as_deref(), Some("@scope/app"));
        assert!(deps.contains(&"@scope/lib".to_string()), "scoped dependency kept");
        assert!(deps.contains(&"vite".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_go_module() {
        let dir = std::env::temp_dir().join(format!(
            "graft-relations-go-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).expect("create");
        fs::write(
            dir.join("go.mod"),
            "module example.com/app\n\nrequire (\n\texample.com/lib v1.2.0\n)\n",
        )
        .expect("write");
        let (package, deps) = read_manifest(&dir);
        assert_eq!(package.as_deref(), Some("example.com/app"));
        assert_eq!(deps, vec!["example.com/lib".to_string()]);
        let _ = fs::remove_dir_all(&dir);
    }
}
