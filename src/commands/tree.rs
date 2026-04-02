use std::fs;
use std::path::Path;

use crate::colors::Colors;
use crate::config::Config;
use crate::numbering;

struct TreeData {
    numbered: Vec<TreeEntry>,
    other: Vec<TreeEntry>,
}

struct TreeEntry {
    display_name: String,
    file_count: usize,
    subdirs: Vec<SubdirEntry>,
}

struct SubdirEntry {
    name: String,
    file_count: usize,
}

/// Display the notez directory tree with Catppuccin Mocha colors.
///
/// Numbered directories (00-99) appear first with file counts and
/// recursive subdirectory listings. Non-numbered directories appear
/// below a divider.
pub fn run_tree() {
    let config = Config::require();
    let root = config.root_path();
    let colors = Colors::new();

    let display_root = config.notez_root.replacen(
        &dirs::home_dir().unwrap().to_string_lossy().to_string(),
        "~",
        1,
    );

    let tree = collect_tree(&root);

    println!();
    println!(
        "  {} {} {}",
        colors.lavender.apply_to("notez"),
        colors.overlay.apply_to("—"),
        colors.overlay.apply_to(&display_root)
    );
    let div_width = 40;
    println!("  {}", colors.divider(div_width));

    for entry in &tree.numbered {
        println!(
            "  {}  {:<24} {} {}",
            colors.overlay.apply_to(&entry.display_name[..2]),
            colors.sapphire.apply_to(&entry.display_name[3..]),
            colors.overlay.apply_to(format!("{:>3}", entry.file_count)),
            colors
                .overlay
                .apply_to(if entry.file_count == 1 { "note" } else { "notes" })
        );
        let sub_count = entry.subdirs.len();
        for (i, sub) in entry.subdirs.iter().enumerate() {
            let connector = if i == sub_count - 1 { "└──" } else { "├──" };
            println!(
                "      {} {:<20} {} {}",
                colors.surface.apply_to(connector),
                colors.overlay.apply_to(format!("{}/", sub.name)),
                colors.overlay.apply_to(format!("{:>3}", sub.file_count)),
                colors
                    .overlay
                    .apply_to(if sub.file_count == 1 { "note" } else { "notes" })
            );
        }
    }

    if !tree.other.is_empty() {
        println!("  {}", colors.divider(div_width));
        for entry in &tree.other {
            println!(
                "  {:<26} {} {}",
                colors.overlay.apply_to(format!("{}/", entry.display_name)),
                colors.overlay.apply_to(format!("{:>3}", entry.file_count)),
                colors
                    .overlay
                    .apply_to(if entry.file_count == 1 { "file" } else { "files" })
            );
        }
    }

    println!();
}

/// Walk the root directory and partition entries into numbered vs other dirs.
fn collect_tree(root: &Path) -> TreeData {
    let numbered_dirs = numbering::scan_numbered_dirs(root);
    let mut numbered = Vec::new();
    let mut other = Vec::new();

    let taken_names: Vec<String> = numbered_dirs.iter().map(|d| d.full_name.clone()).collect();

    for nd in &numbered_dirs {
        let path = root.join(&nd.full_name);
        numbered.push(TreeEntry {
            display_name: nd.full_name.clone(),
            file_count: numbering::count_files_recursive(&path),
            subdirs: collect_subdirs(&path),
        });
    }

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if taken_names.contains(&name) {
                continue;
            }
            other.push(TreeEntry {
                display_name: name,
                file_count: numbering::count_files_recursive(&path),
                subdirs: vec![],
            });
        }
    }

    other.sort_by(|a, b| a.display_name.cmp(&b.display_name));

    TreeData { numbered, other }
}

/// Collect immediate subdirectories of a directory with their file counts.
fn collect_subdirs(dir: &Path) -> Vec<SubdirEntry> {
    let mut subdirs = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return subdirs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(SubdirEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                file_count: numbering::count_files_recursive(&path),
            });
        }
    }
    subdirs.sort_by(|a, b| a.name.cmp(&b.name));
    subdirs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn collect_tree_separates_numbered_and_other() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_quick-notes")).unwrap();
        fs::create_dir(dir.path().join("01_daily-logs")).unwrap();
        fs::create_dir(dir.path().join("random-stuff")).unwrap();
        fs::write(dir.path().join("file.md"), "hello").unwrap();

        let tree = collect_tree(dir.path());
        assert_eq!(tree.numbered.len(), 2);
        assert_eq!(tree.other.len(), 1); // only dirs, not files
    }

    #[test]
    fn collect_subdirs_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("02_projects");
        fs::create_dir_all(project.join("backend")).unwrap();
        fs::create_dir_all(project.join("frontend")).unwrap();
        fs::write(project.join("backend").join("notes.md"), "x").unwrap();
        fs::write(project.join("backend").join("todo.md"), "x").unwrap();
        fs::write(project.join("readme.md"), "x").unwrap();

        let subdirs = collect_subdirs(&project);
        assert_eq!(subdirs.len(), 2);
        // Sort order is alphabetical
        let backend = subdirs.iter().find(|s| s.name == "backend").unwrap();
        assert_eq!(backend.file_count, 2);
    }
}
