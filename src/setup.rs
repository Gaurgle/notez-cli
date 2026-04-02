use std::fs;
use std::path::PathBuf;

use dialoguer::{Confirm, Input, Select};

use crate::colors::Colors;
use crate::config::{Config, detect_tool, expand_tilde};
use crate::numbering;

pub fn run_setup() {
    let colors = Colors::new();
    let existing = Config::load();

    println!();
    println!(
        "  {} {}",
        colors.lavender.apply_to("notez"),
        colors.overlay.apply_to("— setup wizard")
    );
    println!();

    let total = 7;

    // Defaults from existing config or sensible defaults
    let default_root = existing
        .as_ref()
        .map(|c| c.notez_root.clone())
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap()
                .join("notes")
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
                println!(
                    "  {} Welcome! Let's configure your notes.",
                    colors.mauve.apply_to(format!("[{}/{}]", step, total))
                );
                println!();
                println!(
                    "  {} Press Enter to accept defaults, or type a new value.",
                    colors.overlay.apply_to("tip:")
                );
                println!(
                    "  {} Type 'b' to go back a step.\n",
                    colors.overlay.apply_to("    ")
                );
                step = 2;
            }

            // Step 2: Root directory
            2 => {
                println!(
                    "  {} Pick a folder for all your notes.",
                    colors.mauve.apply_to(format!("[{}/{}]", step, total))
                );
                println!(
                    "  {} Enter a full path (e.g. ~/notes, ~/Documents/notes).",
                    colors.overlay.apply_to("   ")
                );
                println!(
                    "  {} This is where notez will create its numbered directories.\n",
                    colors.overlay.apply_to("   ")
                );
                let input: String = Input::new()
                    .with_prompt("  Path")
                    .default(default_root.clone())
                    .interact_text()
                    .unwrap();

                if input.trim() == "b" {
                    step = 1;
                    continue;
                }

                root_dir = expand_tilde(input.trim());
                let root_path = PathBuf::from(&root_dir);

                if !root_path.exists() {
                    let create = Confirm::new()
                        .with_prompt(format!(
                            "  {} does not exist. Create it?",
                            colors.overlay.apply_to(&root_dir)
                        ))
                        .default(true)
                        .interact()
                        .unwrap();

                    if !create {
                        continue;
                    }
                    fs::create_dir_all(&root_path).expect("failed to create root directory");
                }

                step = 3;
            }

            // Step 3: Scan for conflicts
            3 => {
                println!(
                    "  {} Scanning for existing numbered directories...",
                    colors.mauve.apply_to(format!("[{}/{}]", step, total))
                );

                let root_path = PathBuf::from(&root_dir);
                let existing_numbered = numbering::scan_numbered_dirs(&root_path);

                if !existing_numbered.is_empty() {
                    println!(
                        "\n  {} Found existing 00-99 prefixed directories:",
                        colors.yellow.apply_to("!")
                    );
                    for d in &existing_numbered {
                        println!(
                            "    {}",
                            colors.sapphire.apply_to(&d.full_name)
                        );
                    }
                    println!();

                    let choices = vec![
                        "Adopt them into notez numbering (keep as-is)",
                        "Go back and pick a different root directory",
                    ];

                    let choice = Select::new()
                        .with_prompt("  How to handle these?")
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
                        "  {} No conflicts found.\n",
                        colors.green.apply_to("✓")
                    );
                }

                step = 4;
            }

            // Step 4: Quick notes dir name
            4 => {
                println!(
                    "  {} Name for your quick notes folder (will become 00_<name>)",
                    colors.mauve.apply_to(format!("[{}/{}]", step, total))
                );
                let input: String = Input::new()
                    .with_prompt("  Quick notes name")
                    .default(default_quick.clone())
                    .interact_text()
                    .unwrap();

                if input.trim() == "b" {
                    step = 3;
                    continue;
                }

                quick_name = format!("00_{}", numbering::sanitize_name(input.trim()));
                step = 5;
            }

            // Step 5: Daily logs dir name
            5 => {
                println!(
                    "  {} Name for your daily logs folder (will become 01_<name>)",
                    colors.mauve.apply_to(format!("[{}/{}]", step, total))
                );
                let input: String = Input::new()
                    .with_prompt("  Daily logs name")
                    .default(default_daily.clone())
                    .interact_text()
                    .unwrap();

                if input.trim() == "b" {
                    step = 4;
                    continue;
                }

                daily_name = format!("01_{}", numbering::sanitize_name(input.trim()));
                step = 6;
            }

            // Step 6: Detect tools
            6 => {
                println!(
                    "  {} Detecting tools...\n",
                    colors.mauve.apply_to(format!("[{}/{}]", step, total))
                );

                editor = std::env::var("EDITOR").unwrap_or_else(|_| default_editor.clone());
                has_fzf = detect_tool("fzf");
                has_rg = detect_tool("rg");
                has_yazi = detect_tool("yazi");

                let check = |found: bool| -> String {
                    if found {
                        colors.green.apply_to("✓").to_string()
                    } else {
                        format!("{} (using fallback)", colors.yellow.apply_to("✗"))
                    }
                };

                println!("    $EDITOR   {} {}", check(!editor.is_empty()), colors.overlay.apply_to(&editor));
                println!("    fzf       {}", check(has_fzf));
                println!("    rg        {}", check(has_rg));
                println!("    yazi      {}", check(has_yazi));
                println!();

                if editor.is_empty() {
                    let input: String = Input::new()
                        .with_prompt("  Editor command (required)")
                        .default("vim".into())
                        .interact_text()
                        .unwrap();

                    if input.trim() == "b" {
                        step = 5;
                        continue;
                    }
                    editor = input.trim().to_string();
                }

                step = 7;
            }

            // Step 7: Confirm & create
            7 => {
                println!(
                    "  {} Summary:\n",
                    colors.mauve.apply_to(format!("[{}/{}]", step, total))
                );
                println!("    Root:        {}", colors.sapphire.apply_to(&root_dir));
                println!("    Quick notes: {}", colors.sapphire.apply_to(&quick_name));
                println!("    Daily logs:  {}", colors.sapphire.apply_to(&daily_name));
                println!("    Editor:      {}", colors.overlay.apply_to(&editor));
                println!();

                let confirm = Confirm::new()
                    .with_prompt("  Create directories and save config?")
                    .default(true)
                    .interact()
                    .unwrap();

                if !confirm {
                    step = 6;
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

                println!(
                    "\n  {} notez is ready!\n",
                    colors.green.apply_to("✓")
                );

                // Offer shell aliases
                offer_aliases(&colors);

                break;
            }

            _ => break,
        }
    }
}

fn offer_aliases(colors: &Colors) {
    println!(
        "  {} Optional shell aliases for quick access:\n",
        colors.mauve.apply_to("aliases")
    );

    let aliases = vec![
        ("zlog", "notez log", "Quick log entry"),
        ("zlogs", "notez logz", "Open daily logs dir"),
        ("logz", "notez logz", "Open daily logs dir"),
        ("znote", "notez add", "Quick new note"),
    ];

    let mut selected = Vec::new();

    for (alias, target, desc) in &aliases {
        let label = format!(
            "  {} {} → {}",
            colors.sapphire.apply_to(format!("{:<8}", alias)),
            colors.overlay.apply_to("→"),
            colors.overlay.apply_to(format!("{:<14} {}", target, desc))
        );
        println!("{}", label);
        let add = Confirm::new()
            .with_prompt(format!("  Add {}?", alias))
            .default(true)
            .interact()
            .unwrap();
        if add {
            selected.push((*alias, *target));
        }
    }

    if selected.is_empty() {
        println!("\n  {} No aliases selected.\n\n", colors.overlay.apply_to("─"));
        return;
    }

    println!("\n  Add these lines to your shell config:\n");
    for (alias, target) in &selected {
        println!(
            "    {}",
            colors.green.apply_to(format!("alias {}='{}'", alias, target))
        );
    }
    println!();
    println!();
}
