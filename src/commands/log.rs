use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use chrono::Local;

use crate::colors::Colors;
use crate::config::Config;

pub fn run_log(message: Vec<String>) {
    let config = Config::require();
    let logs_path = config.daily_logs_path();
    fs::create_dir_all(&logs_path).expect("failed to create daily logs directory");

    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();
    let msg = message.join(" ");

    if msg.is_empty() {
        eprintln!("Usage: notez log <message>");
        std::process::exit(1);
    }

    append_log_entry(&logs_path, &date, &time, &msg);

    let colors = Colors::new();
    println!(
        "  {} logged to {}-daily-log.md",
        colors.green.apply_to("✓"),
        date
    );
}

/// Append a timestamped entry to the daily log file, creating the file with a header if new.
fn append_log_entry(dir: &Path, date: &str, time: &str, message: &str) {
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
