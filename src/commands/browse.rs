use std::process::Command;

use crate::config::Config;

pub fn run_browse(global: bool) {
    let _ = global;
    let config = Config::require();
    let root = config.root_path();

    if config.has_yazi {
        Command::new("yazi")
            .arg(&root)
            .status()
            .expect("failed to launch yazi");
    } else {
        Command::new(&config.editor)
            .arg(&root)
            .status()
            .expect("failed to launch editor");
    }
}

pub fn run_logz(global: bool) {
    let _ = global;
    let config = Config::require();
    let logs_path = config.daily_logs_path();

    if !logs_path.exists() {
        std::fs::create_dir_all(&logs_path).expect("failed to create daily logs directory");
    }

    if config.has_yazi {
        Command::new("yazi")
            .arg(&logs_path)
            .status()
            .expect("failed to launch yazi");
    } else {
        Command::new(&config.editor)
            .arg(&logs_path)
            .status()
            .expect("failed to launch editor");
    }
}
