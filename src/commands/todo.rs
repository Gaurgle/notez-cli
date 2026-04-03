use std::fs;
use std::path::{Path, PathBuf};

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
    source: PathBuf,   // which TODO.md this came from
    project: String,   // display name (project or "global")
    is_header: bool,   // section header, not a real item
}

fn parse_todos(content: &str) -> Vec<(String, bool)> {
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("- [ ] ") {
                Some((trimmed[6..].to_string(), false))
            } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
                Some((trimmed[6..].to_string(), true))
            } else {
                None
            }
        })
        .collect()
}

fn serialize_todos_for_file(items: &[TodoItem], source: &Path) -> String {
    let mut out = String::from("# TODO\n\n");
    for item in items {
        if item.is_header || item.source != source {
            continue;
        }
        let checkbox = if item.checked { "[x]" } else { "[ ]" };
        out.push_str(&format!("- {} {}\n", checkbox, item.text));
    }
    out
}

fn load_single_todo(path: &Path, project_name: &str) -> Vec<TodoItem> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    parse_todos(&content)
        .into_iter()
        .map(|(text, checked)| TodoItem {
            text,
            checked,
            source: path.to_path_buf(),
            project: project_name.to_string(),
            is_header: false,
        })
        .collect()
}

fn load_global_todos(config: &Config) -> Vec<TodoItem> {
    let root = config.root_path();
    let mut all_items = Vec::new();

    // Scan all subdirs in ~/notez/ for TODO.md
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

        // Add section header
        all_items.push(TodoItem {
            text: dir_name.clone(),
            checked: false,
            source: todo_path.clone(),
            project: dir_name,
            is_header: true,
        });
        all_items.extend(items);
    }

    // Always include global section at the top
    let root_todo = root.join("TODO.md");
    let global_items = load_single_todo(&root_todo, "global");
    let mut result = vec![TodoItem {
        text: "global".to_string(),
        checked: false,
        source: root_todo,
        project: "global".to_string(),
        is_header: true,
    }];
    result.extend(global_items);
    result.extend(all_items);

    result
}

fn save_all_todos(items: &[TodoItem]) {
    // Group by source file and save each
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

pub fn run_todo(global: bool, item: Option<String>) {
    let config = Config::require();

    match item {
        Some(text) => {
            // Quick-add: always to the specific TODO.md (local or global root)
            let root = project::resolve_notez_dir(&config, global);
            let path = root.join("TODO.md");
            let mut items = load_single_todo(&path, "local");
            items.push(TodoItem {
                text,
                checked: false,
                source: path.clone(),
                project: "local".to_string(),
                is_header: false,
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
                let root = project::resolve_notez_dir(&config, false);
                let path = root.join("TODO.md");
                load_single_todo(&path, "local")
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

fn run_todo_tui(mut items: Vec<TodoItem>, global: bool) -> Vec<TodoItem> {
    let mut terminal = tui::enter().expect("failed to enter TUI");
    let mut state = ListState::default();
    if !items.is_empty() {
        // Select first non-header item
        let first = items.iter().position(|i| !i.is_header).unwrap_or(0);
        state.select(Some(first));
    }
    let mut vim = VimCommandMode::new();
    let mut input_mode = false;
    let mut edit_mode = false;
    let mut edit_idx: usize = 0;
    let mut input_buffer = String::new();
    let mut confirm_delete = false;

    loop {
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

                let list_items: Vec<ListItem> = items
                    .iter()
                    .map(|item| {
                        if item.is_header {
                            let real_path = fs::canonicalize(&item.source).unwrap_or(item.source.clone());
                            let path_display = real_path.parent()
                                .map(|p| {
                                    let s = p.to_string_lossy().to_string();
                                    let home = dirs::home_dir().unwrap().to_string_lossy().to_string();
                                    s.replacen(&home, "~", 1)
                                })
                                .unwrap_or_default();
                            let line = Line::from(vec![
                                Span::styled(
                                    format!("  ── {} ", item.text),
                                    Style::default().fg(theme::MAUVE).add_modifier(ratatui::style::Modifier::BOLD),
                                ),
                                Span::styled(
                                    path_display,
                                    Style::default().fg(theme::OVERLAY),
                                ),
                            ]);
                            ListItem::new(line)
                        } else if item.checked {
                            let line = Line::from(vec![
                                Span::styled("  [x] ", Style::default().fg(theme::SURFACE)),
                                Span::styled(
                                    item.text.clone(),
                                    Style::default()
                                        .fg(theme::OVERLAY)
                                        .add_modifier(ratatui::style::Modifier::CROSSED_OUT),
                                ),
                            ]);
                            ListItem::new(line)
                        } else {
                            let line = Line::from(vec![
                                Span::styled("  [ ] ", Style::default().fg(theme::SAPPHIRE)),
                                Span::styled(item.text.clone(), Style::default().fg(theme::TEXT)),
                            ]);
                            ListItem::new(line)
                        }
                    })
                    .collect();

                let todo_count = items.iter().filter(|i| !i.is_header && !i.checked).count();
                let done_count = items.iter().filter(|i| !i.is_header && i.checked).count();

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
                let status = if confirm_delete {
                    Line::from(vec![
                        Span::styled(" delete this todo? ", Style::default().fg(theme::TEXT)),
                        Span::styled("y", Style::default().fg(theme::PEACH).add_modifier(ratatui::style::Modifier::BOLD)),
                        Span::styled("es  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("n", Style::default().fg(theme::SAPPHIRE).add_modifier(ratatui::style::Modifier::BOLD)),
                        Span::styled("o", Style::default().fg(theme::OVERLAY)),
                    ])
                } else if input_mode || edit_mode {
                    let label = if edit_mode { " edit: " } else { " new: " };
                    Line::from(vec![
                        Span::styled(label, Style::default().fg(theme::MAUVE)),
                        Span::styled(input_buffer.as_str(), Style::default().fg(theme::TEXT)),
                    ])
                } else if vim.active {
                    Line::from(vec![
                        Span::styled(vim.buffer.as_str(), Style::default().fg(theme::MAUVE)),
                    ])
                } else {
                    let bold = ratatui::style::Modifier::BOLD;
                    Line::from(vec![
                        Span::styled(" ", Style::default()),
                        Span::styled("x", Style::default().fg(theme::GREEN).add_modifier(bold)),
                        Span::styled(" check  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("a", Style::default().fg(theme::GREEN).add_modifier(bold)),
                        Span::styled("dd  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("e", Style::default().fg(theme::SAPPHIRE).add_modifier(bold)),
                        Span::styled("dit  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("d", Style::default().fg(theme::PEACH).add_modifier(bold)),
                        Span::styled("elete  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("q", Style::default().fg(theme::PEACH).add_modifier(bold)),
                        Span::styled("uit", Style::default().fg(theme::OVERLAY)),
                    ])
                };
                frame.render_widget(Paragraph::new(status), chunks[1]);
            })
            .expect("failed to draw");

        if let Event::Key(key) = event::read().expect("failed to read event") {
            // Ctrl+C always exits
            if key.code == KeyCode::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                break;
            }

            // Confirm delete mode
            if confirm_delete {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Enter => {
                        let selected = state.selected().unwrap_or(0);
                        if selected < items.len() && !items[selected].is_header {
                            items.remove(selected);
                            if selected >= items.len() && !items.is_empty() {
                                state.select(Some(items.len() - 1));
                            }
                        }
                        confirm_delete = false;
                    }
                    _ => {
                        confirm_delete = false;
                    }
                }
                continue;
            }

            // Input mode (adding new todo)
            if input_mode {
                match key.code {
                    KeyCode::Enter => {
                        if !input_buffer.is_empty() {
                            // Find the section the cursor is in — walk back to nearest header
                            let selected = state.selected().unwrap_or(0);
                            let (source, project) = {
                                let mut src = PathBuf::new();
                                let mut proj = "global".to_string();
                                for i in (0..=selected).rev() {
                                    if items[i].is_header {
                                        src = items[i].source.clone();
                                        proj = items[i].project.clone();
                                        break;
                                    }
                                    src = items[i].source.clone();
                                    proj = items[i].project.clone();
                                }
                                (src, proj)
                            };

                            // Insert after the last item in this section
                            let mut insert_at = selected + 1;
                            while insert_at < items.len() && !items[insert_at].is_header {
                                insert_at += 1;
                            }

                            items.insert(insert_at, TodoItem {
                                text: input_buffer.clone(),
                                checked: false,
                                source,
                                project,
                                is_header: false,
                            });
                            state.select(Some(insert_at));
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
                    if !items.is_empty() {
                        // Skip headers when navigating
                        let mut next = selected + 1;
                        while next < items.len() && items[next].is_header {
                            next += 1;
                        }
                        if next < items.len() {
                            state.select(Some(next));
                        }
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if selected > 0 {
                        let mut prev = selected - 1;
                        while prev > 0 && items[prev].is_header {
                            prev -= 1;
                        }
                        if !items[prev].is_header {
                            state.select(Some(prev));
                        }
                    }
                }

                KeyCode::Char(' ') | KeyCode::Char('x') | KeyCode::Enter => {
                    if selected < items.len() && !items[selected].is_header {
                        items[selected].checked = !items[selected].checked;
                    }
                }

                KeyCode::Char('a') => {
                    input_mode = true;
                    input_buffer.clear();
                }

                KeyCode::Char('d') => {
                    if selected < items.len() && !items[selected].is_header {
                        confirm_delete = true;
                    }
                }

                KeyCode::Char('e') => {
                    if selected < items.len() && !items[selected].is_header {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_todo_items_from_markdown() {
        let content = "# TODO\n\n- [ ] first item\n- [x] done item\n- [ ] third item\n";
        let items = parse_todos(content);
        assert_eq!(items.len(), 3);
        assert!(!items[0].1);
        assert_eq!(items[0].0, "first item");
        assert!(items[1].1);
        assert_eq!(items[1].0, "done item");
    }

    #[test]
    fn parse_empty_file() {
        let items = parse_todos("");
        assert!(items.is_empty());
    }

    #[test]
    fn roundtrip_preserves_items() {
        let source = PathBuf::from("/tmp/test/TODO.md");
        let items = vec![
            TodoItem { text: "buy milk".into(), checked: false, source: source.clone(), project: "test".into(), is_header: false },
            TodoItem { text: "fix bug".into(), checked: true, source: source.clone(), project: "test".into(), is_header: false },
        ];
        let md = serialize_todos_for_file(&items, &source);
        let parsed = parse_todos(&md);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "buy milk");
        assert!(parsed[1].1);
    }
}
