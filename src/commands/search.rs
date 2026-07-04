use std::process::Command;

use crate::colors::Colors;
use crate::config::Config;
use crate::project;

pub fn run_search(global: bool, public: bool, term: String) {
    let config = Config::require();
    let root = project::resolve_notez_dir(&config, global, public);
    let colors = Colors::new();

    if global {
        crate::commands::sync::reconcile(&config);
    }

    if config.has_rg && config.has_fzf {
        // rg | fzf with bat preview
        let rg = Command::new("rg")
            .arg("--line-number")
            .arg("--color=always")
            .arg(&term)
            .arg(&root)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("failed to launch rg");

        Command::new("fzf")
            .arg("--ansi")
            .arg("--preview")
            .arg("bat --color=always --highlight-line {2} {1}")
            .arg("--preview-window")
            .arg("up:60%")
            .arg("--delimiter")
            .arg(":")
            .stdin(rg.stdout.unwrap())
            .status()
            .expect("failed to launch fzf");
    } else if config.has_rg {
        // rg only, print results directly
        Command::new("rg")
            .arg("--line-number")
            .arg("--color=always")
            .arg(&term)
            .arg(&root)
            .status()
            .expect("failed to launch rg");
    } else if config.has_fzf {
        // grep | fzf
        let grep = Command::new("grep")
            .arg("-r")
            .arg("-n")
            .arg(&term)
            .arg(&root)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("failed to launch grep");

        Command::new("fzf")
            .arg("--preview")
            .arg("bat --color=always {1}")
            .arg("--delimiter")
            .arg(":")
            .stdin(grep.stdout.unwrap())
            .status()
            .expect("failed to launch fzf");
    } else {
        // grep only
        let status = Command::new("grep")
            .arg("-r")
            .arg("-n")
            .arg(&term)
            .arg(&root)
            .status()
            .expect("failed to launch grep");

        if !status.success() {
            println!(
                "  {} no results for \"{}\"",
                colors.overlay.apply_to("─"),
                term
            );
        }
    }
}
