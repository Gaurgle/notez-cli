use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Local;

use crate::colors::Colors;
use crate::config::Config;
use crate::numbering;
use crate::project;

pub fn run_add(global: bool, title: Option<String>, target: Option<String>, body: Option<String>) {
    let config = Config::require();
    let date = Local::now().format("%Y-%m-%d").to_string();
    let raw_title = title.unwrap_or_else(|| "untitled".into());
    let clean_title = sanitize_title(&raw_title);

    let root = project::resolve_notez_dir(&config, global);

    let target_dir = match target {
        None => {
            if global {
                let dir = config.quick_notes_path();
                fs::create_dir_all(&dir).expect("failed to create quick notes directory");
                dir
            } else {
                fs::create_dir_all(&root).expect("failed to create local notez directory");
                root.clone()
            }
        }
        Some(explicit) if !explicit.is_empty() => resolve_target_dir(&root, &explicit),
        Some(_) => pick_directory(&config, &root),
    };

    let file_path = create_note_file(&target_dir, &date, &clean_title, body.as_deref());
    let colors = Colors::new();
    println!(
        "  {} created {}",
        colors.green.apply_to("✓"),
        colors.sapphire.apply_to(file_path.file_name().unwrap().to_str().unwrap())
    );

    if !global {
        let home_dir = project::ensure_home_project_dir(&config);
        project::mirror_file_to_home(&file_path, &home_dir);
    }

    if body.is_none() {
        Command::new(&config.editor)
            .arg("+4")
            .arg("-c")
            .arg("startinsert")
            .arg(&file_path)
            .status()
            .expect("failed to launch editor");
    }
}

fn create_note_file(dir: &Path, date: &str, title: &str, body: Option<&str>) -> PathBuf {
    fs::create_dir_all(dir).expect("failed to create target directory");
    let file_path = dir.join(format!("{}-{}.md", date, title));
    let content = match body {
        Some(text) => format!("# {}\n\nDate: {}\n\n{}\n", title, date, text),
        None => format!("# {}\n\nDate: {}\n\n", title, date),
    };
    fs::write(&file_path, content).expect("failed to write note file");
    file_path
}

fn sanitize_title(title: &str) -> String {
    title
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

fn resolve_target_dir(root: &Path, target: &str) -> PathBuf {
    let full_path = root.join(target);
    if full_path.exists() {
        return full_path;
    }

    if let Some(matched) =
        numbering::fuzzy_match_dir(root, target.split('/').next().unwrap_or(target))
    {
        let base = root.join(&matched.full_name);
        let remainder: String = target.split('/').skip(1).collect::<Vec<_>>().join("/");
        if remainder.is_empty() {
            return base;
        }
        let nested = base.join(&remainder);
        fs::create_dir_all(&nested).expect("failed to create nested directory");
        return nested;
    }

    eprintln!("Directory not found: {}", target);
    std::process::exit(1);
}

fn pick_directory(config: &Config, root: &Path) -> PathBuf {
    let dirs = numbering::scan_numbered_dirs(root);

    if dirs.is_empty() {
        eprintln!("No notez directories found. Create one with: notez mkdir <name>");
        std::process::exit(1);
    }

    if config.has_fzf {
        let input: String = dirs
            .iter()
            .map(|d| format!("{:02}  {}", d.number, d.name))
            .collect::<Vec<_>>()
            .join("\n");

        let mut child = Command::new("fzf")
            .arg("--prompt")
            .arg("directory> ")
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

        let selected = String::from_utf8(output.stdout).unwrap();
        let selected = selected.trim();
        let number: u8 = selected[..2].parse().expect("invalid selection");
        let dir = dirs
            .iter()
            .find(|d| d.number == number)
            .expect("dir not found");
        root.join(&dir.full_name)
    } else {
        let items: Vec<String> = dirs
            .iter()
            .map(|d| format!("{:02}  {}", d.number, d.name))
            .collect();
        let selection = dialoguer::Select::new()
            .with_prompt("Select directory")
            .items(&items)
            .default(0)
            .interact()
            .expect("selection failed");
        root.join(&dirs[selection].full_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn creates_note_with_title() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_note_file(dir.path(), "2026-04-02", "my-idea", None);

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# my-idea\n"));
        assert!(content.contains("Date: 2026-04-02"));
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "2026-04-02-my-idea.md"
        );
    }

    #[test]
    fn creates_note_with_default_title() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_note_file(dir.path(), "2026-04-02", "untitled", None);
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            "2026-04-02-untitled.md"
        );
    }

    #[test]
    fn creates_note_with_body() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_note_file(
            dir.path(),
            "2026-04-02",
            "my-idea",
            Some("This is the note content"),
        );

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("This is the note content"));
    }

    #[test]
    fn sanitizes_title() {
        assert_eq!(sanitize_title("My Cool Note"), "my-cool-note");
        assert_eq!(sanitize_title("hello world!"), "hello-world");
    }
}
