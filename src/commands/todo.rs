use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use crate::colors::Colors;
use crate::config::Config;
use crate::project;
use crate::tui::{self, theme, VimCommandMode};

// --- Data Model ---

#[derive(Debug, Clone, PartialEq)]
enum CheckState {
    Unchecked,  // [ ]
    Half,       // [/]
    Checked,    // [x]
}

#[derive(Debug, Clone)]
struct TodoItem {
    text: String,
    state: CheckState,
    source: PathBuf,
    project: String,
    is_header: bool,
    is_subtask: bool,
    has_subtasks: bool,
    collapsed: bool,
}

// --- Parsing ---

fn parse_todos_from_content(content: &str) -> Vec<(String, CheckState, bool)> {
    content
        .lines()
        .filter_map(|line| {
            let is_sub = line.starts_with("  - ") || line.starts_with("  -\t");
            let trimmed = line.trim();
            let (text, state) = if trimmed.starts_with("- [ ] ") {
                (trimmed[6..].to_string(), CheckState::Unchecked)
            } else if trimmed.starts_with("- [/] ") {
                (trimmed[6..].to_string(), CheckState::Half)
            } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
                (trimmed[6..].to_string(), CheckState::Checked)
            } else {
                return None;
            };
            Some((text, state, is_sub))
        })
        .collect()
}

fn serialize_todos_for_file(items: &[TodoItem], source: &Path) -> String {
    let mut out = String::from("# TODO\n\n");
    for item in items {
        if item.is_header || item.source != source {
            continue;
        }
        let checkbox = match item.state {
            CheckState::Unchecked => "[ ]",
            CheckState::Half => "[/]",
            CheckState::Checked => "[x]",
        };
        let indent = if item.is_subtask { "  " } else { "" };
        out.push_str(&format!("{}- {} {}\n", indent, checkbox, item.text));
    }
    out
}

fn load_single_todo(path: &Path, project_name: &str) -> Vec<TodoItem> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let parsed = parse_todos_from_content(&content);
    let mut items: Vec<TodoItem> = Vec::new();

    for (text, state, is_sub) in parsed {
        if is_sub {
            // Mark the previous non-subtask item as having subtasks
            if let Some(parent) = items.iter_mut().rev().find(|i| !i.is_subtask && !i.is_header) {
                parent.has_subtasks = true;
            }
        }
        items.push(TodoItem {
            text,
            state,
            source: path.to_path_buf(),
            project: project_name.to_string(),
            is_header: false,
            is_subtask: is_sub,
            has_subtasks: false,
            collapsed: false,
        });
    }

    // Derive parent states from subtasks
    derive_parent_states(&mut items);
    items
}

/// Recalculate parent check states based on their subtasks.
fn derive_parent_states(items: &mut Vec<TodoItem>) {
    let len = items.len();
    for i in 0..len {
        if items[i].is_header || items[i].is_subtask || !items[i].has_subtasks {
            continue;
        }
        // Collect subtask states
        let mut total = 0;
        let mut checked = 0;
        for j in (i + 1)..len {
            if !items[j].is_subtask {
                break;
            }
            total += 1;
            if items[j].state == CheckState::Checked {
                checked += 1;
            }
        }
        if total == 0 {
            continue;
        }
        items[i].state = if checked == 0 {
            CheckState::Unchecked
        } else if checked == total {
            CheckState::Checked
        } else {
            CheckState::Half
        };
    }
}

// --- Loading ---

fn load_local_todos(config: &Config) -> Vec<TodoItem> {
    let root = project::resolve_notez_dir(config, false);
    let path = root.join("TODO.md");
    let cwd = std::env::current_dir().unwrap_or_default();
    let project_name = project::detect_project_name(&cwd);

    let mut result = vec![TodoItem {
        text: project_name.clone(),
        state: CheckState::Unchecked,
        source: path.clone(),
        project: project_name,
        is_header: true,
        is_subtask: false,
        has_subtasks: false,
        collapsed: false,
    }];
    result.extend(load_single_todo(&path, &result[0].project));
    result
}

fn load_global_todos(config: &Config) -> Vec<TodoItem> {
    let root = config.root_path();
    let mut all_items = Vec::new();

    let Ok(entries) = fs::read_dir(&root) else {
        return all_items;
    };

    let mut dirs: Vec<_> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .collect();
    dirs.sort_by_key(|e| e.file_name());

    for entry in dirs {
        let todo_path = entry.path().join("TODO.md");
        if !todo_path.exists() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let items = load_single_todo(&todo_path, &dir_name);
        if items.is_empty() {
            continue;
        }

        all_items.push(TodoItem {
            text: dir_name.clone(),
            state: CheckState::Unchecked,
            source: todo_path.clone(),
            project: dir_name,
            is_header: true,
            is_subtask: false,
            has_subtasks: false,
            collapsed: false,
        });
        all_items.extend(items);
    }

    // Global section at top
    let root_todo = root.join("TODO.md");
    let global_items = load_single_todo(&root_todo, "global");
    let mut result = vec![TodoItem {
        text: "global".to_string(),
        state: CheckState::Unchecked,
        source: root_todo,
        project: "global".to_string(),
        is_header: true,
        is_subtask: false,
        has_subtasks: false,
        collapsed: false,
    }];
    result.extend(global_items);
    result.extend(all_items);

    result
}

// --- Saving ---

fn save_all_todos(items: &[TodoItem]) {
    let mut sources: Vec<PathBuf> = Vec::new();
    for item in items {
        if !item.is_header && !sources.contains(&item.source) {
            sources.push(item.source.clone());
        }
    }
    for source in &sources {
        let content = serialize_todos_for_file(items, source);
        if let Some(parent) = source.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(source, content).ok();
    }
}

// --- Public API ---

pub fn run_todo(global: bool, item: Option<String>) {
    let config = Config::require();

    match item {
        Some(text) => {
            let root = project::resolve_notez_dir(&config, global);
            let path = root.join("TODO.md");
            let project_name = if global {
                "global".to_string()
            } else {
                let cwd = std::env::current_dir().unwrap_or_default();
                project::detect_project_name(&cwd)
            };
            let mut items = load_single_todo(&path, &project_name);
            items.push(TodoItem {
                text,
                state: CheckState::Unchecked,
                source: path.clone(),
                project: project_name,
                is_header: false,
                is_subtask: false,
                has_subtasks: false,
                collapsed: false,
            });
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(&path, serialize_todos_for_file(&items, &path))
                .expect("failed to write TODO.md");

            if !global {
                let home_dir = project::ensure_home_project_dir(&config);
                project::mirror_file_to_home(&path, &home_dir);
            }

            let colors = Colors::new();
            let real_count = items.iter().filter(|i| !i.is_header).count();
            println!(
                "  {} added to TODO ({} items)",
                colors.green.apply_to("✓"),
                real_count
            );
        }
        None => {
            let items = if global {
                load_global_todos(&config)
            } else {
                load_local_todos(&config)
            };
            let updated = run_todo_tui(items, global);
            if global {
                save_all_todos(&updated);
            } else {
                let root = project::resolve_notez_dir(&config, false);
                let path = root.join("TODO.md");
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).ok();
                }
                fs::write(&path, serialize_todos_for_file(&updated, &path))
                    .expect("failed to write TODO.md");

                let home_dir = project::ensure_home_project_dir(&config);
                project::mirror_file_to_home(&path, &home_dir);
            }
        }
    }
}

// --- TUI ---

/// Get indices of visible items (skip subtasks of collapsed parents).
fn get_visible_indices(items: &[TodoItem]) -> Vec<usize> {
    let mut visible = Vec::new();
    let mut skip_subtasks = false;

    for (i, item) in items.iter().enumerate() {
        if item.is_subtask {
            if !skip_subtasks {
                visible.push(i);
            }
        } else {
            skip_subtasks = item.has_subtasks && item.collapsed;
            visible.push(i);
        }
    }
    visible
}

fn run_todo_tui(mut items: Vec<TodoItem>, global: bool) -> Vec<TodoItem> {
    let mut terminal = tui::enter().expect("failed to enter TUI");
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(0));
    }
    let mut vim = VimCommandMode::new();
    let mut input_mode = false;
    let mut subtask_mode = false;
    let mut edit_mode = false;
    let mut edit_idx: usize = 0;
    let mut input_buffer = String::new();
    let mut confirm_delete = false;

    loop {
        // Derive parent states before each render
        derive_parent_states(&mut items);

        terminal
            .draw(|frame| {
                let full = frame.area();
                let area = Rect::new(
                    full.x + 2,
                    full.y + 1,
                    full.width.saturating_sub(4),
                    full.height.saturating_sub(2),
                );

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(area);

                let visible = get_visible_indices(&items);
                let list_items: Vec<ListItem> = visible
                    .iter()
                    .map(|&idx| {
                        let item = &items[idx];
                        if item.is_header {
                            let real_path = fs::canonicalize(&item.source).unwrap_or(item.source.clone());
                            let path_display = real_path.parent()
                                .map(|p| {
                                    let s = p.to_string_lossy().to_string();
                                    let home = dirs::home_dir().unwrap().to_string_lossy().to_string();
                                    s.replacen(&home, "~", 1)
                                })
                                .unwrap_or_default();
                            ListItem::new(Line::from(vec![
                                Span::styled(
                                    format!("  ── {} ", item.text),
                                    Style::default().fg(theme::MAUVE).add_modifier(ratatui::style::Modifier::BOLD),
                                ),
                                Span::styled(path_display, Style::default().fg(theme::OVERLAY)),
                            ]))
                        } else {
                            let indent = if item.is_subtask { "        " } else { "    " };
                            let collapse_icon = if item.has_subtasks {
                                if item.collapsed { "▶ " } else { "▼ " }
                            } else {
                                ""
                            };
                            let (checkbox, style) = match item.state {
                                CheckState::Checked => (
                                    "[x] ",
                                    Style::default().fg(theme::OVERLAY).add_modifier(ratatui::style::Modifier::CROSSED_OUT),
                                ),
                                CheckState::Half => (
                                    "[/] ",
                                    Style::default().fg(theme::YELLOW),
                                ),
                                CheckState::Unchecked => (
                                    "[ ] ",
                                    Style::default().fg(theme::TEXT),
                                ),
                            };
                            let checkbox_color = match item.state {
                                CheckState::Checked => theme::SURFACE,
                                CheckState::Half => theme::YELLOW,
                                CheckState::Unchecked => theme::SAPPHIRE,
                            };
                            ListItem::new(Line::from(vec![
                                Span::styled(indent, Style::default()),
                                Span::styled(collapse_icon, Style::default().fg(theme::SURFACE)),
                                Span::styled(checkbox, Style::default().fg(checkbox_color)),
                                Span::styled(item.text.clone(), style),
                            ]))
                        }
                    })
                    .collect();

                let todo_count = items.iter().filter(|i| !i.is_header && !i.is_subtask && i.state != CheckState::Checked).count();
                let done_count = items.iter().filter(|i| !i.is_header && !i.is_subtask && i.state == CheckState::Checked).count();

                let title_label = if global { "TODO (all projects)" } else { "TODO" };
                let title = Line::from(vec![
                    Span::styled(format!(" {} ", title_label), Style::default().fg(theme::LAVENDER).add_modifier(ratatui::style::Modifier::BOLD)),
                    Span::styled("— ", Style::default().fg(theme::SURFACE)),
                    Span::styled(format!("{} pending", todo_count), Style::default().fg(theme::SAPPHIRE)),
                    Span::styled(" · ", Style::default().fg(theme::SURFACE)),
                    Span::styled(format!("{} done ", done_count), Style::default().fg(theme::GREEN)),
                ]);

                let block = Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .padding(Padding::new(1, 1, 1, 0));

                let list = List::new(list_items)
                    .block(block)
                    .highlight_style(theme::selected())
                    .highlight_symbol("  ▸ ");

                frame.render_stateful_widget(list, chunks[0], &mut state);

                // Status bar
                let bold = ratatui::style::Modifier::BOLD;
                let status = if confirm_delete {
                    Line::from(vec![
                        Span::styled(" delete this todo? ", Style::default().fg(theme::TEXT)),
                        Span::styled("y", Style::default().fg(theme::RED).add_modifier(bold)),
                        Span::styled("es  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("n", Style::default().fg(theme::SAPPHIRE).add_modifier(bold)),
                        Span::styled("o", Style::default().fg(theme::OVERLAY)),
                    ])
                } else if input_mode || subtask_mode || edit_mode {
                    let label = if edit_mode { " edit: " } else if subtask_mode { " subtask: " } else { " new: " };
                    Line::from(vec![
                        Span::styled(label, Style::default().fg(theme::MAUVE)),
                        Span::styled(input_buffer.as_str(), Style::default().fg(theme::TEXT)),
                        Span::styled("▏", Style::default().fg(theme::SAPPHIRE)),
                    ])
                } else if vim.active {
                    Line::from(vec![
                        Span::styled(vim.buffer.as_str(), Style::default().fg(theme::MAUVE)),
                    ])
                } else {
                    let width = chunks[1].width as usize;
                    let left = " xheck  new  subtask  edit  delete";
                    let right = "quit ";
                    let padding = width.saturating_sub(left.len() + right.len());
                    Line::from(vec![
                        Span::styled(" ", Style::default()),
                        Span::styled("x", Style::default().fg(theme::SAPPHIRE).add_modifier(bold)),
                        Span::styled("heck  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("n", Style::default().fg(theme::GREEN).add_modifier(bold)),
                        Span::styled("ew  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("s", Style::default().fg(theme::LAVENDER).add_modifier(bold)),
                        Span::styled("ubtask  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("e", Style::default().fg(theme::MAUVE).add_modifier(bold)),
                        Span::styled("dit  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("d", Style::default().fg(theme::RED).add_modifier(bold)),
                        Span::styled("elete", Style::default().fg(theme::OVERLAY)),
                        Span::styled(" ".repeat(padding), Style::default()),
                        Span::styled("q", Style::default().fg(theme::PEACH).add_modifier(bold)),
                        Span::styled("uit ", Style::default().fg(theme::OVERLAY)),
                    ])
                };
                frame.render_widget(Paragraph::new(status), chunks[1]);
            })
            .expect("failed to draw");

        if let Event::Key(key) = event::read().expect("failed to read event") {
            if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                break;
            }

            // Confirm delete
            if confirm_delete {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Enter => {
                        let vis = get_visible_indices(&items);
                        let vs = state.selected().unwrap_or(0);
                        let ri = vis.get(vs).copied().unwrap_or(0);
                        if ri < items.len() && !items[ri].is_header {
                            if items[ri].has_subtasks {
                                let mut end = ri + 1;
                                while end < items.len() && items[end].is_subtask {
                                    end += 1;
                                }
                                items.drain(ri..end);
                            } else {
                                items.remove(ri);
                            }
                            let new_vis = get_visible_indices(&items);
                            if vs >= new_vis.len() && !new_vis.is_empty() {
                                state.select(Some(new_vis.len() - 1));
                            }
                        }
                        confirm_delete = false;
                    }
                    _ => { confirm_delete = false; }
                }
                continue;
            }

            // Input mode (new todo)
            if input_mode {
                match key.code {
                    KeyCode::Enter => {
                        if !input_buffer.is_empty() {
                            let vis = get_visible_indices(&items);
                            let vs = state.selected().unwrap_or(0);
                            let ri = vis.get(vs).copied().unwrap_or(0);
                            let (source, project) = find_section(&items, ri);
                            let mut insert_at = ri + 1;
                            while insert_at < items.len() && !items[insert_at].is_header {
                                insert_at += 1;
                            }
                            items.insert(insert_at, TodoItem {
                                text: input_buffer.clone(),
                                state: CheckState::Unchecked,
                                source,
                                project,
                                is_header: false,
                                is_subtask: false,
                                has_subtasks: false,
                                collapsed: false,
                            });
                            let new_vis = get_visible_indices(&items);
                            if let Some(pos) = new_vis.iter().position(|&i| i == insert_at) {
                                state.select(Some(pos));
                            }
                        }
                        input_buffer.clear();
                        input_mode = false;
                    }
                    KeyCode::Esc => { input_buffer.clear(); input_mode = false; }
                    KeyCode::Backspace => { input_buffer.pop(); }
                    KeyCode::Char(c) => { input_buffer.push(c); }
                    _ => {}
                }
                continue;
            }

            // Subtask mode
            if subtask_mode {
                match key.code {
                    KeyCode::Enter => {
                        if !input_buffer.is_empty() {
                            let vis = get_visible_indices(&items);
                            let vs = state.selected().unwrap_or(0);
                            let ri = vis.get(vs).copied().unwrap_or(0);
                            let parent_idx = if items[ri].is_subtask {
                                (0..ri).rev().find(|&i| !items[i].is_subtask && !items[i].is_header).unwrap_or(ri)
                            } else if !items[ri].is_header {
                                ri
                            } else {
                                // Can't add subtask to header
                                input_buffer.clear();
                                subtask_mode = false;
                                continue;
                            };

                            items[parent_idx].has_subtasks = true;
                            let source = items[parent_idx].source.clone();
                            let project = items[parent_idx].project.clone();

                            // Insert after the last subtask of this parent
                            let mut insert_at = parent_idx + 1;
                            while insert_at < items.len() && items[insert_at].is_subtask {
                                insert_at += 1;
                            }

                            items.insert(insert_at, TodoItem {
                                text: input_buffer.clone(),
                                state: CheckState::Unchecked,
                                source,
                                project,
                                is_header: false,
                                is_subtask: true,
                                has_subtasks: false,
                                collapsed: false,
                            });
                            // Expand parent if collapsed
                            items[parent_idx].collapsed = false;
                            let new_vis = get_visible_indices(&items);
                            if let Some(pos) = new_vis.iter().position(|&i| i == insert_at) {
                                state.select(Some(pos));
                            }
                        }
                        input_buffer.clear();
                        subtask_mode = false;
                    }
                    KeyCode::Esc => { input_buffer.clear(); subtask_mode = false; }
                    KeyCode::Backspace => { input_buffer.pop(); }
                    KeyCode::Char(c) => { input_buffer.push(c); }
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
                    KeyCode::Esc => { input_buffer.clear(); edit_mode = false; }
                    KeyCode::Backspace => { input_buffer.pop(); }
                    KeyCode::Char(c) => { input_buffer.push(c); }
                    _ => {}
                }
                continue;
            }

            // Vim command mode
            if let Some(cmd) = vim.handle_key(key) {
                if VimCommandMode::is_quit(&cmd) { break; }
                continue;
            }
            if vim.active { continue; }

            let visible = get_visible_indices(&items);
            let vis_sel = state.selected().unwrap_or(0);
            let real_idx = visible.get(vis_sel).copied().unwrap_or(0);

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,

                KeyCode::Char('j') | KeyCode::Down => {
                    if vis_sel + 1 < visible.len() {
                        state.select(Some(vis_sel + 1));
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if vis_sel > 0 {
                        state.select(Some(vis_sel - 1));
                    }
                }

                KeyCode::Char('l') | KeyCode::Right => {
                    if real_idx < items.len() && items[real_idx].has_subtasks && items[real_idx].collapsed {
                        items[real_idx].collapsed = false;
                    }
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    if real_idx < items.len() && items[real_idx].has_subtasks && !items[real_idx].collapsed {
                        items[real_idx].collapsed = true;
                    }
                }

                KeyCode::Char(' ') | KeyCode::Char('x') | KeyCode::Enter => {
                    if real_idx < items.len() && !items[real_idx].is_header {
                        if items[real_idx].has_subtasks {
                            let all_checked = {
                                let mut j = real_idx + 1;
                                let mut all = true;
                                while j < items.len() && items[j].is_subtask {
                                    if items[j].state != CheckState::Checked { all = false; }
                                    j += 1;
                                }
                                all
                            };
                            let new_state = if all_checked { CheckState::Unchecked } else { CheckState::Checked };
                            let mut j = real_idx + 1;
                            while j < items.len() && items[j].is_subtask {
                                items[j].state = new_state.clone();
                                j += 1;
                            }
                        } else {
                            items[real_idx].state = match items[real_idx].state {
                                CheckState::Unchecked => CheckState::Checked,
                                CheckState::Checked => CheckState::Unchecked,
                                CheckState::Half => CheckState::Checked,
                            };
                        }
                    }
                }

                KeyCode::Char('/') => {
                    if real_idx < items.len() && !items[real_idx].is_header && !items[real_idx].has_subtasks {
                        items[real_idx].state = match items[real_idx].state {
                            CheckState::Half => CheckState::Unchecked,
                            _ => CheckState::Half,
                        };
                    }
                }

                KeyCode::Char('n') => {
                    input_mode = true;
                    input_buffer.clear();
                }

                KeyCode::Char('s') => {
                    if real_idx < items.len() && !items[real_idx].is_header {
                        subtask_mode = true;
                        input_buffer.clear();
                    }
                }

                KeyCode::Char('d') => {
                    if real_idx < items.len() && !items[real_idx].is_header {
                        confirm_delete = true;
                    }
                }

                KeyCode::Char('e') => {
                    if real_idx < items.len() && !items[real_idx].is_header {
                        edit_mode = true;
                        edit_idx = real_idx;
                        input_buffer = items[real_idx].text.clone();
                    }
                }

                _ => {}
            }
        }
    }

    tui::leave().expect("failed to leave TUI");
    items
}

fn find_section(items: &[TodoItem], selected: usize) -> (PathBuf, String) {
    for i in (0..=selected).rev() {
        if items[i].is_header {
            return (items[i].source.clone(), items[i].project.clone());
        }
    }
    if let Some(item) = items.first() {
        (item.source.clone(), item.project.clone())
    } else {
        (PathBuf::new(), "local".to_string())
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_states() {
        let content = "# TODO\n\n- [ ] unchecked\n- [/] half done\n- [x] checked\n";
        let items = parse_todos_from_content(content);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].1, CheckState::Unchecked);
        assert_eq!(items[1].1, CheckState::Half);
        assert_eq!(items[2].1, CheckState::Checked);
    }

    #[test]
    fn parse_subtasks() {
        let content = "- [ ] parent\n  - [ ] sub1\n  - [x] sub2\n- [ ] other\n";
        let items = parse_todos_from_content(content);
        assert_eq!(items.len(), 4);
        assert!(!items[0].2); // parent
        assert!(items[1].2);  // subtask
        assert!(items[2].2);  // subtask
        assert!(!items[3].2); // not subtask
    }

    #[test]
    fn derive_parent_from_subtasks() {
        let source = PathBuf::from("/tmp/TODO.md");
        let mut items = vec![
            TodoItem { text: "parent".into(), state: CheckState::Unchecked, source: source.clone(), project: "t".into(), is_header: false, is_subtask: false, has_subtasks: true, collapsed: false },
            TodoItem { text: "sub1".into(), state: CheckState::Checked, source: source.clone(), project: "t".into(), is_header: false, is_subtask: true, has_subtasks: false, collapsed: false },
            TodoItem { text: "sub2".into(), state: CheckState::Unchecked, source: source.clone(), project: "t".into(), is_header: false, is_subtask: true, has_subtasks: false, collapsed: false },
        ];
        derive_parent_states(&mut items);
        assert_eq!(items[0].state, CheckState::Half); // 1 of 2 done
    }

    #[test]
    fn derive_parent_all_done() {
        let source = PathBuf::from("/tmp/TODO.md");
        let mut items = vec![
            TodoItem { text: "parent".into(), state: CheckState::Unchecked, source: source.clone(), project: "t".into(), is_header: false, is_subtask: false, has_subtasks: true, collapsed: false },
            TodoItem { text: "sub1".into(), state: CheckState::Checked, source: source.clone(), project: "t".into(), is_header: false, is_subtask: true, has_subtasks: false, collapsed: false },
            TodoItem { text: "sub2".into(), state: CheckState::Checked, source: source.clone(), project: "t".into(), is_header: false, is_subtask: true, has_subtasks: false, collapsed: false },
        ];
        derive_parent_states(&mut items);
        assert_eq!(items[0].state, CheckState::Checked); // all done
    }

    #[test]
    fn serialize_with_subtasks() {
        let source = PathBuf::from("/tmp/TODO.md");
        let items = vec![
            TodoItem { text: "parent".into(), state: CheckState::Half, source: source.clone(), project: "t".into(), is_header: false, is_subtask: false, has_subtasks: true, collapsed: false },
            TodoItem { text: "sub1".into(), state: CheckState::Checked, source: source.clone(), project: "t".into(), is_header: false, is_subtask: true, has_subtasks: false, collapsed: false },
            TodoItem { text: "sub2".into(), state: CheckState::Unchecked, source: source.clone(), project: "t".into(), is_header: false, is_subtask: true, has_subtasks: false, collapsed: false },
        ];
        let md = serialize_todos_for_file(&items, &source);
        assert!(md.contains("- [/] parent"));
        assert!(md.contains("  - [x] sub1"));
        assert!(md.contains("  - [ ] sub2"));
    }

    #[test]
    fn parse_empty_file() {
        let items = parse_todos_from_content("");
        assert!(items.is_empty());
    }

    #[test]
    fn roundtrip_preserves_items() {
        let source = PathBuf::from("/tmp/test/TODO.md");
        let items = vec![
            TodoItem { text: "buy milk".into(), state: CheckState::Unchecked, source: source.clone(), project: "test".into(), is_header: false, is_subtask: false, has_subtasks: false, collapsed: false },
            TodoItem { text: "fix bug".into(), state: CheckState::Checked, source: source.clone(), project: "test".into(), is_header: false, is_subtask: false, has_subtasks: false, collapsed: false },
        ];
        let md = serialize_todos_for_file(&items, &source);
        let parsed = parse_todos_from_content(&md);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "buy milk");
        assert_eq!(parsed[1].1, CheckState::Checked);
    }
}
