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

pub fn run_todo(global: bool, item: Option<String>) {
    let config = Config::require();
    let path = todo_file_path(&config, global);

    match item {
        Some(text) => {
            let mut items = load_todos(&path);
            items.push(TodoItem {
                text,
                checked: false,
            });
            save_todos(&path, &items);

            let colors = Colors::new();
            println!(
                "  {} added to TODO ({} items)",
                colors.green.apply_to("✓"),
                items.len()
            );
        }
        None => {
            let items = load_todos(&path);
            let updated = run_todo_tui(items);
            save_todos(&path, &updated);
        }
    }
}

fn run_todo_tui(mut items: Vec<TodoItem>) -> Vec<TodoItem> {
    let mut terminal = tui::enter().expect("failed to enter TUI");
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(0));
    }
    let mut vim = VimCommandMode::new();
    let mut input_mode = false;
    let mut edit_mode = false;
    let mut edit_idx: usize = 0;
    let mut input_buffer = String::new();

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
                        if item.checked {
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

                let todo_count = items.iter().filter(|i| !i.checked).count();
                let done_count = items.iter().filter(|i| i.checked).count();

                let title = Line::from(vec![
                    Span::styled(" TODO ", Style::default().fg(theme::LAVENDER).add_modifier(ratatui::style::Modifier::BOLD)),
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
                let status = if input_mode || edit_mode {
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
                        Span::styled("t", Style::default().fg(theme::SAPPHIRE).add_modifier(bold)),
                        Span::styled("oggle  ", Style::default().fg(theme::OVERLAY)),
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

#[cfg(test)]
mod tests {
    use super::*;

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
