use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use crate::config::Config;
use crate::numbering;
use crate::project;
use crate::tui::{self, theme, VimCommandMode};

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
                let full = frame.area();

                // Margin from terminal edges
                let area = Rect::new(
                    full.x + 2,
                    full.y + 1,
                    full.width.saturating_sub(4),
                    full.height.saturating_sub(2),
                );
                let inner_width = area.width.saturating_sub(6) as usize; // padding + borders + highlight

                // Layout: main list + status bar
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(area);

                let items: Vec<ListItem> = visible
                    .iter()
                    .map(|&idx| {
                        let node = &nodes[idx];
                        let indent = "  ".repeat(node.depth);

                        // Tree connectors for nested items
                        let icon = if node.depth == 0 {
                            if node.is_dir {
                                if node.expanded { "▼ " } else { "▶ " }
                            } else {
                                "  "
                            }
                        } else {
                            if node.is_dir {
                                if node.expanded { "├─▼ " } else { "├─▶ " }
                            } else {
                                "│   "
                            }
                        };

                        let name = &node.name;

                        if node.is_dir && node.child_count > 0 {
                            let count_str = format!("{} notes", node.child_count);
                            let prefix = format!("{}{}{}", indent, icon, name);
                            let dots_len = inner_width.saturating_sub(prefix.len() + count_str.len() + 1);
                            let dots = "·".repeat(dots_len.max(2));

                            let line = Line::from(vec![
                                Span::styled(format!("{}{}", indent, icon), Style::default().fg(theme::SURFACE)),
                                Span::styled(name.to_string(), Style::default().fg(theme::SAPPHIRE)),
                                Span::styled(format!(" {} ", dots), Style::default().fg(theme::SURFACE)),
                                Span::styled(count_str, Style::default().fg(theme::OVERLAY)),
                            ]);
                            ListItem::new(line)
                        } else if node.is_dir {
                            let line = Line::from(vec![
                                Span::styled(format!("{}{}", indent, icon), Style::default().fg(theme::SURFACE)),
                                Span::styled(name.to_string(), Style::default().fg(theme::SAPPHIRE)),
                            ]);
                            ListItem::new(line)
                        } else {
                            let line = Line::from(vec![
                                Span::styled(format!("{}{}", indent, icon), Style::default().fg(theme::SURFACE)),
                                Span::styled(name.to_string(), Style::default().fg(theme::TEXT)),
                            ]);
                            ListItem::new(line)
                        }
                    })
                    .collect();

                let header = Line::from(vec![
                    Span::styled(" notez ", Style::default().fg(theme::LAVENDER).add_modifier(ratatui::style::Modifier::BOLD)),
                    Span::styled("— ", Style::default().fg(theme::SURFACE)),
                    Span::styled(format!("{} ", title), Style::default().fg(theme::OVERLAY)),
                ]);

                let block = Block::default()
                    .title(header)
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .padding(Padding::new(1, 1, 1, 0));

                let list = List::new(items)
                    .block(block)
                    .highlight_style(theme::selected())
                    .highlight_symbol("  ▸ ");

                frame.render_stateful_widget(list, chunks[0], &mut state);

                // Status bar
                let status = if vim.active {
                    Line::from(vec![
                        Span::styled(vim.buffer.as_str(), Style::default().fg(theme::MAUVE)),
                    ])
                } else {
                    let bold = ratatui::style::Modifier::BOLD;
                    Line::from(vec![
                        Span::styled(" ", Style::default()),
                        Span::styled("n", Style::default().fg(theme::SAPPHIRE).add_modifier(bold)),
                        Span::styled("avigate  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("e", Style::default().fg(theme::MAUVE).add_modifier(bold)),
                        Span::styled("xpand  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("o", Style::default().fg(theme::GREEN).add_modifier(bold)),
                        Span::styled("pen  ", Style::default().fg(theme::OVERLAY)),
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
                            let vis = get_visible_nodes(&nodes);
                            if let Some(pos) = vis.iter().position(|&i| i == parent) {
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
                            terminal = tui::enter().expect("failed to re-enter TUI");
                        }
                    }
                }

                _ => {}
            }
        }
    }

    tui::leave().expect("failed to leave TUI");
}

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

    for dir_node in dirs_numbered.into_iter().chain(dirs_other).chain(files) {
        let idx = nodes.len();
        let dir_path = dir_node.path.clone();
        let is_dir = dir_node.is_dir;
        nodes.push(dir_node);

        if is_dir {
            build_children(&dir_path, depth + 1, Some(idx), nodes);
        }
    }
}

fn get_visible_nodes(nodes: &[TreeNode]) -> Vec<usize> {
    let mut visible = Vec::new();

    for (idx, node) in nodes.iter().enumerate() {
        if node.depth == 0 {
            visible.push(idx);
            continue;
        }

        // Check all ancestors are expanded
        let mut ancestor_expanded = true;
        let mut check = node.parent_idx;
        while let Some(p) = check {
            if !nodes[p].expanded {
                ancestor_expanded = false;
                break;
            }
            check = nodes[p].parent_idx;
        }
        if ancestor_expanded {
            visible.push(idx);
        }
    }

    visible
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn collect_tree_builds_nodes() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("00_quick-notes")).unwrap();
        fs::create_dir(dir.path().join("01_daily-logs")).unwrap();
        fs::create_dir(dir.path().join("random-stuff")).unwrap();
        fs::write(dir.path().join("file.md"), "hello").unwrap();

        let nodes = build_tree_nodes(dir.path());
        let top_level: Vec<_> = nodes.iter().filter(|n| n.depth == 0).collect();
        assert_eq!(top_level.len(), 4);
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
        let visible = get_visible_nodes(&nodes);
        assert!(visible.iter().all(|&i| nodes[i].depth == 0));
    }
}
