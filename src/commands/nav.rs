use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use crate::colors::Colors;
use crate::config::Config;
use crate::numbering;
use crate::project;

/// Open a directory picker over `~/notez/`, then open the chosen directory in yazi.
pub fn run_nav() {
    let config = Config::require();
    let root = config.root_path();

    if !root.exists() {
        let colors = Colors::new();
        eprintln!(
            "  {} Global notez root does not exist: {}",
            colors.peach.apply_to("✗"),
            root.display()
        );
        std::process::exit(1);
    }

    let dirs = numbering::scan_numbered_dirs(&root);
    if dirs.is_empty() {
        let colors = Colors::new();
        eprintln!(
            "  {} No numbered directories in {}. Create one with: notez -g mkdir <name>",
            colors.overlay.apply_to("─"),
            root.display()
        );
        std::process::exit(1);
    }

    let target = pick_directory(&config, &root, &dirs);

    if config.has_yazi {
        Command::new("yazi")
            .arg(&target)
            .status()
            .expect("failed to launch yazi");
    } else {
        Command::new(&config.editor)
            .arg(&target)
            .status()
            .expect("failed to launch editor");
    }
}

fn pick_directory(
    config: &Config,
    root: &Path,
    dirs: &[numbering::NumberedDir],
) -> std::path::PathBuf {
    // Project-mirrored dirs (one per project that has saved local notes) get the
    // lock icon; everything else (created via `notez -g mkdir`) gets the globe.
    let mirrored: HashSet<String> = project::ProjectMapping::load()
        .projects
        .keys()
        .cloned()
        .collect();
    let icon_for = |name: &str| {
        if mirrored.contains(name) {
            project::ICON_PRIVATE
        } else {
            project::ICON_PUBLIC
        }
    };

    if config.has_fzf {
        let input: String = dirs
            .iter()
            .map(|d| format!("{:02}  {}  {}", d.number, icon_for(&d.name), d.name))
            .collect::<Vec<_>>()
            .join("\n");

        let mut child = Command::new("fzf")
            .arg("--prompt")
            .arg("nav> ")
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
            .map(|d| format!("{:02}  {}  {}", d.number, icon_for(&d.name), d.name))
            .collect();
        let selection = dialoguer::Select::new()
            .with_prompt("Navigate to")
            .items(&items)
            .default(0)
            .interact()
            .expect("selection failed");
        root.join(&dirs[selection].full_name)
    }
}
