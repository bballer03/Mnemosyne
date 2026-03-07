use crate::errors::CoreResult;
use serde::{Deserialize, Serialize};
use std::{
    cmp, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapToCodeRequest {
    pub leak_id: String,
    pub class_name: Option<String>,
    pub project_root: PathBuf,
    pub include_git_info: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMapResult {
    pub leak_id: String,
    pub locations: Vec<CodeLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeLocation {
    pub file: PathBuf,
    pub line: u32,
    pub symbol: String,
    pub code_snippet: String,
    pub git: Option<GitMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitMetadata {
    pub author: String,
    pub commit: String,
    pub date: String,
    pub message: String,
}

/// Attempt to map a leak identifier to likely source files using simple
/// heuristics. This is intentionally lightweight so it can run quickly during
/// interactive MCP sessions.
pub fn map_to_code(request: &MapToCodeRequest) -> CoreResult<SourceMapResult> {
    let root = if request.project_root.as_os_str().is_empty() {
        std::env::current_dir()?
    } else {
        request.project_root.clone()
    };

    let class_hint = request
        .class_name
        .as_deref()
        .or_else(|| class_from_leak_id(&request.leak_id))
        .map(|s| s.to_string());

    let candidates = candidate_files(&root, class_hint.as_deref());
    let mut locations = Vec::new();
    for path in candidates {
        if let Some(location) = build_location(&root, &path, request)? {
            locations.push(location);
        }
        if locations.len() >= 3 {
            break;
        }
    }

    if locations.is_empty() {
        locations.push(fallback_location(
            &root,
            class_hint.as_deref(),
            &request.leak_id,
        ));
    }

    Ok(SourceMapResult {
        leak_id: request.leak_id.clone(),
        locations,
    })
}

fn candidate_files(project_root: &Path, class_hint: Option<&str>) -> Vec<PathBuf> {
    let class_path = class_hint
        .map(|class| class.replace('.', "/"))
        .unwrap_or_else(|| "com/example/LeakHotspot".into());

    let mut candidates = Vec::new();
    let bases = [
        project_root
            .join("src")
            .join("main")
            .join("java")
            .join(&class_path),
        project_root
            .join("src")
            .join("main")
            .join("kotlin")
            .join(&class_path),
        project_root.join("src").join(&class_path),
        project_root.join(&class_path),
    ];

    for base in bases {
        candidates.push(base.with_extension("java"));
        candidates.push(base.with_extension("kt"));
    }

    candidates
}

fn build_location(
    project_root: &Path,
    candidate: &Path,
    request: &MapToCodeRequest,
) -> CoreResult<Option<CodeLocation>> {
    if !candidate.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(candidate)?;
    let (line, symbol) = find_symbol(
        &contents,
        request
            .class_name
            .as_deref()
            .or_else(|| class_from_leak_id(&request.leak_id)),
    );
    let snippet = extract_snippet(&contents, line);
    let git = if request.include_git_info {
        git_metadata(project_root, candidate)
    } else {
        None
    };

    Ok(Some(CodeLocation {
        file: candidate.to_path_buf(),
        line,
        symbol,
        code_snippet: snippet,
        git,
    }))
}

fn extract_snippet(contents: &str, line: u32) -> String {
    if contents.is_empty() {
        return String::new();
    }

    let lines: Vec<&str> = contents.lines().collect();
    let idx = cmp::min(
        lines.len().saturating_sub(1),
        line.saturating_sub(1) as usize,
    );
    let start = idx.saturating_sub(1);
    let end = cmp::min(lines.len(), idx + 2);
    lines[start..end].join("\n")
}

fn find_symbol(contents: &str, hint: Option<&str>) -> (u32, String) {
    if let Some(hint) = hint {
        if let Some(line_idx) = contents
            .lines()
            .position(|line| line.contains(hint.split('.').next_back().unwrap_or(hint)))
        {
            let symbol = contents
                .lines()
                .nth(line_idx)
                .map(|l| l.trim().to_string())
                .unwrap_or_else(|| hint.to_string());
            return (line_idx as u32 + 1, symbol);
        }
    }

    for (idx, line) in contents.lines().enumerate() {
        if line.contains("class ") || line.contains("fn ") || line.contains("void ") {
            return (idx as u32 + 1, line.trim().to_string());
        }
    }

    (1, contents.lines().next().unwrap_or("").trim().to_string())
}

fn fallback_location(project_root: &Path, class_hint: Option<&str>, leak_id: &str) -> CodeLocation {
    let pseudo_file = project_root
        .join(".mnemosyne")
        .join("unmapped")
        .join(format!("{}.txt", leak_id.replace(':', "_")));
    CodeLocation {
        file: pseudo_file,
        line: 1,
        symbol: class_hint.unwrap_or(leak_id).to_string(),
        code_snippet: "No matching source file found. Provide --class to improve results.".into(),
        git: None,
    }
}

fn git_metadata(project_root: &Path, file: &Path) -> Option<GitMetadata> {
    let relative = file.strip_prefix(project_root).unwrap_or(file);
    let root = project_root.to_path_buf();
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("log")
        .arg("-1")
        .arg("--date=iso")
        .arg("--pretty=format:%an|%H|%ad|%s")
        .arg("--")
        .arg(relative)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().splitn(4, '|').collect();
    if parts.len() < 4 {
        return None;
    }

    Some(GitMetadata {
        author: parts[0].to_string(),
        commit: parts[1].to_string(),
        date: parts[2].to_string(),
        message: parts[3].to_string(),
    })
}

fn class_from_leak_id(leak_id: &str) -> Option<&str> {
    leak_id.split("::").next().filter(|part| !part.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn maps_existing_file() {
        let dir = tempdir().unwrap();
        let src_dir = dir
            .path()
            .join("src")
            .join("main")
            .join("java")
            .join("com")
            .join("example");
        std::fs::create_dir_all(&src_dir).unwrap();
        let file_path = src_dir.join("MemoryKeeper.java");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(
            file,
            "package com.example;\n\npublic class MemoryKeeper {{\n  void retain() {{}}\n}}"
        )
        .unwrap();

        let request = MapToCodeRequest {
            leak_id: "com.example.MemoryKeeper::deadbeef".into(),
            class_name: Some("com.example.MemoryKeeper".into()),
            project_root: dir.path().to_path_buf(),
            include_git_info: false,
        };

        let response = map_to_code(&request).unwrap();
        assert_eq!(response.leak_id, request.leak_id);
        assert!(!response.locations.is_empty());
        let location = &response.locations[0];
        assert_eq!(location.file, file_path);
        assert!(location.code_snippet.contains("MemoryKeeper"));
    }
}
