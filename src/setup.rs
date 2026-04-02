use std::fs;
use std::path::PathBuf;

use dialoguer::{Confirm, Select};
use rustyline::DefaultEditor;

use crate::colors::Colors;
use crate::config::{Config, detect_tool, expand_tilde};
use crate::numbering;

const DIV_W: usize = 50;

fn prompt_input(colors: &Colors, rl: &mut DefaultEditor, label: &str, default: &str) -> String {
    let prompt = format!(
        "    {} {}: ",
        colors.overlay.apply_to(label),
        colors.surface.apply_to(format!("[{}]", default))
    );

    match rl.readline_with_initial(&prompt, (default, "")) {
        Ok(line) => {
            let input = line.trim().to_string();
            if input.is_empty() {
                default.to_string()
            } else {
                input
            }
        }
        Err(_) => default.to_string(),
    }
}

fn step_header(colors: &Colors, step: usize, total: usize, title: &str) {
    println!();
    println!(
        "  {} {} {}",
        colors.surface.apply_to("──"),
        colors.mauve.apply_to(format!("Step {} of {} — {}", step, total, title)),
        colors.surface.apply_to("─".repeat(DIV_W.saturating_sub(title.len() + 16)))
    );
    println!();
}

pub fn run_setup() {
    let colors = Colors::new();
    let existing = Config::load();
    let mut rl = DefaultEditor::new().expect("failed to initialize input handler");

    // Header
    println!();
    println!("  {}", colors.divider(DIV_W));
    println!(
        "  {}  {}",
        colors.lavender.apply_to("notez"),
        colors.overlay.apply_to("setup")
    );
    println!("  {}", colors.divider(DIV_W));

    let total = 7;

    // Defaults from existing config or sensible defaults
    let default_root = existing
        .as_ref()
        .map(|c| c.notez_root.clone())
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap()
                .join("notez")
                .to_string_lossy()
                .to_string()
        });
    let default_quick = existing
        .as_ref()
        .map(|c| c.quick_notes_dir.replace("00_", ""))
        .unwrap_or_else(|| "quick-notes".into());
    let default_daily = existing
        .as_ref()
        .map(|c| c.daily_logs_dir.replace("01_", ""))
        .unwrap_or_else(|| "daily-logs".into());
    let default_editor = existing
        .as_ref()
        .map(|c| c.editor.clone())
        .unwrap_or_else(|| {
            std::env::var("EDITOR").unwrap_or_else(|_| "vim".into())
        });

    // Wizard state
    let mut root_dir = String::new();
    let mut quick_name = String::new();
    let mut daily_name = String::new();
    let mut editor = String::new();
    let mut has_fzf = false;
    let mut has_rg = false;
    let mut has_yazi = false;

    let mut step = 1;

    loop {
        match step {
            // Step 1: Welcome
            1 => {
                step_header(&colors, step, total, "Welcome");

                println!("    Let's get notez set up. This will only take a moment.");
                println!();
                println!(
                    "    {} Each step has a suggested default shown in brackets.",
                    colors.overlay.apply_to("•")
                );
                println!(
                    "    {} Just press {} to accept it, or type something new.",
                    colors.overlay.apply_to("•"),
                    colors.green.apply_to("Enter")
                );
                println!(
                    "    {} Type {} at any prompt to go back a step.",
                    colors.overlay.apply_to("•"),
                    colors.yellow.apply_to("b")
                );

                step = 2;
            }

            // Step 2: Root directory
            2 => {
                step_header(&colors, step, total, "Where to keep your notes");

                let display_root = default_root.replacen(
                    &dirs::home_dir().unwrap().to_string_lossy().to_string(),
                    "~",
                    1,
                );

                println!("    Your notes will be stored at:");
                println!();
                println!(
                    "      {}",
                    colors.sapphire.apply_to(&display_root)
                );
                println!();

                let change = Confirm::new()
                    .with_prompt("    Use this location?")
                    .default(true)
                    .interact()
                    .unwrap();

                if !change {
                    println!();
                    let input = prompt_input(&colors, &mut rl, "Folder path", &default_root);

                    if input == "b" {
                        step = 1;
                        continue;
                    }
                    root_dir = expand_tilde(&input);
                } else {
                    root_dir = expand_tilde(&default_root);
                }
                let root_path = PathBuf::from(&root_dir);

                if !root_path.exists() {
                    println!();
                    println!(
                        "    That folder doesn't exist yet: {}",
                        colors.yellow.apply_to(&root_dir)
                    );
                    let create = Confirm::new()
                        .with_prompt("    Would you like to create it?")
                        .default(true)
                        .interact()
                        .unwrap();

                    if !create {
                        continue;
                    }
                    fs::create_dir_all(&root_path).expect("failed to create root directory");
                    println!(
                        "    {} Folder created!",
                        colors.green.apply_to("✓")
                    );
                }

                step = 3;
            }

            // Step 3: Scan for conflicts
            3 => {
                step_header(&colors, step, total, "Quick check");

                println!("    Looking for any existing numbered folders...");
                println!();

                let root_path = PathBuf::from(&root_dir);
                let existing_numbered = numbering::scan_numbered_dirs(&root_path);

                if !existing_numbered.is_empty() {
                    println!(
                        "    {} Found some folders that use the 00-99 numbering notez uses:",
                        colors.yellow.apply_to("heads up!")
                    );
                    println!();
                    for d in &existing_numbered {
                        println!(
                            "      {}  {}",
                            colors.overlay.apply_to("•"),
                            colors.sapphire.apply_to(&d.full_name)
                        );
                    }
                    println!();
                    println!("    These won't be changed. notez will just work around them.");
                    println!();

                    let choices = vec![
                        "Sounds good, keep them as they are",
                        "Let me pick a different folder instead",
                    ];

                    let choice = Select::new()
                        .with_prompt("    What would you like to do?")
                        .items(&choices)
                        .default(0)
                        .interact()
                        .unwrap();

                    match choice {
                        1 => {
                            step = 2;
                            continue;
                        }
                        _ => {}
                    }
                } else {
                    println!(
                        "    {} All clear, no conflicts.",
                        colors.green.apply_to("✓")
                    );
                }

                step = 4;
            }

            // Step 4: Quick notes dir name
            4 => {
                step_header(&colors, step, total, "Quick notes folder");

                println!("    When you jot down a quick note, it goes here.");
                println!(
                    "    notez will add a {} prefix automatically.",
                    colors.sapphire.apply_to("00_")
                );
                println!();

                let input = prompt_input(&colors, &mut rl, "Folder name", &default_quick);

                if input == "b" {
                    step = 3;
                    continue;
                }

                quick_name = format!("00_{}", numbering::sanitize_name(&input));
                println!(
                    "\n    {} Will create: {}",
                    colors.green.apply_to("✓"),
                    colors.sapphire.apply_to(&quick_name)
                );

                step = 5;
            }

            // Step 5: Daily logs dir name
            5 => {
                step_header(&colors, step, total, "Daily logs folder");

                println!("    Your daily log entries will be saved here.");
                println!(
                    "    notez will add a {} prefix automatically.",
                    colors.sapphire.apply_to("01_")
                );
                println!();

                let input = prompt_input(&colors, &mut rl, "Folder name", &default_daily);

                if input == "b" {
                    step = 4;
                    continue;
                }

                daily_name = format!("01_{}", numbering::sanitize_name(&input));
                println!(
                    "\n    {} Will create: {}",
                    colors.green.apply_to("✓"),
                    colors.sapphire.apply_to(&daily_name)
                );

                step = 6;
            }

            // Step 6: Detect tools
            6 => {
                step_header(&colors, step, total, "Checking your tools");

                println!("    notez works best with a few optional tools.");
                println!("    Don't worry if some are missing — there are built-in alternatives.");
                println!();

                editor = std::env::var("EDITOR").unwrap_or_else(|_| default_editor.clone());
                has_fzf = detect_tool("fzf");
                has_rg = detect_tool("rg");
                has_yazi = detect_tool("yazi");

                let found = |name: &str, ok: bool, detail: &str| {
                    if ok {
                        println!(
                            "    {}  {:<10} {}",
                            colors.green.apply_to("✓"),
                            name,
                            colors.overlay.apply_to(detail)
                        );
                    } else {
                        println!(
                            "    {}  {:<10} {}",
                            colors.yellow.apply_to("–"),
                            name,
                            colors.overlay.apply_to("not found, will use built-in alternative")
                        );
                    }
                };

                found("editor", !editor.is_empty(), &format!("using {}", editor));
                found("fzf", has_fzf, "for fuzzy search and selection");
                found("rg", has_rg, "for fast note searching");
                found("yazi", has_yazi, "for browsing your notes folder");

                if editor.is_empty() {
                    println!();
                    println!("    notez needs a text editor to open your notes.");
                    let input = prompt_input(&colors, &mut rl, "Editor", "vim");

                    if input == "b" {
                        step = 5;
                        continue;
                    }
                    editor = input;
                }

                step = 7;
            }

            // Step 7: Confirm & create
            7 => {
                step_header(&colors, step, total, "Ready to go!");

                println!("    Here's what notez will set up:");
                println!();
                println!(
                    "    {}  {}",
                    colors.overlay.apply_to("Notes folder:"),
                    colors.sapphire.apply_to(&root_dir)
                );
                println!(
                    "    {}  {}",
                    colors.overlay.apply_to("Quick notes: "),
                    colors.sapphire.apply_to(&quick_name)
                );
                println!(
                    "    {}  {}",
                    colors.overlay.apply_to("Daily logs:  "),
                    colors.sapphire.apply_to(&daily_name)
                );
                println!(
                    "    {}  {}",
                    colors.overlay.apply_to("Editor:      "),
                    colors.sapphire.apply_to(&editor)
                );
                println!();

                let confirm = Confirm::new()
                    .with_prompt("    Look good? Create everything?")
                    .default(true)
                    .interact()
                    .unwrap();

                if !confirm {
                    step = 2;
                    continue;
                }

                // Create directories
                let root_path = PathBuf::from(&root_dir);
                fs::create_dir_all(root_path.join(&quick_name))
                    .expect("failed to create quick notes dir");
                fs::create_dir_all(root_path.join(&daily_name))
                    .expect("failed to create daily logs dir");

                // Save config
                let config = Config {
                    notez_root: root_dir.clone(),
                    quick_notes_dir: quick_name.clone(),
                    daily_logs_dir: daily_name.clone(),
                    editor: editor.clone(),
                    has_fzf,
                    has_rg,
                    has_yazi,
                };
                config.save().expect("failed to save config");

                // Done!
                println!();
                println!("  {}", colors.divider(DIV_W));
                println!(
                    "  {}  You're all set!",
                    colors.green.apply_to("✓")
                );
                println!("  {}", colors.divider(DIV_W));
                println!();
                println!("    Here are some things to try:");
                println!();
                println!(
                    "    {}  {}       {}",
                    colors.overlay.apply_to("•"),
                    colors.sapphire.apply_to("notez add"),
                    colors.overlay.apply_to("create a new note")
                );
                println!(
                    "    {}  {}       {}",
                    colors.overlay.apply_to("•"),
                    colors.sapphire.apply_to("notez log"),
                    colors.overlay.apply_to("write a quick log entry")
                );
                println!(
                    "    {}  {}      {}",
                    colors.overlay.apply_to("•"),
                    colors.sapphire.apply_to("notez tree"),
                    colors.overlay.apply_to("see your notes at a glance")
                );
                println!(
                    "    {}  {}           {}",
                    colors.overlay.apply_to("•"),
                    colors.sapphire.apply_to("notez"),
                    colors.overlay.apply_to("browse your notes folder")
                );
                println!();

                break;
            }

            _ => break,
        }
    }
}
