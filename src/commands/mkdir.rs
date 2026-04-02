use std::fs;
use std::path::Path;

use crate::colors::Colors;
use crate::config::Config;
use crate::numbering;
use crate::project;

pub fn run_mkdir(global: bool, name_parts: Vec<String>) {
    let config = Config::require();
    let root = project::resolve_notez_dir(&config, global);
    let raw_name = name_parts.join(" ");

    if raw_name.is_empty() {
        eprintln!("Usage: notez mkdir <name>");
        std::process::exit(1);
    }

    let sanitized = numbering::sanitize_name(&raw_name);
    let colors = Colors::new();

    // Ensure root exists
    fs::create_dir_all(&root).expect("failed to create notez directory");

    let existing = numbering::scan_numbered_dirs(&root);
    if !existing.is_empty() {
        println!(
            "\n  {} Existing directories:",
            colors.overlay.apply_to("─")
        );
        for d in &existing {
            println!(
                "    {:02}  {}",
                d.number,
                colors.sapphire.apply_to(&d.name)
            );
        }
        println!();
    }

    match create_numbered_dir(&root, &sanitized) {
        Ok(full_name) => {
            println!(
                "  {} created {}",
                colors.green.apply_to("✓"),
                colors.sapphire.apply_to(&full_name)
            );

            if !global {
                let created_dir = root.join(&full_name);
                let home_dir = project::ensure_home_project_dir(&config);
                project::mirror_dir_to_home(&created_dir, &home_dir);
            }
        }
        Err(e) => {
            eprintln!("  {} {}", colors.peach.apply_to("✗"), e);
            std::process::exit(1);
        }
    }
}

/// Creates a numbered directory under `root` with the given `name`.
///
/// The name is sanitized (lowercased, spaces to hyphens) and prefixed with
/// the next available two-digit number (00-99).
fn create_numbered_dir(root: &Path, name: &str) -> Result<String, String> {
    let sanitized = numbering::sanitize_name(name);
    let number = numbering::next_available_number(root)
        .ok_or_else(|| "all 100 directory slots (00-99) are taken".to_string())?;

    let dir_name = format!("{:02}_{}", number, sanitized);
    let full_path = root.join(&dir_name);

    fs::create_dir_all(&full_path)
        .map_err(|e| format!("failed to create directory: {}", e))?;

    Ok(dir_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn creates_first_numbered_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = create_numbered_dir(dir.path(), "recipes");
        assert!(result.is_ok());
        assert!(dir.path().join("00_recipes").exists());
    }

    #[test]
    fn creates_sequential_numbered_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_quick-notes")).unwrap();
        fs::create_dir(dir.path().join("01_daily-logs")).unwrap();

        let result = create_numbered_dir(dir.path(), "recipes");
        assert!(result.is_ok());
        assert!(dir.path().join("02_recipes").exists());
    }

    #[test]
    fn sanitizes_dir_name() {
        let dir = tempfile::tempdir().unwrap();
        let result = create_numbered_dir(dir.path(), "Project Ideas");
        assert!(result.is_ok());
        assert!(dir.path().join("00_project-ideas").exists());
    }
}
