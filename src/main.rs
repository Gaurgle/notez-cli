use clap::{Parser, Subcommand};

mod colors;
mod commands;
mod config;
mod numbering;
mod project;
mod setup;

#[derive(Parser)]
#[command(name = "notez", about = "A CLI note-taking tool", version)]
struct Cli {
    /// Use global ~/notez/ instead of local ./notez/
    #[arg(short = 'g', long = "global", global = true)]
    global: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new note
    Add {
        /// Note title (defaults to "untitled")
        title: Vec<String>,
        /// Target directory (fzf picker if flag given without value)
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        r#in: Option<String>,
    },
    /// Append a timestamped entry to today's daily log
    Log {
        /// Log message
        message: Vec<String>,
    },
    /// Open daily logs directory
    Logz,
    /// Open daily logs directory (alias for logz)
    Logs,
    /// Create a new numbered subdirectory
    Mkdir {
        /// Directory name
        name: Vec<String>,
    },
    /// Search notes content
    Search {
        /// Search term
        term: String,
    },
    /// Show directory tree
    Tree,
    /// Run the setup wizard
    Setup,
    /// Quick log entry (alias for log)
    Zlog {
        /// Log message
        message: Vec<String>,
    },
    /// Open daily logs directory (alias for logz)
    Zlogs,
    /// Quick new note (alias for add)
    Znote {
        /// Note title (defaults to "untitled")
        title: Vec<String>,
        /// Target directory
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        r#in: Option<String>,
    },
}

/// Split args into title words and optional body.
/// Shell quotes mean args with spaces were quoted by the user.
/// e.g., `notez add my idea "this is the note"` → args: ["my", "idea", "this is the note"]
/// Words without spaces = title, last arg with spaces = body.
fn split_title_body(args: Vec<String>) -> (Option<String>, Option<String>) {
    if args.is_empty() {
        return (None, None);
    }

    let mut title_parts = Vec::new();
    let mut body = None;

    for arg in &args {
        if arg.contains(' ') {
            body = Some(arg.clone());
        } else {
            title_parts.push(arg.clone());
        }
    }

    let title = if title_parts.is_empty() {
        None
    } else {
        Some(title_parts.join(" "))
    };

    (title, body)
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => commands::browse::run_browse(cli.global),
        Some(Commands::Add { title, r#in }) => {
            let (t, body) = split_title_body(title);
            commands::add::run_add(cli.global, t, r#in, body)
        }
        Some(Commands::Log { message }) => commands::log::run_log(cli.global, message),
        Some(Commands::Logz) | Some(Commands::Logs) => commands::browse::run_logz(cli.global),
        Some(Commands::Mkdir { name }) => commands::mkdir::run_mkdir(cli.global, name),
        Some(Commands::Search { term }) => commands::search::run_search(cli.global, term),
        Some(Commands::Tree) => commands::tree::run_tree(cli.global),
        Some(Commands::Setup) => setup::run_setup(),
        Some(Commands::Zlog { message }) => commands::log::run_log(cli.global, message),
        Some(Commands::Zlogs) => commands::browse::run_logz(cli.global),
        Some(Commands::Znote { title, r#in }) => {
            let (t, body) = split_title_body(title);
            commands::add::run_add(cli.global, t, r#in, body)
        }
    }
}
