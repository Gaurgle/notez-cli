use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

use crate::Cli;

/// Generate shell completions and write them to stdout.
pub fn run_completions(shell: String) {
    let shell = match shell.to_lowercase().as_str() {
        "zsh" => Shell::Zsh,
        "bash" => Shell::Bash,
        "fish" => Shell::Fish,
        _ => {
            eprintln!("Unsupported shell: {}. Try: zsh, bash, fish", shell);
            std::process::exit(1);
        }
    };

    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "notez", &mut io::stdout());
}
