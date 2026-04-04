use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::colors::Colors;
use crate::config::Config;
use crate::project;

pub fn run_edit(global: bool, public: bool, term: Option<String>) {
    let config = Config::require();
    let root = project::resolve_notez_dir(&config, global, public);

    if !root.exists() {
        let colors = Colors::new();
        eprintln!(
            "  {} No notez directory here.",
            colors.peach.apply_to("✗")
        );
        std::process::exit(1);
    }

    let files = scan_notes(&root);
    if files.is_empty() {
        let colors = Colors::new();
        eprintln!("  {} No notes found.", colors.overlay.apply_to("─"));
        std::process::exit(1);
    }

    let file = match term {
        Some(query) => {
            let matches = fuzzy_filter(&files, &query);
            match matches.len() {
                0 => {
                    let colors = Colors::new();
                    eprintln!(
                        "  {} No notes matching \"{}\"",
                        colors.peach.apply_to("✗"),
                        query
                    );
                    std::process::exit(1);
                }
                1 => matches[0].clone(),
                _ => pick_file(&config, &matches, &root),
            }
        }
        None => pick_file(&config, &files, &root),
    };

    Command::new(&config.editor)
        .arg(&file)
        .status()
        .expect("failed to launch editor");
}

fn scan_notes(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    scan_notes_recursive(root, &mut files);
    files.sort();
    files
}

fn scan_notes_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_notes_recursive(&path, files);
        } else if path.extension().map_or(false, |e| e == "md") {
            files.push(path);
        }
    }
}

fn fuzzy_filter(files: &[PathBuf], query: &str) -> Vec<PathBuf> {
    let query_lower = query.to_lowercase();
    files
        .iter()
        .filter(|f| {
            f.file_name()
                .unwrap()
                .to_string_lossy()
                .to_lowercase()
                .contains(&query_lower)
        })
        .cloned()
        .collect()
}

fn pick_file(config: &Config, files: &[PathBuf], root: &Path) -> PathBuf {
    if config.has_fzf {
        let input: String = files
            .iter()
            .map(|f| f.strip_prefix(root).unwrap_or(f).to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        let mut child = Command::new("fzf")
            .arg("--prompt")
            .arg("note> ")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("failed to launch fzf");

        {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(input.as_bytes()).unwrap();
        }

        let output = child.wait_with_output().expect("fzf failed");
        if !output.status.success() {
            std::process::exit(1);
        }

        let selected = String::from_utf8(output.stdout).unwrap().trim().to_string();
        root.join(selected)
    } else {
        let items: Vec<String> = files
            .iter()
            .map(|f| f.strip_prefix(root).unwrap_or(f).to_string_lossy().to_string())
            .collect();
        let selection = dialoguer::Select::new()
            .with_prompt("Select note")
            .items(&items)
            .default(0)
            .interact()
            .expect("selection failed");
        files[selection].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scan_finds_md_files_recursively() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("00_notes");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("note1.md"), "x").unwrap();
        fs::write(sub.join("note2.md"), "x").unwrap();
        fs::write(dir.path().join("root.md"), "x").unwrap();
        fs::write(dir.path().join("ignore.txt"), "x").unwrap();

        let files = scan_notes(dir.path());
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn fuzzy_match_filters_by_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("2026-04-02-api-design.md"), "x").unwrap();
        fs::write(dir.path().join("2026-04-02-bug-fix.md"), "x").unwrap();
        fs::write(dir.path().join("2026-04-01-meeting.md"), "x").unwrap();

        let files = scan_notes(dir.path());
        let matches = fuzzy_filter(&files, "api");
        assert_eq!(matches.len(), 1);
        assert!(matches[0].file_name().unwrap().to_str().unwrap().contains("api"));
    }

    #[test]
    fn fuzzy_match_partial() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("2026-04-02-api-design.md"), "x").unwrap();
        fs::write(dir.path().join("2026-04-02-api-notes.md"), "x").unwrap();
        fs::write(dir.path().join("2026-04-01-meeting.md"), "x").unwrap();

        let files = scan_notes(dir.path());
        let matches = fuzzy_filter(&files, "api");
        assert_eq!(matches.len(), 2);
    }
}
