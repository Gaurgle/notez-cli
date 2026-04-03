use clap::{Parser, Subcommand};

mod colors;
mod commands;
mod config;
mod numbering;
mod project;
mod setup;
mod tui;

#[derive(Parser)]
#[command(name = "notez", about = "A CLI note-taking tool", version, disable_help_flag = true)]
pub struct Cli {
    /// Show help
    #[arg(short = 'h', long = "help", global = true)]
    help: bool,

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
    /// Create a demo project for screenshots
    Demo,
    /// Quick log entry (alias for log)
    Zlog {
        /// Log message
        message: Vec<String>,
    },
    /// Open daily logs directory (alias for logz)
    Zlogs,
    /// Generate shell completions
    Completions {
        /// Shell to generate for (zsh, bash, fish)
        shell: String,
    },
    /// Manage project todos
    Todo {
        /// Quick-add a todo item
        item: Option<String>,
    },
    /// Open an existing note
    Edit {
        /// Search term to fuzzy-match note filename
        term: Option<String>,
    },
    /// Interactive todo manager (alias for todo)
    Todoz,
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

fn print_help() {
    let c = colors::Colors::new();
    let div = c.divider(50);

    println!();
    println!("  {}", div);
    println!(
        "  {}  {}",
        c.lavender.apply_to("notez"),
        c.overlay.apply_to("a local-first note-taking tool")
    );
    println!("  {}", div);
    println!();

    let cmd = |name: &str, desc: &str| {
        println!(
            "    {:<28} {}",
            c.sapphire.apply_to(name),
            c.overlay.apply_to(desc)
        );
    };

    println!("  {}", c.mauve.apply_to("Notes"));
    cmd("notez add [title]", "create a note, open in editor");
    cmd("notez add [title] \"body\"", "create with content, no editor");
    cmd("notez add [title] --in", "create in a subdirectory (picker)");
    cmd("notez edit [term]", "open an existing note (fuzzy search)");
    println!();

    println!("  {}", c.mauve.apply_to("Daily Logs"));
    cmd("notez log <message>", "append to today's log");
    cmd("notez logz / logs", "browse daily logs");
    println!();

    println!("  {}", c.mauve.apply_to("Todos"));
    cmd("notez todo", "interactive todo manager");
    cmd("notez todo \"item\"", "quick-add a todo");
    cmd("notez todoz", "alias for notez todo");
    println!();

    println!("  {}", c.mauve.apply_to("Browse & Organize"));
    cmd("notez", "browse notes in yazi");
    cmd("notez tree", "interactive tree navigator");
    cmd("notez search <term>", "search note content (rg + fzf)");
    cmd("notez mkdir <name>", "create a numbered subdirectory");
    println!();

    println!("  {}", c.mauve.apply_to("Shortcuts"));
    cmd("notez zlog <message>", "same as notez log");
    cmd("notez zlogs", "same as notez logz");
    cmd("notez znote [title]", "same as notez add");
    cmd("notez todoz", "same as notez todo");
    println!();

    println!("  {}", c.mauve.apply_to("Setup"));
    cmd("notez setup", "run the setup wizard");
    cmd("notez completions <shell>", "generate shell completions");
    println!();

    println!("  {}", div);
    println!(
        "  {}    {}",
        c.sapphire.apply_to("-g"),
        c.overlay.apply_to("use before subcommand for global ~/notez/")
    );
    println!(
        "  {}    {}",
        c.sapphire.apply_to("-h"),
        c.overlay.apply_to("show this help")
    );
    println!("  {}", div);
    println!();

    let key = |k: &str, ks: &console::Style, desc: &str| {
        let pad = 5usize.saturating_sub(k.len());
        println!("    {}{}  {}", ks.apply_to(k), " ".repeat(pad), c.overlay.apply_to(desc));
    };

    println!("  {}", c.mauve.apply_to("Todo keys"));
    key("x", &c.sapphire, "check");
    key("a", &c.yellow, "almost");
    key("n", &c.green, "new");
    key("s", &c.lavender, "subtask");
    key("e", &c.mauve, "edit");
    key("d", &c.peach, "delete");
    key("h/l", &c.overlay, "fold");
    key("q", &c.peach, "quit");
    println!();

    println!("  {}", c.mauve.apply_to("Tree keys"));
    key("j/k", &c.sapphire, "move");
    key("h/l", &c.mauve, "fold");
    key("o", &c.green, "open");
    key("q", &c.peach, "quit");
    println!();
}

fn main() {
    let cli = Cli::parse();

    if cli.help {
        print_help();
        return;
    }

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
        Some(Commands::Demo) => commands::demo::run_demo(),
        Some(Commands::Zlog { message }) => commands::log::run_log(cli.global, message),
        Some(Commands::Zlogs) => commands::browse::run_logz(cli.global),
        Some(Commands::Completions { shell }) => commands::completions::run_completions(shell),
        Some(Commands::Todo { item }) => commands::todo::run_todo(cli.global, item),
        Some(Commands::Todoz) => commands::todo::run_todo(cli.global, None),
        Some(Commands::Edit { term }) => commands::edit::run_edit(cli.global, term),
        Some(Commands::Znote { title, r#in }) => {
            let (t, body) = split_title_body(title);
            commands::add::run_add(cli.global, t, r#in, body)
        }
    }
}
