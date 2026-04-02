use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Application configuration loaded from a key=value file.
///
/// Default location: `$XDG_CONFIG_HOME/notez/config` (falls back to `~/.config/notez/config`).
#[derive(Debug, Clone)]
pub struct Config {
    pub notez_root: String,
    pub quick_notes_dir: String,
    pub daily_logs_dir: String,
    pub editor: String,
    pub has_fzf: bool,
    pub has_rg: bool,
    pub has_yazi: bool,
}

impl Config {
    /// Returns the default config file path based on XDG_CONFIG_HOME.
    pub fn config_path() -> PathBuf {
        let xdg = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = dirs::home_dir().expect("could not determine home directory");
            home.join(".config").to_string_lossy().into()
        });
        PathBuf::from(xdg).join("notez").join("config")
    }

    /// Load config from the default path, returning `None` if the file doesn't exist.
    pub fn load() -> Option<Self> {
        Self::load_from(&Self::config_path())
    }

    /// Load config from an arbitrary path. Returns `None` if the file can't be read.
    pub fn load_from(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        let mut notez_root = String::new();
        let mut quick_notes_dir = String::new();
        let mut daily_logs_dir = String::new();
        let mut editor = String::new();
        let mut has_fzf = false;
        let mut has_rg = false;
        let mut has_yazi = false;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                match key.trim() {
                    "NOTEZ_ROOT" => notez_root = expand_tilde(val.trim()),
                    "QUICK_NOTES_DIR" => quick_notes_dir = val.trim().to_string(),
                    "DAILY_LOGS_DIR" => daily_logs_dir = val.trim().to_string(),
                    "EDITOR" => editor = val.trim().to_string(),
                    "HAS_FZF" => has_fzf = val.trim() == "true",
                    "HAS_RG" => has_rg = val.trim() == "true",
                    "HAS_YAZI" => has_yazi = val.trim() == "true",
                    _ => {}
                }
            }
        }

        Some(Config {
            notez_root,
            quick_notes_dir,
            daily_logs_dir,
            editor,
            has_fzf,
            has_rg,
            has_yazi,
        })
    }

    /// Serialize this config to the given path, creating parent directories as needed.
    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = format!(
            "NOTEZ_ROOT={}\nQUICK_NOTES_DIR={}\nDAILY_LOGS_DIR={}\nEDITOR={}\nHAS_FZF={}\nHAS_RG={}\nHAS_YAZI={}\n",
            self.notez_root,
            self.quick_notes_dir,
            self.daily_logs_dir,
            self.editor,
            self.has_fzf,
            self.has_rg,
            self.has_yazi,
        );
        fs::write(path, content)
    }

    /// Save config to the default path.
    pub fn save(&self) -> io::Result<()> {
        self.save_to(&Self::config_path())
    }

    pub fn root_path(&self) -> PathBuf {
        PathBuf::from(&self.notez_root)
    }

    pub fn quick_notes_path(&self) -> PathBuf {
        self.root_path().join(&self.quick_notes_dir)
    }

    pub fn daily_logs_path(&self) -> PathBuf {
        self.root_path().join(&self.daily_logs_dir)
    }

    /// Load config or trigger the setup wizard if none exists.
    pub fn require() -> Self {
        match Self::load() {
            Some(config) => config,
            None => {
                eprintln!("notez is not set up yet. Running setup wizard...\n");
                crate::setup::run_setup();
                Self::load().expect("setup completed but config still missing")
            }
        }
    }
}

/// Expand a leading `~` to the user's home directory.
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Some(home) = dirs::home_dir() {
            return path.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

/// Check whether a CLI tool is available on PATH.
pub fn detect_tool(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn load_valid_config() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config");
        fs::write(
            &config_path,
            "NOTEZ_ROOT=~/notes\nQUICK_NOTES_DIR=00_quick-notes\nDAILY_LOGS_DIR=01_daily-logs\nEDITOR=nvim\nHAS_FZF=true\nHAS_RG=true\nHAS_YAZI=false\n",
        )
        .unwrap();

        let config = Config::load_from(&config_path).unwrap();
        assert!(config.notez_root.ends_with("notes"));
        assert_eq!(config.quick_notes_dir, "00_quick-notes");
        assert_eq!(config.daily_logs_dir, "01_daily-logs");
        assert_eq!(config.editor, "nvim");
        assert!(config.has_fzf);
        assert!(config.has_rg);
        assert!(!config.has_yazi);
    }

    #[test]
    fn load_missing_config_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("nonexistent");
        let result = Config::load_from(&config_path);
        assert!(result.is_none());
    }

    #[test]
    fn save_and_reload_config() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config");
        let config = Config {
            notez_root: "/tmp/test-notes".into(),
            quick_notes_dir: "00_quick-notes".into(),
            daily_logs_dir: "01_daily-logs".into(),
            editor: "vim".into(),
            has_fzf: true,
            has_rg: false,
            has_yazi: true,
        };
        config.save_to(&config_path).unwrap();

        let loaded = Config::load_from(&config_path).unwrap();
        assert_eq!(loaded.notez_root, config.notez_root);
        assert_eq!(loaded.editor, "vim");
        assert!(!loaded.has_rg);
    }

    #[test]
    fn expand_tilde_in_root() {
        let expanded = expand_tilde("~/notes");
        assert!(!expanded.starts_with('~'));
        assert!(expanded.ends_with("notes"));
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config");
        fs::write(
            &config_path,
            "# comment\n\nNOTEZ_ROOT=~/notes\nQUICK_NOTES_DIR=00_quick-notes\nDAILY_LOGS_DIR=01_daily-logs\nEDITOR=nvim\nHAS_FZF=true\nHAS_RG=true\nHAS_YAZI=true\n",
        )
        .unwrap();

        let config = Config::load_from(&config_path).unwrap();
        assert!(config.notez_root.ends_with("notes"));
    }
}
