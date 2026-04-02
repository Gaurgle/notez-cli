use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use chrono::Local;

use crate::colors::Colors;
use crate::config::Config;
use crate::project;

pub fn run_log(global: bool, message: Vec<String>) {
    let config = Config::require();

    let logs_dir = if global {
        config.daily_logs_path()
    } else {
        project::local_notez_dir()
    };
    fs::create_dir_all(&logs_dir).expect("failed to create logs directory");

    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();
    let msg = message.join(" ");

    if msg.is_empty() {
        eprintln!("Usage: notez log <message>");
        std::process::exit(1);
    }

    let file_path = append_log_entry(&logs_dir, &date, &time, &msg);

    if !global {
        let home_dir = project::ensure_home_project_dir(&config);
        project::mirror_file_to_home(&file_path, &home_dir);
    }

    let colors = Colors::new();
    println!(
        "  {} logged to {}-daily-log.md",
        colors.green.apply_to("✓"),
        date
    );
}

/// Append a timestamped entry to the daily log file, creating the file with a header if new.
fn append_log_entry(dir: &Path, date: &str, time: &str, message: &str) -> std::path::PathBuf {
    let file_path = dir.join(format!("{}-daily-log.md", date));
    let is_new = !file_path.exists();

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .expect("failed to open log file");

    if is_new {
        writeln!(file, "# Daily Log - {}\n", date).expect("failed to write header");
    }
    writeln!(file, "{} - {}", time, message).expect("failed to write log entry");

    file_path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn creates_new_log_file_with_header() {
        let dir = tempfile::tempdir().unwrap();
        let date = "2026-04-02";
        let file = dir.path().join("2026-04-02-daily-log.md");

        append_log_entry(dir.path(), date, "10:30", "started working on notez");

        let content = fs::read_to_string(&file).unwrap();
        assert!(content.starts_with("# Daily Log - 2026-04-02\n"));
        assert!(content.contains("10:30 - started working on notez"));
    }

    #[test]
    fn appends_to_existing_log_file() {
        let dir = tempfile::tempdir().unwrap();
        let date = "2026-04-02";

        append_log_entry(dir.path(), date, "10:30", "first entry");
        append_log_entry(dir.path(), date, "11:00", "second entry");

        let file = dir.path().join("2026-04-02-daily-log.md");
        let content = fs::read_to_string(&file).unwrap();
        let header_count = content.matches("# Daily Log").count();
        assert_eq!(header_count, 1);
        assert!(content.contains("10:30 - first entry"));
        assert!(content.contains("11:00 - second entry"));
    }
}
