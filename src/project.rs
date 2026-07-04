use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::numbering;

/// Detect the project name for the given directory.
/// Uses git repo name if available, falls back to directory name.
pub fn detect_project_name(dir: &Path) -> String {
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(dir)
        .stderr(std::process::Stdio::null())
        .output()
    {
        if output.status.success() {
            let toplevel = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Some(name) = Path::new(&toplevel).file_name() {
                return numbering::sanitize_name(&name.to_string_lossy());
            }
        }
    }

    dir.file_name()
        .map(|n| numbering::sanitize_name(&n.to_string_lossy()))
        .unwrap_or_else(|| "unnamed".into())
}

/// Returns the private local notez directory (.notez/).
pub fn local_private_dir() -> PathBuf {
    std::env::current_dir()
        .expect("failed to get current directory")
        .join(".notez")
}

/// Returns the public local notez directory (notez/).
pub fn local_public_dir() -> PathBuf {
    std::env::current_dir()
        .expect("failed to get current directory")
        .join("notez")
}

/// Returns the local notez directory based on public flag.
pub fn local_notez_dir(public: bool) -> PathBuf {
    if public {
        local_public_dir()
    } else {
        local_private_dir()
    }
}

/// Resolve the working directory for a command.
pub fn resolve_notez_dir(config: &Config, global: bool, public: bool) -> PathBuf {
    if global {
        config.root_path()
    } else {
        local_notez_dir(public)
    }
}

/// Resolve the quick notes directory.
pub fn resolve_quick_notes_dir(config: &Config, global: bool, public: bool) -> PathBuf {
    if global {
        config.quick_notes_path()
    } else {
        local_notez_dir(public).join(&config.quick_notes_dir)
    }
}

/// Resolve the daily logs directory.
pub fn resolve_daily_logs_dir(config: &Config, global: bool, public: bool) -> PathBuf {
    if global {
        config.daily_logs_path()
    } else {
        local_notez_dir(public).join(&config.daily_logs_dir)
    }
}

/// Auto-add .notez to .gitignore if not already there.
/// Uses the no-slash form so the rule also matches when .notez is a symlink.
pub fn ensure_gitignore() {
    let cwd = std::env::current_dir().unwrap_or_default();
    let gitignore = cwd.join(".gitignore");
    let entry = ".notez";

    if let Ok(content) = std::fs::read_to_string(&gitignore) {
        if content.lines().any(|l| {
            let t = l.trim();
            t == ".notez" || t == ".notez/"
        }) {
            return;
        }
    }

    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore)
    {
        writeln!(f, "{}", entry).ok();
    }
}

/// Nerdfont icons for private/public.
pub const ICON_PRIVATE: &str = "\u{f023}"; // lock
pub const ICON_PUBLIC: &str = "\u{f0ac}"; // globe

/// Ensure the project has a numbered directory in the home notez root.
/// Returns the path to the project's home dir (e.g., ~/notez/02_my-project/).
pub fn ensure_home_project_dir(config: &Config) -> PathBuf {
    let cwd = std::env::current_dir().expect("failed to get current directory");
    let project_name = detect_project_name(&cwd);
    let project_home_dir = ensure_home_dir_for(&config.root_path(), &project_name);

    let mut mapping = ProjectMapping::load();
    if mapping.get_path(&project_name).is_none() {
        mapping.add(&project_name, &cwd.to_string_lossy());
        mapping.save().expect("failed to save project mapping");
    }

    project_home_dir
}

/// Find or create the numbered home dir for a project name, without
/// touching the project mapping. Usable outside the project's cwd (sync).
pub fn ensure_home_dir_for(home_root: &Path, project_name: &str) -> PathBuf {
    let dirs = numbering::scan_numbered_dirs(home_root);
    if let Some(dir) = dirs.iter().find(|d| d.name == project_name) {
        return home_root.join(&dir.full_name);
    }

    let number =
        numbering::next_available_number(home_root).expect("all 100 directory slots are taken");
    let dir_name = format!("{:02}_{}", number, project_name);
    let project_home_dir = home_root.join(&dir_name);
    fs::create_dir_all(&project_home_dir).expect("failed to create project home dir");
    project_home_dir
}

/// Create a symlink for a file in the home project directory.
/// Returns true if a new link was created.
pub fn mirror_file_to_home(source: &Path, home_project_dir: &Path) -> bool {
    let file_name = source.file_name().expect("no file name");
    mirror_to_home(source, &home_project_dir.join(file_name))
}

/// Create a symlink for a directory in the home project directory.
/// Returns true if a new link was created.
pub fn mirror_dir_to_home(source_dir: &Path, home_project_dir: &Path) -> bool {
    let dir_name = source_dir.file_name().expect("no dir name");
    mirror_to_home(source_dir, &home_project_dir.join(dir_name))
}

fn mirror_to_home(source: &Path, link: &Path) -> bool {
    if link.exists() || link.is_symlink() {
        return false;
    }

    #[cfg(unix)]
    return std::os::unix::fs::symlink(source, link).is_ok();
    #[cfg(not(unix))]
    false
}

// --- Project Mapping ---

/// Maps project names to their local working directory paths.
/// Stored as a simple `name=path` text file.
pub struct ProjectMapping {
    pub projects: HashMap<String, String>,
}

impl ProjectMapping {
    pub fn new() -> Self {
        Self {
            projects: HashMap::new(),
        }
    }

    fn mapping_path() -> PathBuf {
        let xdg = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = dirs::home_dir().expect("could not determine home directory");
            home.join(".config").to_string_lossy().into()
        });
        PathBuf::from(xdg).join("notez").join("projects")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::mapping_path())
    }

    pub fn load_from(path: &Path) -> Self {
        let mut mapping = Self::new();
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((name, path)) = line.split_once('=') {
                    mapping
                        .projects
                        .insert(name.trim().to_string(), path.trim().to_string());
                }
            }
        }
        mapping
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&Self::mapping_path())
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content: String = self
            .projects
            .iter()
            .map(|(name, path)| format!("{}={}", name, path))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, content)
    }

    pub fn add(&mut self, name: &str, path: &str) {
        self.projects.insert(name.to_string(), path.to_string());
    }

    pub fn get_path(&self, name: &str) -> Option<String> {
        self.projects.get(name).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detect_project_name_from_git() {
        let dir = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .unwrap();

        let name = detect_project_name(dir.path());
        // tempdir names are random, just verify it's not empty and is sanitized
        assert!(!name.is_empty());
        assert!(!name.contains(' '));
    }

    #[test]
    fn detect_project_name_no_git_uses_dir_name() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("my-project");
        fs::create_dir(&sub).unwrap();

        let name = detect_project_name(&sub);
        assert_eq!(name, "my-project");
    }

    #[test]
    fn load_empty_project_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects");
        let mapping = ProjectMapping::load_from(&path);
        assert!(mapping.projects.is_empty());
    }

    #[test]
    fn save_and_reload_project_mapping() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects");
        let mut mapping = ProjectMapping::new();
        mapping.add("my-project", "/home/user/repos/my-project");
        mapping.save_to(&path).unwrap();

        let loaded = ProjectMapping::load_from(&path);
        assert_eq!(
            loaded.get_path("my-project"),
            Some("/home/user/repos/my-project".to_string())
        );
    }

    #[test]
    fn symlink_file_creates_link() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let target_dir = dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        let source_file = source_dir.join("note.md");
        fs::write(&source_file, "hello").unwrap();

        mirror_file_to_home(&source_file, &target_dir);

        let link = target_dir.join("note.md");
        assert!(link.exists());
        assert!(link.is_symlink());
        assert_eq!(fs::read_to_string(&link).unwrap(), "hello");
    }

    #[test]
    fn symlink_dir_creates_link() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source").join("02_subdir");
        let target_dir = dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(source_dir.join("file.md"), "x").unwrap();

        mirror_dir_to_home(&source_dir, &target_dir);

        let link = target_dir.join("02_subdir");
        assert!(link.exists());
        assert!(link.is_symlink());
    }

    #[test]
    fn symlink_skips_if_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = dir.path().join("source");
        let target_dir = dir.path().join("target");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        let source_file = source_dir.join("note.md");
        fs::write(&source_file, "hello").unwrap();

        mirror_file_to_home(&source_file, &target_dir);
        mirror_file_to_home(&source_file, &target_dir); // should not panic

        let link = target_dir.join("note.md");
        assert!(link.is_symlink());
    }
}
