use std::process::Command;

use crate::config::Config;
use crate::project;

pub fn run_browse(global: bool, public: bool) {
    let config = Config::require();
    let dir = project::resolve_notez_dir(&config, global, public);

    if !global && !public {
        project::ensure_gitignore();
    }

    if !dir.exists() {
        std::fs::create_dir_all(&dir).expect("failed to create notez directory");
    }

    if config.has_yazi {
        Command::new("yazi")
            .arg(&dir)
            .status()
            .expect("failed to launch yazi");
    } else {
        Command::new(&config.editor)
            .arg(&dir)
            .status()
            .expect("failed to launch editor");
    }
}

pub fn run_logz(global: bool, public: bool) {
    let config = Config::require();
    let dir = project::resolve_daily_logs_dir(&config, global, public);

    if !dir.exists() {
        std::fs::create_dir_all(&dir).expect("failed to create directory");
    }

    if config.has_yazi {
        Command::new("yazi")
            .arg(&dir)
            .status()
            .expect("failed to launch yazi");
    } else {
        Command::new(&config.editor)
            .arg(&dir)
            .status()
            .expect("failed to launch editor");
    }
}
