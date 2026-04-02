// Numbered directory utilities

use std::fs;
use std::path::Path;

/// A parsed numbered directory entry (e.g., "03_project-ideas").
#[derive(Debug, Clone)]
pub struct NumberedDir {
    pub number: u8,
    pub name: String,
    pub full_name: String,
}

/// Lowercases, collapses whitespace to hyphens, and strips non-alphanumeric/hyphen chars.
pub fn sanitize_name(name: &str) -> String {
    name.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

/// Scans `root` for directories matching the `NN_name` pattern, sorted by number.
pub fn scan_numbered_dirs(root: &Path) -> Vec<NumberedDir> {
    let mut dirs = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return dirs;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        if let Some(parsed) = parse_numbered_name(&file_name) {
            dirs.push(parsed);
        }
    }
    dirs.sort_by_key(|d| d.number);
    dirs
}

/// Parses a directory name like "03_project-ideas" into a `NumberedDir`.
fn parse_numbered_name(name: &str) -> Option<NumberedDir> {
    if name.len() < 3 {
        return None;
    }
    let prefix = &name[..2];
    let number: u8 = prefix.parse().ok()?;
    if !name[2..].starts_with('_') {
        return None;
    }
    let dir_name = name[3..].to_string();
    Some(NumberedDir {
        number,
        name: dir_name,
        full_name: name.to_string(),
    })
}

/// Returns the lowest unused number in 0..100, filling gaps first.
pub fn next_available_number(root: &Path) -> Option<u8> {
    let existing = scan_numbered_dirs(root);
    let taken: std::collections::HashSet<u8> = existing.iter().map(|d| d.number).collect();
    (0..100).find(|n| !taken.contains(n))
}

/// Finds a numbered dir whose name matches `query` exactly, or starts with it.
pub fn fuzzy_match_dir(root: &Path, query: &str) -> Option<NumberedDir> {
    let dirs = scan_numbered_dirs(root);
    let query_lower = query.to_lowercase();
    // Exact match first
    if let Some(d) = dirs.iter().find(|d| d.name == query_lower) {
        return Some(d.clone());
    }
    // Prefix match
    dirs.into_iter().find(|d| d.name.starts_with(&query_lower))
}

/// Counts immediate files (not directories) in `path`.
#[allow(dead_code)]
pub fn count_files(path: &Path) -> usize {
    fs::read_dir(path)
        .map(|entries| entries.flatten().filter(|e| e.path().is_file()).count())
        .unwrap_or(0)
}

/// Recursively counts all files under `path`.
pub fn count_files_recursive(path: &Path) -> usize {
    let mut count = 0;
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            count += 1;
        } else if p.is_dir() {
            count += count_files_recursive(&p);
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn sanitize_name_lowercases_and_replaces_spaces() {
        assert_eq!(sanitize_name("Project Ideas"), "project-ideas");
    }

    #[test]
    fn sanitize_name_handles_multiple_spaces_and_special_chars() {
        assert_eq!(sanitize_name("My  Cool   Project!"), "my-cool-project");
    }

    #[test]
    fn scan_empty_dir_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = scan_numbered_dirs(dir.path());
        assert!(dirs.is_empty());
    }

    #[test]
    fn scan_finds_numbered_dirs() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_quick-notes")).unwrap();
        fs::create_dir(dir.path().join("01_daily-logs")).unwrap();
        fs::create_dir(dir.path().join("random-stuff")).unwrap();

        let dirs = scan_numbered_dirs(dir.path());
        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[0].number, 0);
        assert_eq!(dirs[0].name, "quick-notes");
        assert_eq!(dirs[1].number, 1);
        assert_eq!(dirs[1].name, "daily-logs");
    }

    #[test]
    fn next_number_with_no_dirs() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(next_available_number(dir.path()), Some(0));
    }

    #[test]
    fn next_number_skips_existing() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_first")).unwrap();
        fs::create_dir(dir.path().join("01_second")).unwrap();
        assert_eq!(next_available_number(dir.path()), Some(2));
    }

    #[test]
    fn next_number_fills_gaps() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_first")).unwrap();
        fs::create_dir(dir.path().join("02_third")).unwrap();
        assert_eq!(next_available_number(dir.path()), Some(1));
    }

    #[test]
    fn fuzzy_match_finds_dir_by_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_quick-notes")).unwrap();
        fs::create_dir(dir.path().join("02_project-ideas")).unwrap();

        let result = fuzzy_match_dir(dir.path(), "project-ideas");
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "project-ideas");
    }

    #[test]
    fn fuzzy_match_partial() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("02_project-ideas")).unwrap();

        let result = fuzzy_match_dir(dir.path(), "project");
        assert!(result.is_some());
    }
}
