# notez v3 — Interactive Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add interactive tree navigator, todo manager, and note editor/opener — all with ratatui TUIs using Catppuccin Mocha styling.

**Architecture:** Shared `src/tui/` module provides terminal setup/teardown, Catppuccin color theme for ratatui, vim command mode (`:wq`/`:qa`), and editor launch helper. Three independent feature commands build on this foundation. Tree and todo use ratatui TUI; edit uses fzf/dialoguer (no TUI needed).

**Tech Stack:** ratatui 0.29, crossterm 0.28 (new). Existing: clap, chrono, console, dialoguer, dirs.

**Project location:** `/Users/at-a/Repos/notez-cli`

---

## File Structure

```
src/
  tui/
    mod.rs             # CREATE: terminal setup/teardown, editor launch, vim command mode
    theme.rs           # CREATE: Catppuccin Mocha ratatui Color/Style constants
  commands/
    tree.rs            # REWRITE: interactive ratatui tree navigator
    todo.rs            # CREATE: todo TUI + quick-add
    edit.rs            # CREATE: edit/open existing notes via fzf
    mod.rs             # MODIFY: add pub mod todo; pub mod edit;
  main.rs              # MODIFY: add Todo, Edit subcommands + dispatch
  Cargo.toml           # MODIFY: add ratatui, crossterm
```

---

### Task 1: Add Dependencies and Shared TUI Module

**Files:**
- Modify: `Cargo.toml`
- Create: `src/tui/mod.rs`
- Create: `src/tui/theme.rs`
- Modify: `src/main.rs` (add `mod tui;`)

- [ ] **Step 1: Add ratatui and crossterm to Cargo.toml**

Add to `[dependencies]`:
```toml
ratatui = "0.29"
crossterm = "0.28"
```

- [ ] **Step 2: Create `src/tui/theme.rs`**

Catppuccin Mocha colors for ratatui (these are the true RGB values, ratatui supports them):

```rust
use ratatui::style::{Color, Modifier, Style};

// Catppuccin Mocha palette — true RGB values
pub const PEACH: Color = Color::Rgb(250, 179, 135);
pub const GREEN: Color = Color::Rgb(166, 227, 161);
pub const YELLOW: Color = Color::Rgb(249, 226, 175);
pub const SAPPHIRE: Color = Color::Rgb(116, 199, 236);
pub const LAVENDER: Color = Color::Rgb(180, 190, 254);
pub const MAUVE: Color = Color::Rgb(203, 166, 247);
pub const OVERLAY: Color = Color::Rgb(127, 132, 156);
pub const SURFACE: Color = Color::Rgb(69, 71, 90);
pub const BASE: Color = Color::Rgb(30, 30, 46);
pub const TEXT: Color = Color::Rgb(205, 214, 244);
pub const SUBTEXT: Color = Color::Rgb(166, 173, 200);

pub fn header() -> Style {
    Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)
}

pub fn selected() -> Style {
    Style::default().fg(SAPPHIRE).bg(SURFACE)
}

pub fn normal() -> Style {
    Style::default().fg(TEXT)
}

pub fn dimmed() -> Style {
    Style::default().fg(OVERLAY)
}

pub fn success() -> Style {
    Style::default().fg(GREEN)
}

pub fn dir_name() -> Style {
    Style::default().fg(SAPPHIRE)
}

pub fn file_name() -> Style {
    Style::default().fg(TEXT)
}

pub fn count() -> Style {
    Style::default().fg(OVERLAY)
}

pub fn border() -> Style {
    Style::default().fg(SURFACE)
}

pub fn checked() -> Style {
    Style::default().fg(OVERLAY).add_modifier(Modifier::CROSSED_OUT)
}

pub fn unchecked() -> Style {
    Style::default().fg(SAPPHIRE)
}

pub fn command_line() -> Style {
    Style::default().fg(MAUVE)
}
```

- [ ] **Step 3: Create `src/tui/mod.rs`**

Terminal setup/teardown, editor launch, and vim command mode:

```rust
pub mod theme;

use std::io;
use std::path::Path;
use std::process::Command;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;

pub type Terminal = ratatui::Terminal<CrosstermBackend<io::Stdout>>;

/// Enter the TUI: raw mode + alternate screen. Returns the Terminal.
pub fn enter() -> io::Result<Terminal> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    ratatui::Terminal::new(backend)
}

/// Leave the TUI: restore terminal state.
pub fn leave() -> io::Result<()> {
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

/// Open a file in $EDITOR, then restore the TUI.
/// Leaves alternate screen, runs editor, re-enters alternate screen.
pub fn open_in_editor(editor: &str, file: &Path) -> io::Result<()> {
    leave()?;
    Command::new(editor)
        .arg(file)
        .status()
        .expect("failed to launch editor");
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    Ok(())
}

/// Tracks vim-style command input (e.g., `:wq`, `:qa`).
pub struct VimCommandMode {
    pub active: bool,
    pub buffer: String,
}

impl VimCommandMode {
    pub fn new() -> Self {
        Self {
            active: false,
            buffer: String::new(),
        }
    }

    /// Process a key event. Returns Some(command) if a command was completed (Enter pressed).
    /// Returns None if still collecting input or not in command mode.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        if !self.active {
            if key.code == KeyCode::Char(':') {
                self.active = true;
                self.buffer.clear();
                self.buffer.push(':');
                return None;
            }
            return None;
        }

        match key.code {
            KeyCode::Enter => {
                let cmd = self.buffer.clone();
                self.active = false;
                self.buffer.clear();
                Some(cmd)
            }
            KeyCode::Esc => {
                self.active = false;
                self.buffer.clear();
                None
            }
            KeyCode::Backspace => {
                self.buffer.pop();
                if self.buffer.is_empty() {
                    self.active = false;
                }
                None
            }
            KeyCode::Char(c) => {
                self.buffer.push(c);
                None
            }
            _ => None,
        }
    }

    /// Check if a completed command means "quit".
    pub fn is_quit(cmd: &str) -> bool {
        matches!(cmd.as_ref(), ":wq" | ":qa" | ":q" | ":q!")
    }
}
```

- [ ] **Step 4: Add `mod tui;` to main.rs**

Add after the other mod declarations:
```rust
mod tui;
```

- [ ] **Step 5: Verify it compiles**

```bash
cd /Users/at-a/Repos/notez-cli && cargo build
```

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/tui/ src/main.rs
git commit -m "feat: add shared TUI module with ratatui theme and vim command mode"
```

---

### Task 2: Interactive Tree

**Files:**
- Rewrite: `src/commands/tree.rs`

- [ ] **Step 1: Write tests for tree data collection**

Keep the existing `collect_tree` and `collect_subdirs` test functions and data structures. Add a new test for the tree node flattening:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn collect_tree_separates_numbered_and_other() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_quick-notes")).unwrap();
        fs::create_dir(dir.path().join("01_daily-logs")).unwrap();
        fs::create_dir(dir.path().join("random-stuff")).unwrap();
        fs::write(dir.path().join("file.md"), "hello").unwrap();

        let nodes = build_tree_nodes(dir.path());
        let dirs: Vec<_> = nodes.iter().filter(|n| n.is_dir).collect();
        assert!(dirs.len() >= 3); // 00, 01, random-stuff
    }

    #[test]
    fn collect_subdirs_recursive() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("02_projects");
        fs::create_dir_all(project.join("backend")).unwrap();
        fs::create_dir_all(project.join("frontend")).unwrap();
        fs::write(project.join("backend").join("notes.md"), "x").unwrap();
        fs::write(project.join("backend").join("todo.md"), "x").unwrap();
        fs::write(project.join("readme.md"), "x").unwrap();

        let nodes = build_tree_nodes(dir.path());
        let project_node = nodes.iter().find(|n| n.name == "02_projects").unwrap();
        assert!(project_node.is_dir);
        assert!(project_node.child_count > 0);
    }

    #[test]
    fn flatten_tree_shows_top_level_initially() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_quick-notes")).unwrap();
        fs::create_dir(dir.path().join("01_daily-logs")).unwrap();

        let nodes = build_tree_nodes(dir.path());
        let visible = get_visible_nodes(&nodes);
        // Top-level dirs should be visible
        assert!(visible.len() >= 2);
    }
}
```

- [ ] **Step 2: Implement tree data model**

The tree needs a flat list of nodes with depth/expansion state for the TUI. Replace the entire `src/commands/tree.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding};

use crate::config::Config;
use crate::numbering;
use crate::project;
use crate::tui::{self, theme, VimCommandMode};

/// A node in the tree (file or directory).
#[derive(Debug, Clone)]
struct TreeNode {
    name: String,
    path: PathBuf,
    is_dir: bool,
    depth: usize,
    expanded: bool,
    child_count: usize,
    parent_idx: Option<usize>,
}

pub fn run_tree(global: bool) {
    let config = Config::require();
    let root = project::resolve_notez_dir(&config, global);

    if !root.exists() {
        let colors = crate::colors::Colors::new();
        println!(
            "\n  {} No notez directory here.\n",
            colors.overlay.apply_to("─")
        );
        return;
    }

    let nodes = build_tree_nodes(&root);
    if nodes.is_empty() {
        let colors = crate::colors::Colors::new();
        println!("\n  {} Empty notez directory.\n", colors.overlay.apply_to("─"));
        return;
    }

    let display_root = if global {
        config.notez_root.replacen(
            &dirs::home_dir().unwrap().to_string_lossy().to_string(),
            "~",
            1,
        )
    } else {
        "./notez".to_string()
    };

    run_tree_tui(nodes, &config.editor, &display_root);
}

fn run_tree_tui(mut nodes: Vec<TreeNode>, editor: &str, title: &str) {
    let mut terminal = tui::enter().expect("failed to enter TUI");
    let mut state = ListState::default();
    state.select(Some(0));
    let mut vim = VimCommandMode::new();

    loop {
        let visible = get_visible_nodes(&nodes);

        terminal
            .draw(|frame| {
                let area = frame.area();

                let items: Vec<ListItem> = visible
                    .iter()
                    .map(|&idx| {
                        let node = &nodes[idx];
                        let indent = "  ".repeat(node.depth);
                        let icon = if node.is_dir {
                            if node.expanded { "▼ " } else { "▶ " }
                        } else {
                            "  "
                        };
                        let count_str = if node.is_dir && node.child_count > 0 {
                            format!("  ({})", node.child_count)
                        } else {
                            String::new()
                        };
                        let line = format!("{}{}{}{}", indent, icon, node.name, count_str);

                        let style = if node.is_dir {
                            theme::dir_name()
                        } else {
                            theme::file_name()
                        };
                        ListItem::new(line).style(style)
                    })
                    .collect();

                let header = format!(" notez — {} ", title);
                let block = Block::default()
                    .title(header)
                    .title_style(theme::header())
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .padding(Padding::new(1, 1, 0, 0));

                let list = List::new(items)
                    .block(block)
                    .highlight_style(theme::selected())
                    .highlight_symbol("▸ ");

                frame.render_stateful_widget(list, area, &mut state);

                // Command line at bottom
                if vim.active {
                    let cmd_area = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
                    let cmd = Line::from(vim.buffer.as_str()).style(theme::command_line());
                    frame.render_widget(cmd, cmd_area);
                }
            })
            .expect("failed to draw");

        if let Event::Key(key) = event::read().expect("failed to read event") {
            // Vim command mode
            if let Some(cmd) = vim.handle_key(key) {
                if VimCommandMode::is_quit(&cmd) {
                    break;
                }
                continue;
            }
            if vim.active {
                continue;
            }

            let visible = get_visible_nodes(&nodes);
            let selected = state.selected().unwrap_or(0);

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,

                KeyCode::Char('j') | KeyCode::Down => {
                    if selected + 1 < visible.len() {
                        state.select(Some(selected + 1));
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if selected > 0 {
                        state.select(Some(selected - 1));
                    }
                }

                KeyCode::Char('l') | KeyCode::Right => {
                    if selected < visible.len() {
                        let idx = visible[selected];
                        if nodes[idx].is_dir && !nodes[idx].expanded {
                            nodes[idx].expanded = true;
                        }
                    }
                }

                KeyCode::Char('h') | KeyCode::Left => {
                    if selected < visible.len() {
                        let idx = visible[selected];
                        if nodes[idx].is_dir && nodes[idx].expanded {
                            nodes[idx].expanded = false;
                        } else if let Some(parent) = nodes[idx].parent_idx {
                            // Jump to parent
                            let visible = get_visible_nodes(&nodes);
                            if let Some(pos) = visible.iter().position(|&i| i == parent) {
                                state.select(Some(pos));
                            }
                        }
                    }
                }

                KeyCode::Enter | KeyCode::Char('o') => {
                    if selected < visible.len() {
                        let idx = visible[selected];
                        if nodes[idx].is_dir {
                            nodes[idx].expanded = !nodes[idx].expanded;
                        } else {
                            let path = nodes[idx].path.clone();
                            tui::open_in_editor(editor, &path).ok();
                            // Need to re-enter TUI after editor
                            terminal = tui::enter().expect("failed to re-enter TUI");
                        }
                    }
                }

                KeyCode::Char('/') => {
                    // TODO: filter mode — future enhancement
                }

                _ => {}
            }
        }
    }

    tui::leave().expect("failed to leave TUI");
}

/// Build a flat list of tree nodes from the notez root directory.
/// Numbered dirs come first (sorted by number), then other dirs, then files.
fn build_tree_nodes(root: &Path) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    build_children(root, 0, None, &mut nodes);
    nodes
}

fn build_children(dir: &Path, depth: usize, parent_idx: Option<usize>, nodes: &mut Vec<TreeNode>) {
    let mut dirs_numbered = Vec::new();
    let mut dirs_other = Vec::new();
    let mut files = Vec::new();

    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            let child_count = numbering::count_files_recursive(&path);
            let is_numbered = name.len() >= 3
                && name[..2].chars().all(|c| c.is_ascii_digit())
                && name.as_bytes().get(2) == Some(&b'_');

            let node = TreeNode {
                name: name.clone(),
                path,
                is_dir: true,
                depth,
                expanded: false,
                child_count,
                parent_idx,
            };

            if is_numbered {
                dirs_numbered.push(node);
            } else {
                dirs_other.push(node);
            }
        } else if name.ends_with(".md") {
            files.push(TreeNode {
                name,
                path,
                is_dir: false,
                depth,
                expanded: false,
                child_count: 0,
                parent_idx,
            });
        }
    }

    dirs_numbered.sort_by_key(|n| n.name.clone());
    dirs_other.sort_by_key(|n| n.name.clone());
    files.sort_by_key(|n| n.name.clone());

    // Add numbered dirs first, then others, then files
    for dir_node in dirs_numbered.into_iter().chain(dirs_other).chain(files) {
        let idx = nodes.len();
        let dir_path = dir_node.path.clone();
        let is_dir = dir_node.is_dir;
        nodes.push(dir_node);

        // Pre-build children for dirs (they start collapsed)
        if is_dir {
            build_children(&dir_path, depth + 1, Some(idx), nodes);
        }
    }
}

/// Get indices of currently visible nodes (respecting expanded/collapsed state).
fn get_visible_nodes(nodes: &[TreeNode]) -> Vec<usize> {
    let mut visible = Vec::new();
    let mut skip_depth: Option<usize> = None;

    for (idx, node) in nodes.iter().enumerate() {
        if let Some(sd) = skip_depth {
            if node.depth > sd {
                continue;
            } else {
                skip_depth = None;
            }
        }

        if node.depth == 0 {
            visible.push(idx);
            if node.is_dir && !node.expanded {
                skip_depth = Some(node.depth);
            }
        } else {
            // Check if all ancestors are expanded
            let mut parent_visible = true;
            let mut check = node.parent_idx;
            while let Some(p) = check {
                if !nodes[p].expanded {
                    parent_visible = false;
                    break;
                }
                check = nodes[p].parent_idx;
            }
            if parent_visible {
                visible.push(idx);
                if node.is_dir && !node.expanded {
                    skip_depth = Some(node.depth);
                }
            }
        }
    }

    visible
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn collect_tree_separates_numbered_and_other() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_quick-notes")).unwrap();
        fs::create_dir(dir.path().join("01_daily-logs")).unwrap();
        fs::create_dir(dir.path().join("random-stuff")).unwrap();
        fs::write(dir.path().join("file.md"), "hello").unwrap();

        let nodes = build_tree_nodes(dir.path());
        let top_level: Vec<_> = nodes.iter().filter(|n| n.depth == 0).collect();
        assert_eq!(top_level.len(), 4); // 00, 01, random-stuff, file.md
    }

    #[test]
    fn numbered_dirs_come_first() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_quick-notes")).unwrap();
        fs::create_dir(dir.path().join("zzz-other")).unwrap();
        fs::create_dir(dir.path().join("01_daily-logs")).unwrap();

        let nodes = build_tree_nodes(dir.path());
        let top_level: Vec<_> = nodes.iter().filter(|n| n.depth == 0).collect();
        assert_eq!(top_level[0].name, "00_quick-notes");
        assert_eq!(top_level[1].name, "01_daily-logs");
        assert_eq!(top_level[2].name, "zzz-other");
    }

    #[test]
    fn children_have_correct_depth() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("00_notes");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("note.md"), "x").unwrap();

        let nodes = build_tree_nodes(dir.path());
        let child = nodes.iter().find(|n| n.name == "note.md").unwrap();
        assert_eq!(child.depth, 1);
    }

    #[test]
    fn visibility_respects_collapsed_state() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("00_notes");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("note.md"), "x").unwrap();

        let nodes = build_tree_nodes(dir.path());
        // All start collapsed
        let visible = get_visible_nodes(&nodes);
        // Only top-level should be visible
        assert!(visible.iter().all(|&i| nodes[i].depth == 0));
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd /Users/at-a/Repos/notez-cli && cargo test --lib commands::tree
```

- [ ] **Step 4: Verify TUI compiles and runs**

```bash
cargo build
cargo run -- tree  # should show interactive TUI (press q to quit)
```

- [ ] **Step 5: Commit**

```bash
git add src/commands/tree.rs
git commit -m "feat: replace static tree with interactive ratatui TUI navigator"
```

---

### Task 3: Todo Command

**Files:**
- Create: `src/commands/todo.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write tests for todo data parsing**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_todo_items_from_markdown() {
        let content = "# TODO\n\n- [ ] first item\n- [x] done item\n- [ ] third item\n";
        let items = parse_todos(content);
        assert_eq!(items.len(), 3);
        assert!(!items[0].checked);
        assert_eq!(items[0].text, "first item");
        assert!(items[1].checked);
        assert_eq!(items[1].text, "done item");
    }

    #[test]
    fn serialize_todo_items_to_markdown() {
        let items = vec![
            TodoItem { text: "first".into(), checked: false },
            TodoItem { text: "done".into(), checked: true },
        ];
        let md = serialize_todos(&items);
        assert!(md.contains("- [ ] first"));
        assert!(md.contains("- [x] done"));
        assert!(md.starts_with("# TODO\n"));
    }

    #[test]
    fn parse_empty_file() {
        let items = parse_todos("");
        assert!(items.is_empty());
    }

    #[test]
    fn roundtrip_preserves_items() {
        let items = vec![
            TodoItem { text: "buy milk".into(), checked: false },
            TodoItem { text: "fix bug".into(), checked: true },
            TodoItem { text: "write docs".into(), checked: false },
        ];
        let md = serialize_todos(&items);
        let parsed = parse_todos(&md);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].text, "buy milk");
        assert!(parsed[1].checked);
    }
}
```

- [ ] **Step 2: Implement todo data model and parsing**

```rust
use std::fs;
use std::path::PathBuf;

use crossterm::event::{self, Event, KeyCode};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use crate::colors::Colors;
use crate::config::Config;
use crate::project;
use crate::tui::{self, theme, VimCommandMode};

#[derive(Debug, Clone)]
struct TodoItem {
    text: String,
    checked: bool,
}

fn parse_todos(content: &str) -> Vec<TodoItem> {
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("- [ ] ") {
                Some(TodoItem {
                    text: trimmed[6..].to_string(),
                    checked: false,
                })
            } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
                Some(TodoItem {
                    text: trimmed[6..].to_string(),
                    checked: true,
                })
            } else {
                None
            }
        })
        .collect()
}

fn serialize_todos(items: &[TodoItem]) -> String {
    let mut out = String::from("# TODO\n\n");
    for item in items {
        let checkbox = if item.checked { "[x]" } else { "[ ]" };
        out.push_str(&format!("- {} {}\n", checkbox, item.text));
    }
    out
}

fn todo_file_path(config: &Config, global: bool) -> PathBuf {
    let root = project::resolve_notez_dir(config, global);
    root.join("TODO.md")
}

fn load_todos(path: &PathBuf) -> Vec<TodoItem> {
    match fs::read_to_string(path) {
        Ok(content) => parse_todos(&content),
        Err(_) => Vec::new(),
    }
}

fn save_todos(path: &PathBuf, items: &[TodoItem]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(path, serialize_todos(items)).expect("failed to write TODO.md");
}
```

- [ ] **Step 3: Implement quick-add command**

```rust
pub fn run_todo(global: bool, item: Option<String>) {
    let config = Config::require();
    let path = todo_file_path(&config, global);

    match item {
        Some(text) => {
            // Quick-add mode
            let mut items = load_todos(&path);
            items.push(TodoItem {
                text,
                checked: false,
            });
            save_todos(&path, &items);

            let colors = Colors::new();
            println!(
                "  {} added to TODO ({}  items)",
                colors.green.apply_to("✓"),
                items.len()
            );
        }
        None => {
            // Interactive TUI mode
            let items = load_todos(&path);
            let updated = run_todo_tui(items, &config.editor);
            save_todos(&path, &updated);
        }
    }
}
```

- [ ] **Step 4: Implement todo TUI**

```rust
fn run_todo_tui(mut items: Vec<TodoItem>, editor: &str) -> Vec<TodoItem> {
    let mut terminal = tui::enter().expect("failed to enter TUI");
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(0));
    }
    let mut vim = VimCommandMode::new();
    let mut input_mode = false;
    let mut input_buffer = String::new();
    let mut edit_mode = false;
    let mut edit_idx: usize = 0;

    loop {
        terminal
            .draw(|frame| {
                let area = frame.area();

                let list_items: Vec<ListItem> = items
                    .iter()
                    .map(|item| {
                        let checkbox = if item.checked { "  [x] " } else { "  [ ] " };
                        let style = if item.checked {
                            theme::checked()
                        } else {
                            theme::unchecked()
                        };
                        ListItem::new(format!("{}{}", checkbox, item.text)).style(style)
                    })
                    .collect();

                let block = Block::default()
                    .title(" TODO ")
                    .title_style(theme::header())
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .padding(Padding::new(1, 1, 0, 0));

                let list = List::new(list_items)
                    .block(block)
                    .highlight_style(theme::selected())
                    .highlight_symbol("▸ ");

                frame.render_stateful_widget(list, area, &mut state);

                // Input line at bottom
                if input_mode || edit_mode {
                    let label = if edit_mode { "edit: " } else { "new: " };
                    let input_area = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
                    let input = Paragraph::new(format!("{}{}", label, input_buffer))
                        .style(theme::command_line());
                    frame.render_widget(input, input_area);
                } else if vim.active {
                    let cmd_area = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
                    let cmd = Paragraph::new(vim.buffer.as_str()).style(theme::command_line());
                    frame.render_widget(cmd, cmd_area);
                }
            })
            .expect("failed to draw");

        if let Event::Key(key) = event::read().expect("failed to read event") {
            // Input mode (adding new todo)
            if input_mode {
                match key.code {
                    KeyCode::Enter => {
                        if !input_buffer.is_empty() {
                            items.push(TodoItem {
                                text: input_buffer.clone(),
                                checked: false,
                            });
                            state.select(Some(items.len() - 1));
                        }
                        input_buffer.clear();
                        input_mode = false;
                    }
                    KeyCode::Esc => {
                        input_buffer.clear();
                        input_mode = false;
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                    }
                    _ => {}
                }
                continue;
            }

            // Edit mode
            if edit_mode {
                match key.code {
                    KeyCode::Enter => {
                        if !input_buffer.is_empty() && edit_idx < items.len() {
                            items[edit_idx].text = input_buffer.clone();
                        }
                        input_buffer.clear();
                        edit_mode = false;
                    }
                    KeyCode::Esc => {
                        input_buffer.clear();
                        edit_mode = false;
                    }
                    KeyCode::Backspace => {
                        input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        input_buffer.push(c);
                    }
                    _ => {}
                }
                continue;
            }

            // Vim command mode
            if let Some(cmd) = vim.handle_key(key) {
                if VimCommandMode::is_quit(&cmd) {
                    break;
                }
                continue;
            }
            if vim.active {
                continue;
            }

            let selected = state.selected().unwrap_or(0);

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,

                KeyCode::Char('j') | KeyCode::Down => {
                    if !items.is_empty() && selected + 1 < items.len() {
                        state.select(Some(selected + 1));
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if selected > 0 {
                        state.select(Some(selected - 1));
                    }
                }

                KeyCode::Char(' ') | KeyCode::Char('x') | KeyCode::Enter => {
                    if selected < items.len() {
                        items[selected].checked = !items[selected].checked;
                    }
                }

                KeyCode::Char('a') => {
                    input_mode = true;
                    input_buffer.clear();
                }

                KeyCode::Char('d') => {
                    if selected < items.len() {
                        items.remove(selected);
                        if selected >= items.len() && !items.is_empty() {
                            state.select(Some(items.len() - 1));
                        }
                    }
                }

                KeyCode::Char('e') => {
                    if selected < items.len() {
                        edit_mode = true;
                        edit_idx = selected;
                        input_buffer = items[selected].text.clone();
                    }
                }

                _ => {}
            }
        }
    }

    tui::leave().expect("failed to leave TUI");
    items
}
```

- [ ] **Step 5: Add `Todo` subcommand to main.rs**

Add to `Commands` enum:
```rust
    /// Manage project todos
    Todo {
        /// Quick-add a todo item
        item: Option<String>,
    },
```

Add to match in `main()`:
```rust
Some(Commands::Todo { item }) => commands::todo::run_todo(cli.global, item),
```

Add to `src/commands/mod.rs`:
```rust
pub mod todo;
```

- [ ] **Step 6: Run tests**

```bash
cargo test --lib commands::todo
```

- [ ] **Step 7: Verify TUI compiles and runs**

```bash
cargo build
cargo run -- todo "test item"
cargo run -- todo  # should show interactive TUI
```

- [ ] **Step 8: Commit**

```bash
git add src/commands/todo.rs src/commands/mod.rs src/main.rs
git commit -m "feat: add interactive todo manager with TUI and quick-add"
```

---

### Task 4: Edit/Open Command

**Files:**
- Create: `src/commands/edit.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write tests for file scanning and fuzzy match**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scan_finds_md_files_recursively() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("00_notes");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("note1.md"), "x").unwrap();
        fs::write(sub.join("note2.md"), "x").unwrap();
        fs::write(dir.path().join("root.md"), "x").unwrap();
        fs::write(dir.path().join("ignore.txt"), "x").unwrap();

        let files = scan_notes(dir.path());
        assert_eq!(files.len(), 3); // only .md files
    }

    #[test]
    fn fuzzy_match_filters_by_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("2026-04-02-api-design.md"), "x").unwrap();
        fs::write(dir.path().join("2026-04-02-bug-fix.md"), "x").unwrap();
        fs::write(dir.path().join("2026-04-01-meeting.md"), "x").unwrap();

        let files = scan_notes(dir.path());
        let matches = fuzzy_filter(&files, "api");
        assert_eq!(matches.len(), 1);
        assert!(matches[0].file_name().unwrap().to_str().unwrap().contains("api"));
    }

    #[test]
    fn fuzzy_match_partial() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("2026-04-02-api-design.md"), "x").unwrap();
        fs::write(dir.path().join("2026-04-02-api-notes.md"), "x").unwrap();
        fs::write(dir.path().join("2026-04-01-meeting.md"), "x").unwrap();

        let files = scan_notes(dir.path());
        let matches = fuzzy_filter(&files, "api");
        assert_eq!(matches.len(), 2);
    }
}
```

- [ ] **Step 2: Implement edit command**

```rust
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::colors::Colors;
use crate::config::Config;
use crate::project;

pub fn run_edit(global: bool, term: Option<String>) {
    let config = Config::require();
    let root = project::resolve_notez_dir(&config, global);

    if !root.exists() {
        let colors = Colors::new();
        eprintln!(
            "  {} No notez directory here.",
            colors.peach.apply_to("✗")
        );
        std::process::exit(1);
    }

    let files = scan_notes(&root);
    if files.is_empty() {
        let colors = Colors::new();
        eprintln!("  {} No notes found.", colors.overlay.apply_to("─"));
        std::process::exit(1);
    }

    let file = match term {
        Some(query) => {
            let matches = fuzzy_filter(&files, &query);
            match matches.len() {
                0 => {
                    eprintln!("No notes matching \"{}\"", query);
                    std::process::exit(1);
                }
                1 => matches[0].clone(),
                _ => pick_file(&config, &matches, &root),
            }
        }
        None => pick_file(&config, &files, &root),
    };

    Command::new(&config.editor)
        .arg(&file)
        .status()
        .expect("failed to launch editor");
}

fn scan_notes(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    scan_notes_recursive(root, &mut files);
    files.sort();
    files
}

fn scan_notes_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_notes_recursive(&path, files);
        } else if path.extension().map_or(false, |e| e == "md") {
            files.push(path);
        }
    }
}

fn fuzzy_filter(files: &[PathBuf], query: &str) -> Vec<PathBuf> {
    let query_lower = query.to_lowercase();
    files
        .iter()
        .filter(|f| {
            f.file_name()
                .unwrap()
                .to_string_lossy()
                .to_lowercase()
                .contains(&query_lower)
        })
        .cloned()
        .collect()
}

fn pick_file(config: &Config, files: &[PathBuf], root: &Path) -> PathBuf {
    if config.has_fzf {
        let input: String = files
            .iter()
            .map(|f| f.strip_prefix(root).unwrap_or(f).to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        let mut child = Command::new("fzf")
            .arg("--prompt")
            .arg("note> ")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("failed to launch fzf");

        {
            use std::io::Write;
            let stdin = child.stdin.as_mut().unwrap();
            stdin.write_all(input.as_bytes()).unwrap();
        }

        let output = child.wait_with_output().expect("fzf failed");
        if !output.status.success() {
            std::process::exit(1);
        }

        let selected = String::from_utf8(output.stdout).unwrap().trim().to_string();
        root.join(selected)
    } else {
        let items: Vec<String> = files
            .iter()
            .map(|f| f.strip_prefix(root).unwrap_or(f).to_string_lossy().to_string())
            .collect();
        let selection = dialoguer::Select::new()
            .with_prompt("Select note")
            .items(&items)
            .default(0)
            .interact()
            .expect("selection failed");
        files[selection].clone()
    }
}
```

- [ ] **Step 3: Add `Edit` subcommand to main.rs**

Add to `Commands` enum:
```rust
    /// Open an existing note
    Edit {
        /// Search term to fuzzy-match note filename
        term: Option<String>,
    },
```

Add to match in `main()`:
```rust
Some(Commands::Edit { term }) => commands::edit::run_edit(cli.global, term),
```

Add to `src/commands/mod.rs`:
```rust
pub mod edit;
```

- [ ] **Step 4: Run tests**

```bash
cargo test --lib commands::edit
```

- [ ] **Step 5: Verify it compiles and works**

```bash
cargo build
cargo run -- edit          # should show fzf picker
cargo run -- edit api      # should fuzzy match
```

- [ ] **Step 6: Commit**

```bash
git add src/commands/edit.rs src/commands/mod.rs src/main.rs
git commit -m "feat: add edit command with fuzzy search and fzf picker"
```

---

### Task 5: Update README, Build, and Install

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update README**

Add new commands to the appropriate sections:

Under **Browsing & Search**:
```
| `notez tree` | Interactive tree navigator (vim keys) |
| `notez edit [term]` | Open an existing note (fuzzy search) |
```

Add new **Todo** section:
```
### Todos

| Command | Description |
|---|---|
| `notez todo` | Interactive todo manager |
| `notez todo "item"` | Quick-add a todo item |
```

- [ ] **Step 2: Run all tests**

```bash
cargo test
```

- [ ] **Step 3: Build release and install**

```bash
cargo build --release
cp target/release/notez ~/.local/bin/notez
```

- [ ] **Step 4: Commit and push**

```bash
git add README.md
git commit -m "docs: update README with tree, todo, and edit commands"
git push origin main
```
