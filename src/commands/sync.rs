use std::fs;
use std::path::{Path, PathBuf};

use crate::colors::Colors;
use crate::config::Config;
use crate::numbering;
use crate::project::{self, ProjectMapping};

/// What a reconcile pass changed.
#[derive(Debug, Default)]
pub struct SyncReport {
    /// Symlinks created in ~/notez/ for notes that existed only locally.
    pub linked: Vec<PathBuf>,
    /// Broken symlinks removed from ~/notez/.
    pub pruned: Vec<PathBuf>,
    /// Mapped projects whose path no longer exists on disk.
    pub missing_projects: Vec<String>,
}

impl SyncReport {
    pub fn is_clean(&self) -> bool {
        self.linked.is_empty() && self.pruned.is_empty() && self.missing_projects.is_empty()
    }
}

/// Self-healing pass over the global notez root: mirror any local notes the
/// CLI never saw (created out-of-band by editors, agents, git) and prune
/// symlinks whose targets are gone. Idempotent and cheap; global-mode
/// commands run it on startup so the global view can't go stale.
pub fn reconcile(config: &Config) -> SyncReport {
    let mapping = ProjectMapping::load();
    sync_mapping(&config.root_path(), &mapping)
}

/// Core reconcile logic, parameterized for tests.
pub fn sync_mapping(home_root: &Path, mapping: &ProjectMapping) -> SyncReport {
    let mut report = SyncReport::default();

    for (name, path) in &mapping.projects {
        let project_path = Path::new(path);
        if !project_path.exists() {
            report.missing_projects.push(name.clone());
            continue;
        }

        let entries = collect_local_note_entries(project_path);
        if entries.is_empty() {
            continue;
        }

        let home_dir = project::ensure_home_dir_for(home_root, name);
        for source in entries {
            let created = if source.is_dir() {
                project::mirror_dir_to_home(&source, &home_dir)
            } else {
                project::mirror_file_to_home(&source, &home_dir)
            };
            if created {
                let file_name = source.file_name().expect("readdir entry has a name");
                report.linked.push(home_dir.join(file_name));
            }
        }
    }

    report.pruned = prune_broken_links(home_root);
    report
}

/// Top-level entries of a project's private (.notez/) and public (notez/)
/// stores - the same granularity the mirror-on-create path links at.
/// Symlinked entries are skipped: they point back into the global root
/// (pre-2026-04-15 inverted layout) or are stale, never out-of-band notes.
fn collect_local_note_entries(project_path: &Path) -> Vec<PathBuf> {
    let stores = [project_path.join(".notez"), project_path.join("notez")];
    let mut entries = Vec::new();
    for store in stores.iter().filter(|s| s.is_dir()) {
        let Ok(dir) = fs::read_dir(store) else {
            continue;
        };
        for entry in dir.flatten() {
            let is_hidden = entry.file_name().to_string_lossy().starts_with('.');
            if !is_hidden && !entry.path().is_symlink() {
                entries.push(entry.path());
            }
        }
    }
    entries
}

/// Remove symlinks in numbered home dirs whose targets no longer exist
/// (project deleted or moved). Only touches the top level, where mirrors live.
fn prune_broken_links(home_root: &Path) -> Vec<PathBuf> {
    let mut pruned = Vec::new();
    for dir in numbering::scan_numbered_dirs(home_root) {
        let Ok(entries) = fs::read_dir(home_root.join(&dir.full_name)) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // exists() follows the link, so symlink + !exists = dangling
            if path.is_symlink() && !path.exists() && fs::remove_file(&path).is_ok() {
                pruned.push(path);
            }
        }
    }
    pruned
}

/// Explicit `notez sync`: run the reconcile pass and report what changed.
pub fn run_sync() {
    let config = Config::require();
    let report = reconcile(&config);
    let colors = Colors::new();

    if report.is_clean() {
        println!(
            "  {} {}",
            colors.green.apply_to("✓"),
            colors.overlay.apply_to("global view already in sync")
        );
        return;
    }

    for link in &report.linked {
        println!(
            "  {} linked {}",
            colors.green.apply_to("+"),
            colors.sapphire.apply_to(display_relative(link, &config))
        );
    }
    for path in &report.pruned {
        println!(
            "  {} pruned {}",
            colors.peach.apply_to("-"),
            colors.overlay.apply_to(display_relative(path, &config))
        );
    }
    for name in &report.missing_projects {
        println!(
            "  {} project path missing for {}",
            colors.yellow.apply_to("!"),
            colors.overlay.apply_to(name)
        );
    }
}

fn display_relative(path: &Path, config: &Config) -> String {
    path.strip_prefix(config.root_path())
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn mapping_with(name: &str, path: &Path) -> ProjectMapping {
        let mut mapping = ProjectMapping::new();
        mapping.add(name, &path.to_string_lossy());
        mapping
    }

    #[test]
    fn links_out_of_band_private_note() {
        let tmp = tempfile::tempdir().unwrap();
        let home_root = tmp.path().join("notez");
        let repo = tmp.path().join("myrepo");
        fs::create_dir_all(home_root.join("03_myrepo")).unwrap();
        fs::create_dir_all(repo.join(".notez")).unwrap();
        fs::write(repo.join(".notez/review.md"), "# review").unwrap();

        let report = sync_mapping(&home_root, &mapping_with("myrepo", &repo));

        let link = home_root.join("03_myrepo/review.md");
        assert_eq!(report.linked, vec![link.clone()]);
        assert!(link.is_symlink());
        assert_eq!(fs::read_to_string(&link).unwrap(), "# review");
    }

    #[test]
    fn links_public_store_and_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let home_root = tmp.path().join("notez");
        let repo = tmp.path().join("myrepo");
        fs::create_dir_all(home_root.join("00_myrepo")).unwrap();
        fs::create_dir_all(repo.join(".notez/00_quick-notes")).unwrap();
        fs::create_dir_all(repo.join("notez")).unwrap();
        fs::write(repo.join("notez/public.md"), "public").unwrap();

        let report = sync_mapping(&home_root, &mapping_with("myrepo", &repo));

        assert_eq!(report.linked.len(), 2);
        assert!(home_root.join("00_myrepo/00_quick-notes").is_symlink());
        assert!(home_root.join("00_myrepo/public.md").is_symlink());
    }

    #[test]
    fn second_run_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let home_root = tmp.path().join("notez");
        let repo = tmp.path().join("myrepo");
        fs::create_dir_all(&home_root).unwrap();
        fs::create_dir_all(repo.join(".notez")).unwrap();
        fs::write(repo.join(".notez/note.md"), "x").unwrap();
        let mapping = mapping_with("myrepo", &repo);

        let first = sync_mapping(&home_root, &mapping);
        let second = sync_mapping(&home_root, &mapping);

        assert_eq!(first.linked.len(), 1);
        assert!(second.is_clean());
    }

    #[test]
    fn allocates_home_dir_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let home_root = tmp.path().join("notez");
        let repo = tmp.path().join("myrepo");
        fs::create_dir_all(&home_root).unwrap();
        fs::create_dir_all(repo.join(".notez")).unwrap();
        fs::write(repo.join(".notez/note.md"), "x").unwrap();

        sync_mapping(&home_root, &mapping_with("myrepo", &repo));

        assert!(home_root.join("00_myrepo/note.md").is_symlink());
    }

    #[test]
    fn skips_project_without_local_notes() {
        let tmp = tempfile::tempdir().unwrap();
        let home_root = tmp.path().join("notez");
        let repo = tmp.path().join("bare-repo");
        fs::create_dir_all(&home_root).unwrap();
        fs::create_dir_all(&repo).unwrap();

        let report = sync_mapping(&home_root, &mapping_with("bare-repo", &repo));

        assert!(report.is_clean());
        // No empty home dir allocated for a project with nothing to mirror
        assert!(numbering::scan_numbered_dirs(&home_root).is_empty());
    }

    #[test]
    fn skips_hidden_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let home_root = tmp.path().join("notez");
        let repo = tmp.path().join("myrepo");
        fs::create_dir_all(&home_root).unwrap();
        fs::create_dir_all(repo.join(".notez")).unwrap();
        fs::write(repo.join(".notez/.tags"), "").unwrap();
        fs::write(repo.join(".notez/note.md"), "x").unwrap();

        let report = sync_mapping(&home_root, &mapping_with("myrepo", &repo));

        assert_eq!(report.linked.len(), 1);
        assert!(report.linked[0].ends_with("note.md"));
    }

    #[test]
    fn skips_symlinked_entries_in_local_store() {
        let tmp = tempfile::tempdir().unwrap();
        let home_root = tmp.path().join("notez");
        let repo = tmp.path().join("myrepo");
        fs::create_dir_all(&home_root).unwrap();
        fs::create_dir_all(repo.join(".notez")).unwrap();
        // Stale inverted-layout link pointing back into the global root
        std::os::unix::fs::symlink(home_root.join("12_gone"), repo.join(".notez/12_gone")).unwrap();
        fs::write(repo.join(".notez/real.md"), "x").unwrap();
        let mapping = mapping_with("myrepo", &repo);

        let report = sync_mapping(&home_root, &mapping);

        assert_eq!(report.linked.len(), 1);
        assert!(report.linked[0].ends_with("real.md"));
        // And stays clean on the next pass - no link/prune churn
        assert!(sync_mapping(&home_root, &mapping).is_clean());
    }

    #[test]
    fn reports_missing_project_path() {
        let tmp = tempfile::tempdir().unwrap();
        let home_root = tmp.path().join("notez");
        fs::create_dir_all(&home_root).unwrap();
        let gone = tmp.path().join("deleted-repo");

        let report = sync_mapping(&home_root, &mapping_with("deleted-repo", &gone));

        assert_eq!(report.missing_projects, vec!["deleted-repo".to_string()]);
    }

    #[test]
    fn prunes_dangling_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let home_root = tmp.path().join("notez");
        let home_dir = home_root.join("05_ghost");
        fs::create_dir_all(&home_dir).unwrap();
        let dangling = home_dir.join("gone.md");
        std::os::unix::fs::symlink(tmp.path().join("nonexistent.md"), &dangling).unwrap();

        let report = sync_mapping(&home_root, &ProjectMapping::new());

        assert_eq!(report.pruned, vec![dangling.clone()]);
        assert!(!dangling.is_symlink());
    }

    #[test]
    fn keeps_valid_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let home_root = tmp.path().join("notez");
        let home_dir = home_root.join("05_alive");
        fs::create_dir_all(&home_dir).unwrap();
        let target = tmp.path().join("real.md");
        fs::write(&target, "real").unwrap();
        let link = home_dir.join("real.md");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let report = sync_mapping(&home_root, &ProjectMapping::new());

        assert!(report.pruned.is_empty());
        assert!(link.is_symlink());
    }
}
