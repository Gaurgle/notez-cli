use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use crate::commands::todo::{
    dim_color, flags_slots, fuzzy_match, match_tag_prefix, mouse_x_to_filter_dot,
    next_char_boundary, parse_filter, parse_filter_sets, prev_char_boundary, toggle_filter_tag,
    FLAG_COLORS, FLAG_DEFS, FLAG_SHAPES,
};
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
    flags: u8,
    scope_icon: &'static str, // lock/globe for global view top-level dirs
}

fn load_tags(root: &Path) -> HashMap<String, u8> {
    let path = root.join(".tags");
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    content
        .lines()
        .filter_map(|line| {
            let (name, flags_str) = line.split_once(':')?;
            let flags = flags_str.trim().parse::<u8>().ok()?;
            Some((name.to_string(), flags))
        })
        .collect()
}

fn save_tags(root: &Path, tags: &HashMap<String, u8>) {
    let mut out = String::new();
    let mut entries: Vec<_> = tags.iter().filter(|(_, &f)| f != 0).collect();
    entries.sort_by_key(|(k, _)| k.clone());
    for (name, flags) in entries {
        out.push_str(&format!("{}:{}\n", name, flags));
    }
    let path = root.join(".tags");
    fs::write(&path, out).ok();
}

pub fn run_tree(global: bool, public: bool) {
    let config = Config::require();

    if global {
        crate::commands::sync::reconcile(&config);
        let root = project::resolve_notez_dir(&config, true, false);
        if !root.exists() {
            let colors = crate::colors::Colors::new();
            println!(
                "\n  {} No notez directory here.\n",
                colors.overlay.apply_to("─")
            );
            return;
        }
        let mut nodes = build_tree_nodes(&root);
        let tags = load_tags(&root);
        for node in nodes.iter_mut() {
            let rel = node
                .path
                .strip_prefix(&root)
                .unwrap_or(&node.path)
                .to_string_lossy()
                .to_string();
            if let Some(&flags) = tags.get(&rel) {
                node.flags = flags;
            }
        }
        // All depth-0 dirs in the global tree live under notez_root, which IS the
        // private store (since the layout inversion of 2026-04-15). Mark them private;
        // the second loop below adds public entries from project mappings as siblings.
        for node in nodes.iter_mut() {
            if node.is_dir && node.depth == 0 {
                node.scope_icon = project::ICON_PRIVATE;
            }
        }

        // Also scan project paths for public notez/ dirs (like todoz -g does)
        let mapping = project::ProjectMapping::load();
        let existing_names: Vec<String> = nodes
            .iter()
            .filter(|n| n.is_dir && n.depth == 0)
            .map(|n| n.name.clone())
            .collect();
        for (name, path) in &mapping.projects {
            let public_dir = PathBuf::from(path).join("notez");
            if !public_dir.exists() {
                continue;
            }
            // Skip if this project already has a public entry (check by resolving existing symlinks)
            let already_public = nodes.iter().any(|n| {
                n.is_dir && n.depth == 0 && {
                    let resolved = fs::canonicalize(&n.path).unwrap_or(n.path.clone());
                    resolved == public_dir || resolved.starts_with(&public_dir)
                }
            });
            if already_public {
                continue;
            }

            let mut public_nodes = build_tree_nodes(&public_dir);
            if public_nodes.is_empty() {
                continue;
            }

            // Create a synthetic project wrapper node
            let wrapper_idx = nodes.len();
            let child_count = numbering::count_files_recursive(&public_dir);
            nodes.push(TreeNode {
                name: name.clone(),
                path: public_dir.clone(),
                is_dir: true,
                depth: 0,
                expanded: false,
                child_count,
                parent_idx: None,
                flags: 0,
                scope_icon: project::ICON_PUBLIC,
            });

            // Reparent all scanned nodes as children of the wrapper
            let offset = nodes.len();
            for node in public_nodes.iter_mut() {
                node.depth += 1;
                if let Some(ref mut p) = node.parent_idx {
                    *p += offset;
                } else {
                    node.parent_idx = Some(wrapper_idx);
                }
            }
            let tags = load_tags(&public_dir);
            for node in public_nodes.iter_mut() {
                let rel = node
                    .path
                    .strip_prefix(&public_dir)
                    .unwrap_or(&node.path)
                    .to_string_lossy()
                    .to_string();
                if let Some(&flags) = tags.get(&rel) {
                    node.flags = flags;
                }
            }
            nodes.extend(public_nodes);
        }

        if nodes.is_empty() {
            let colors = crate::colors::Colors::new();
            println!(
                "\n  {} Empty notez directory.\n",
                colors.overlay.apply_to("─")
            );
            return;
        }

        let path = config.notez_root.replacen(
            &dirs::home_dir().unwrap().to_string_lossy().to_string(),
            "~",
            1,
        );
        let title = "notez (global)".to_string();
        run_tree_tui(nodes, &config.editor, &title, &path, &root);
    } else if public {
        let root = project::local_public_dir();
        if !root.exists() {
            let colors = crate::colors::Colors::new();
            println!(
                "\n  {} No notez directory here.\n",
                colors.overlay.apply_to("─")
            );
            return;
        }
        let mut nodes = build_tree_nodes(&root);
        if nodes.is_empty() {
            let colors = crate::colors::Colors::new();
            println!(
                "\n  {} Empty notez directory.\n",
                colors.overlay.apply_to("─")
            );
            return;
        }
        let tags = load_tags(&root);
        for node in nodes.iter_mut() {
            let rel = node
                .path
                .strip_prefix(&root)
                .unwrap_or(&node.path)
                .to_string_lossy()
                .to_string();
            if let Some(&flags) = tags.get(&rel) {
                node.flags = flags;
            }
        }
        for node in nodes.iter_mut() {
            if node.depth == 0 {
                node.scope_icon = project::ICON_PUBLIC;
            }
        }
        let cwd = std::env::current_dir().unwrap_or_default();
        let name = crate::project::detect_project_name(&cwd);
        let title = format!("{} notez ({})", project::ICON_PUBLIC, name);
        run_tree_tui(nodes, &config.editor, &title, "./notez", &root);
    } else {
        // Local: show both private and public
        let private_root = project::local_private_dir();
        let public_root = project::local_public_dir();
        let mut nodes = Vec::new();

        // Private nodes
        if private_root.exists() {
            let mut private_nodes = build_tree_nodes(&private_root);
            for node in private_nodes.iter_mut() {
                if node.depth == 0 {
                    node.scope_icon = project::ICON_PRIVATE;
                }
            }
            let tags = load_tags(&private_root);
            for node in private_nodes.iter_mut() {
                let rel = node
                    .path
                    .strip_prefix(&private_root)
                    .unwrap_or(&node.path)
                    .to_string_lossy()
                    .to_string();
                if let Some(&flags) = tags.get(&rel) {
                    node.flags = flags;
                }
            }
            nodes.extend(private_nodes);
        }

        // Public nodes — adjust parent_idx offsets
        if public_root.exists() {
            let offset = nodes.len();
            let mut public_nodes = build_tree_nodes(&public_root);
            for node in public_nodes.iter_mut() {
                if node.depth == 0 {
                    node.scope_icon = project::ICON_PUBLIC;
                }
                if let Some(ref mut p) = node.parent_idx {
                    *p += offset;
                }
            }
            let tags = load_tags(&public_root);
            for node in public_nodes.iter_mut() {
                let rel = node
                    .path
                    .strip_prefix(&public_root)
                    .unwrap_or(&node.path)
                    .to_string_lossy()
                    .to_string();
                if let Some(&flags) = tags.get(&rel) {
                    node.flags = flags;
                }
            }
            nodes.extend(public_nodes);
        }

        if nodes.is_empty() {
            let colors = crate::colors::Colors::new();
            println!(
                "\n  {} No notez directory here.\n",
                colors.overlay.apply_to("─")
            );
            return;
        }

        let cwd = std::env::current_dir().unwrap_or_default();
        let name = crate::project::detect_project_name(&cwd);
        let title = format!("notez ({})", name);
        let root = if private_root.exists() {
            private_root
        } else {
            public_root
        };
        run_tree_tui(nodes, &config.editor, &title, ".notez + notez", &root);
    }
}

fn run_tree_tui(
    mut nodes: Vec<TreeNode>,
    editor: &str,
    title: &str,
    title_path: &str,
    root: &Path,
) {
    let mut terminal = tui::enter().expect("failed to enter TUI");
    let mut state = ListState::default();
    state.select(Some(0));
    let mut vim = VimCommandMode::new();
    let mut search_mode = false;
    let mut search_buffer = String::new();
    let mut cursor_pos: usize = 0;
    let mut focus_active = false;
    let mut pre_focus_expanded: Vec<(usize, bool)> = Vec::new();
    let mut show_help = false;
    let mut flag_mode = false;
    let mut preview_scroll: u16 = 0;
    let mut last_preview_idx: usize = usize::MAX; // track selection changes to reset scroll
    let mut filter_strip_area: Rect = Rect::default();
    let mut list_inner_area: Rect = Rect::default();
    let mut visible_for_mouse: Vec<usize> = Vec::new();
    let mut prev_filter: (String, u8) = (String::new(), 0);

    loop {
        derive_dir_flags(&mut nodes);

        // Auto-expand directories that contain filter matches whenever the filter
        // changes — without this, matches inside collapsed dirs stay hidden.
        let cur_filter = parse_filter(&search_buffer);
        let filter_active = !cur_filter.0.is_empty() || cur_filter.1 != 0;
        let filter_changed = cur_filter != prev_filter;
        if filter_active && filter_changed {
            let (text_q, tag_sets) = parse_filter_sets(&search_buffer);
            let keep = compute_node_filter_keep(&nodes, &text_q, &tag_sets);
            for (i, k) in keep.iter().enumerate() {
                if *k && nodes[i].is_dir {
                    nodes[i].expanded = true;
                }
            }
        }
        prev_filter = cur_filter.clone();

        let visible = compute_visible_with_filter(&nodes, &search_buffer);
        let sel = state.selected().unwrap_or(0);
        let real_idx = visible.get(sel).copied().unwrap_or(0);

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

                // Layout: vertical rows (main content + status bar)
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(area);

                // Split main content into tree (left) + preview (right)
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(rows[0]);
                let chunks = [cols[0], rows[1]];
                let inner_width = cols[0].width.saturating_sub(6) as usize;

                let items: Vec<ListItem> = visible
                    .iter()
                    .map(|&idx| {
                        let node = &nodes[idx];
                        let indent = "  ".repeat(node.depth);

                        // Tree connectors for nested items
                        let icon = if node.depth == 0 {
                            if node.is_dir {
                                if node.expanded {
                                    "▼ "
                                } else {
                                    "▶ "
                                }
                            } else {
                                "  "
                            }
                        } else {
                            if node.is_dir {
                                if node.expanded {
                                    "├─▼ "
                                } else {
                                    "├─▶ "
                                }
                            } else {
                                "│   "
                            }
                        };

                        let name = &node.name;
                        let scope = node.scope_icon;

                        let mut spans = Vec::new();
                        spans.extend(flags_slots(node.flags));

                        if node.is_dir && node.child_count > 0 {
                            let count_str = format!("{}", node.child_count);
                            let scope_len = if scope.is_empty() { 0 } else { 2 }; // icon + space
                            let prefix_len = 7 + indent.len() + icon.len() + name.len() + scope_len;
                            let avail =
                                inner_width.saturating_sub(prefix_len + count_str.len() + 2);

                            spans.push(Span::styled(
                                format!("{}{}", indent, icon),
                                Style::default().fg(theme::SURFACE),
                            ));
                            if !scope.is_empty() {
                                spans.push(Span::styled(
                                    format!("{} ", scope),
                                    Style::default().fg(theme::OVERLAY),
                                ));
                            }
                            spans.push(Span::styled(
                                name.to_string(),
                                Style::default().fg(theme::SAPPHIRE),
                            ));
                            if avail > 3 {
                                let dots = "·".repeat(avail);
                                spans.push(Span::styled(
                                    format!(" {} ", dots),
                                    Style::default().fg(theme::SURFACE),
                                ));
                            } else {
                                spans.push(Span::styled(" ", Style::default()));
                            }
                            spans
                                .push(Span::styled(count_str, Style::default().fg(theme::OVERLAY)));
                            ListItem::new(Line::from(spans))
                        } else if node.is_dir {
                            spans.push(Span::styled(
                                format!("{}{}", indent, icon),
                                Style::default().fg(theme::SURFACE),
                            ));
                            if !scope.is_empty() {
                                spans.push(Span::styled(
                                    format!("{} ", scope),
                                    Style::default().fg(theme::OVERLAY),
                                ));
                            }
                            spans.push(Span::styled(
                                name.to_string(),
                                Style::default().fg(theme::SAPPHIRE),
                            ));
                            ListItem::new(Line::from(spans))
                        } else {
                            spans.push(Span::styled(
                                format!("{}{}", indent, icon),
                                Style::default().fg(theme::SURFACE),
                            ));
                            spans.push(Span::styled(
                                name.to_string(),
                                Style::default().fg(theme::TEXT),
                            ));
                            ListItem::new(Line::from(spans))
                        }
                    })
                    .collect();

                let title_spans = vec![
                    Span::styled(
                        format!(" {} ", title),
                        Style::default()
                            .fg(theme::LAVENDER)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    ),
                    Span::styled("— ", Style::default().fg(theme::SURFACE)),
                    Span::styled(
                        format!("{} ", title_path),
                        Style::default().fg(theme::OVERLAY),
                    ),
                ];
                let header = Line::from(title_spans);

                // Build the filter strip for the left pane (mirrors todoz UX).
                let (_, active_tags) = parse_filter(&search_buffer);
                let mut filter_spans: Vec<Span> = Vec::new();
                filter_spans.push(Span::raw("     ")); // 4 highlight indent + 1 flag-leading space
                let bits = [
                    crate::commands::todo::FLAG_IMPORTANT,
                    crate::commands::todo::FLAG_PRIO,
                    crate::commands::todo::FLAG_LONGTERM,
                    crate::commands::todo::FLAG_IDEA,
                    crate::commands::todo::FLAG_BLOCKED,
                ];
                for (i, &bit) in bits.iter().enumerate() {
                    let is_active = active_tags & bit != 0;
                    let style = if is_active {
                        Style::default()
                            .fg(FLAG_COLORS[i])
                            .add_modifier(ratatui::style::Modifier::BOLD)
                    } else {
                        Style::default().fg(dim_color(FLAG_COLORS[i]))
                    };
                    filter_spans.push(Span::styled(FLAG_SHAPES[i].to_string(), style));
                }
                filter_spans.push(Span::raw("  "));
                if search_mode {
                    let (before, after) =
                        search_buffer.split_at(cursor_pos.min(search_buffer.len()));
                    let cursor_char = after
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| " ".to_string());
                    let rest = if after.len() > cursor_char.len() {
                        &after[cursor_char.len()..]
                    } else {
                        ""
                    };
                    filter_spans.push(Span::styled("/", Style::default().fg(theme::YELLOW)));
                    filter_spans.push(Span::styled(
                        before.to_string(),
                        Style::default().fg(theme::TEXT),
                    ));
                    filter_spans.push(Span::styled(
                        cursor_char,
                        Style::default().fg(theme::BASE).bg(theme::SAPPHIRE),
                    ));
                    filter_spans.push(Span::styled(
                        rest.to_string(),
                        Style::default().fg(theme::TEXT),
                    ));
                    if search_buffer.is_empty() {
                        filter_spans.push(Span::styled(
                            "  text + #tag or click a dot",
                            Style::default().fg(Color::Rgb(80, 80, 95)),
                        ));
                    }
                } else if !search_buffer.is_empty() {
                    filter_spans.push(Span::styled("/", Style::default().fg(theme::YELLOW)));
                    for word in search_buffer.split(' ') {
                        if word.is_empty() {
                            continue;
                        }
                        let mut tag_color: Option<Color> = None;
                        if let Some(name) = word.strip_prefix('#') {
                            for (idx, &(_, tag_name, _, _)) in FLAG_DEFS.iter().enumerate() {
                                if tag_name.eq_ignore_ascii_case(name) {
                                    tag_color = Some(FLAG_COLORS[idx]);
                                    break;
                                }
                            }
                        }
                        let style = match tag_color {
                            Some(c) => Style::default()
                                .fg(c)
                                .add_modifier(ratatui::style::Modifier::BOLD),
                            None => Style::default().fg(theme::YELLOW),
                        };
                        filter_spans.push(Span::styled(format!("{} ", word), style));
                    }
                    filter_spans.push(Span::styled(
                        " esc to clear ",
                        Style::default().fg(theme::OVERLAY),
                    ));
                } else {
                    filter_spans.push(Span::styled("/", Style::default().fg(theme::YELLOW)));
                    filter_spans.push(Span::styled("filter", Style::default().fg(theme::OVERLAY)));
                }

                let block = Block::default()
                    .title(header)
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .padding(Padding::new(1, 1, 1, 0));

                // Carve out [filter strip, divider, list] inside the block.
                let inner = block.inner(chunks[0]);
                let inner_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1), // filter strip
                        Constraint::Length(1), // dim divider
                        Constraint::Min(1),    // list
                    ])
                    .split(inner);
                let filter_strip_rect = inner_chunks[0];
                let divider_rect = inner_chunks[1];
                let list_inner_rect = inner_chunks[2];
                filter_strip_area = filter_strip_rect;
                list_inner_area = list_inner_rect;
                visible_for_mouse = visible.clone();

                frame.render_widget(block, chunks[0]);
                frame.render_widget(Paragraph::new(Line::from(filter_spans)), filter_strip_rect);
                let divider_line = Line::from(Span::styled(
                    "─".repeat(divider_rect.width as usize),
                    Style::default().fg(Color::Rgb(50, 50, 65)),
                ));
                frame.render_widget(Paragraph::new(divider_line), divider_rect);

                let list = List::new(items)
                    .highlight_style(theme::selected())
                    .highlight_symbol("  ▸ ");
                frame.render_stateful_widget(list, list_inner_rect, &mut state);

                // Reset preview scroll when selection changes
                if real_idx != last_preview_idx {
                    preview_scroll = 0;
                    last_preview_idx = real_idx;
                }

                // Preview pane
                let preview_lines: Vec<Line> = if real_idx < nodes.len() && !nodes[real_idx].is_dir
                {
                    match fs::read_to_string(&nodes[real_idx].path) {
                        Ok(content) => content
                            .lines()
                            .map(|line| {
                                let trimmed = line.to_string();
                                if trimmed.starts_with('#') {
                                    Line::from(Span::styled(
                                        trimmed,
                                        Style::default()
                                            .fg(theme::MAUVE)
                                            .add_modifier(ratatui::style::Modifier::BOLD),
                                    ))
                                } else if trimmed.starts_with("- [") {
                                    Line::from(Span::styled(
                                        trimmed,
                                        Style::default().fg(theme::SAPPHIRE),
                                    ))
                                } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                                    Line::from(Span::styled(
                                        trimmed,
                                        Style::default().fg(theme::TEXT),
                                    ))
                                } else {
                                    Line::from(Span::styled(
                                        trimmed,
                                        Style::default().fg(theme::SUBTEXT),
                                    ))
                                }
                            })
                            .collect(),
                        Err(_) => vec![Line::from(Span::styled(
                            "  unable to read file",
                            Style::default().fg(theme::OVERLAY),
                        ))],
                    }
                } else if real_idx < nodes.len() && nodes[real_idx].is_dir {
                    match fs::read_dir(&nodes[real_idx].path) {
                        Ok(entries) => {
                            let mut names: Vec<String> = entries
                                .flatten()
                                .map(|e| e.file_name().to_string_lossy().to_string())
                                .collect();
                            names.sort();
                            names
                                .iter()
                                .map(|n| {
                                    let color = if n.ends_with(".md") {
                                        theme::TEXT
                                    } else {
                                        theme::SAPPHIRE
                                    };
                                    Line::from(Span::styled(
                                        format!("  {}", n),
                                        Style::default().fg(color),
                                    ))
                                })
                                .collect()
                        }
                        Err(_) => vec![],
                    }
                } else {
                    vec![]
                };

                let total_lines = preview_lines.len() as u16;
                let preview_height = cols[1].height.saturating_sub(2);
                let max_scroll = total_lines.saturating_sub(preview_height);
                if preview_scroll > max_scroll {
                    preview_scroll = max_scroll;
                }

                // Preview title with tags
                let mut preview_title_spans = vec![];
                if real_idx < nodes.len() {
                    preview_title_spans.extend(flags_slots(nodes[real_idx].flags));
                } else {
                    preview_title_spans.extend(flags_slots(0));
                }
                preview_title_spans.push(Span::styled(
                    if real_idx < nodes.len() {
                        format!("{} ", nodes[real_idx].name)
                    } else {
                        String::new()
                    },
                    Style::default().fg(theme::OVERLAY),
                ));

                let real_path_display = if real_idx < nodes.len() {
                    let resolved = fs::canonicalize(&nodes[real_idx].path)
                        .unwrap_or(nodes[real_idx].path.clone());
                    let home = dirs::home_dir().unwrap().to_string_lossy().to_string();
                    let s = resolved
                        .to_string_lossy()
                        .to_string()
                        .replacen(&home, "~", 1);
                    format!(" {} ", s)
                } else {
                    String::new()
                };

                let preview_block = Block::default()
                    .title(Line::from(preview_title_spans))
                    .title_bottom(Line::from(Span::styled(
                        real_path_display,
                        Style::default().fg(theme::OVERLAY),
                    )))
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .padding(Padding::new(1, 1, 0, 0));

                frame.render_widget(
                    Paragraph::new(preview_lines)
                        .block(preview_block)
                        .scroll((preview_scroll, 0)),
                    cols[1],
                );

                // Status bar
                let status = if vim.active {
                    Line::from(vec![Span::styled(
                        vim.buffer.as_str(),
                        Style::default().fg(theme::MAUVE),
                    )])
                } else if flag_mode {
                    let cur_flags = if real_idx < nodes.len() {
                        nodes[real_idx].flags
                    } else {
                        0
                    };
                    let mut spans =
                        vec![Span::styled(" tags: ", Style::default().fg(theme::MAUVE))];
                    for (idx, &(bit, _, _, label)) in FLAG_DEFS.iter().enumerate() {
                        let active = cur_flags & bit != 0;
                        let color = FLAG_COLORS[idx];
                        // Number: tag color. Colon: gray. Label: tag color when active, gray otherwise.
                        spans.push(Span::styled(
                            format!("{}", idx + 1),
                            Style::default().fg(color),
                        ));
                        spans.push(Span::styled(":", Style::default().fg(theme::OVERLAY)));
                        spans.push(Span::styled(
                            format!("{} ", label),
                            Style::default().fg(if active { color } else { theme::OVERLAY }),
                        ));
                        spans.push(Span::styled(" ", Style::default()));
                    }
                    Line::from(spans)
                } else {
                    let bold = ratatui::style::Modifier::BOLD;
                    let width = area.width as usize;
                    let left = " open  tags  focus  view all";
                    let right_len = 4; // "quit "
                    let padding = width.saturating_sub(left.len() + right_len);
                    Line::from(vec![
                        Span::styled(" ", Style::default()),
                        Span::styled("o", Style::default().fg(theme::GREEN).add_modifier(bold)),
                        Span::styled("pen  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("t", Style::default().fg(theme::PEACH).add_modifier(bold)),
                        Span::styled("ags  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("f", Style::default().fg(theme::GREEN).add_modifier(bold)),
                        Span::styled("ocus  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("v", Style::default().fg(theme::SAPPHIRE).add_modifier(bold)),
                        Span::styled("iew all", Style::default().fg(theme::OVERLAY)),
                        Span::styled(" ".repeat(padding), Style::default()),
                        Span::styled("q", Style::default().fg(theme::PEACH).add_modifier(bold)),
                        Span::styled("uit ", Style::default().fg(theme::OVERLAY)),
                    ])
                };
                frame.render_widget(Paragraph::new(status), chunks[1]);

                // Help overlay
                if show_help {
                    let help_text = vec![
                        Line::from(Span::styled(
                            "  keybindings",
                            Style::default()
                                .fg(theme::MAUVE)
                                .add_modifier(ratatui::style::Modifier::BOLD),
                        )),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("  o", Style::default().fg(theme::GREEN)),
                            Span::styled(
                                " / enter          open file / toggle dir",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  l", Style::default().fg(theme::MAUVE)),
                            Span::styled(
                                "                  expand directory",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  h", Style::default().fg(theme::MAUVE)),
                            Span::styled(
                                "                  collapse / go to parent",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  f", Style::default().fg(theme::GREEN)),
                            Span::styled(
                                "                  focus directory",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  /", Style::default().fg(theme::YELLOW)),
                            Span::styled(
                                "                  filter — fuzzy text + #tagname, or click a dot",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  t", Style::default().fg(theme::PEACH)),
                            Span::styled(
                                "                  tag mode (1-5 toggle, t again to close)",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  v", Style::default().fg(theme::SAPPHIRE)),
                            Span::styled(
                                "                  view all / collapse all",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  j/k", Style::default().fg(theme::TEXT)),
                            Span::styled(
                                "                navigate",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  q", Style::default().fg(theme::PEACH)),
                            Span::styled(
                                "                  quit",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(""),
                        Line::from(Span::styled(
                            "  press any key to close",
                            Style::default().fg(theme::OVERLAY),
                        )),
                    ];
                    let help_h = help_text.len() as u16 + 2;
                    let help_w = 44_u16;
                    let hx = full.x + (full.width.saturating_sub(help_w)) / 2;
                    let hy = full.y + (full.height.saturating_sub(help_h)) / 2;
                    let help_area = Rect::new(hx, hy, help_w, help_h);
                    let help_block = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme::SURFACE))
                        .border_type(ratatui::widgets::BorderType::Rounded)
                        .style(Style::default().bg(theme::BASE));
                    frame.render_widget(ratatui::widgets::Clear, help_area);
                    frame.render_widget(Paragraph::new(help_text).block(help_block), help_area);
                }
            })
            .expect("failed to draw");

        let ev = event::read().expect("failed to read event");

        // Mouse: scroll the preview pane, click filter-strip dots, or click the
        // search field to enter typing mode.
        if let Event::Mouse(mouse) = ev {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    preview_scroll = preview_scroll.saturating_add(3);
                }
                MouseEventKind::ScrollUp => {
                    preview_scroll = preview_scroll.saturating_sub(3);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if mouse.row == filter_strip_area.y {
                        if let Some(d) = mouse_x_to_filter_dot(mouse.column, filter_strip_area.x) {
                            toggle_filter_tag(&mut search_buffer, FLAG_DEFS[d as usize].1);
                            cursor_pos = search_buffer.len();
                            continue;
                        }
                        // Anywhere else on the strip → enter typing mode.
                        search_mode = true;
                        cursor_pos = search_buffer.len();
                        continue;
                    }
                    // Click on a list row: toggle directories, select files,
                    // click a tag dot to toggle that tag (files only).
                    if mouse.row >= list_inner_area.y
                        && mouse.row < list_inner_area.y.saturating_add(list_inner_area.height)
                        && mouse.column >= list_inner_area.x
                        && mouse.column < list_inner_area.x.saturating_add(list_inner_area.width)
                    {
                        let list_row = (mouse.row - list_inner_area.y) as usize;
                        let vis_idx = state.offset() + list_row;
                        if let Some(&real) = visible_for_mouse.get(vis_idx) {
                            if let Some(pos) = visible_for_mouse.iter().position(|&i| i == real) {
                                state.select(Some(pos));
                            }
                            // Tag dot click on a file → toggle that tag.
                            if !nodes[real].is_dir {
                                if let Some(d) =
                                    mouse_x_to_filter_dot(mouse.column, list_inner_area.x)
                                {
                                    nodes[real].flags ^= FLAG_DEFS[d as usize].0;
                                    continue;
                                }
                            }
                            // Otherwise: dirs toggle, files just select.
                            if nodes[real].is_dir {
                                nodes[real].expanded = !nodes[real].expanded;
                            }
                        }
                        continue;
                    }
                }
                _ => {}
            }
            continue;
        }

        if let Event::Key(key) = ev {
            // Ctrl+C always exits
            if key.code == KeyCode::Char('c')
                && key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
            {
                break;
            }

            // Help overlay — any key closes it
            if show_help {
                show_help = false;
                continue;
            }

            // Flag mode
            if flag_mode {
                let mut consumed = true;
                match key.code {
                    KeyCode::Char('1')
                    | KeyCode::Char('2')
                    | KeyCode::Char('3')
                    | KeyCode::Char('4')
                    | KeyCode::Char('5') => {
                        let idx = match key.code {
                            KeyCode::Char(c) => (c as u8 - b'1') as usize,
                            _ => unreachable!(),
                        };
                        if idx < FLAG_DEFS.len() {
                            let visible = compute_visible_with_filter(&nodes, &search_buffer);
                            let vs = state.selected().unwrap_or(0);
                            let ri = visible.get(vs).copied().unwrap_or(0);
                            if ri < nodes.len() && !nodes[ri].is_dir {
                                nodes[ri].flags ^= FLAG_DEFS[idx].0;
                            }
                        }
                    }
                    KeyCode::Char('t') | KeyCode::Esc => {
                        flag_mode = false;
                    }
                    // `/` is a global shortcut: exit flag mode AND pass the key through
                    // so the search/filter handler picks it up.
                    KeyCode::Char('/') => {
                        flag_mode = false;
                        consumed = false;
                    }
                    KeyCode::Char('j')
                    | KeyCode::Down
                    | KeyCode::Char('k')
                    | KeyCode::Up
                    | KeyCode::Char('h')
                    | KeyCode::Left
                    | KeyCode::Char('l')
                    | KeyCode::Right => {
                        consumed = false;
                    }
                    _ => {}
                }
                if consumed {
                    continue;
                }
            }

            // Search mode
            if search_mode {
                match key.code {
                    KeyCode::Enter | KeyCode::Esc => {
                        search_mode = false;
                        cursor_pos = 0;
                        if key.code == KeyCode::Esc {
                            search_buffer.clear();
                        }
                    }
                    KeyCode::Left => {
                        cursor_pos = prev_char_boundary(&search_buffer, cursor_pos);
                    }
                    KeyCode::Right => {
                        cursor_pos = next_char_boundary(&search_buffer, cursor_pos);
                    }
                    KeyCode::Backspace => {
                        if cursor_pos > 0 {
                            let prev = prev_char_boundary(&search_buffer, cursor_pos);
                            search_buffer.remove(prev);
                            cursor_pos = prev;
                        } else {
                            search_buffer.clear();
                            search_mode = false;
                        }
                    }
                    KeyCode::Char(c) => {
                        search_buffer.insert(cursor_pos, c);
                        cursor_pos += c.len_utf8();
                    }
                    _ => {}
                }
                state.select(Some(0));
                continue;
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

            let visible = compute_visible_with_filter(&nodes, &search_buffer);
            let selected = state.selected().unwrap_or(0);
            let real_idx = visible.get(selected).copied().unwrap_or(0);

            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Esc => {
                    if !search_buffer.is_empty() {
                        search_buffer.clear();
                    } else {
                        break;
                    }
                }

                KeyCode::Char('j') | KeyCode::Down => {
                    if selected + 1 < visible.len() {
                        let target = visible[selected + 1];
                        if focus_active {
                            let old_top = find_top_dir(&nodes, real_idx);
                            let new_top = find_top_dir(&nodes, target);
                            if old_top != new_top {
                                if let Some(ot) = old_top {
                                    nodes[ot].expanded = false;
                                }
                                if let Some(nt) = new_top {
                                    nodes[nt].expanded = true;
                                }
                                let new_vis = get_visible_nodes(&nodes);
                                if let Some(pos) = new_vis.iter().position(|&i| i == target) {
                                    state.select(Some(pos));
                                }
                            } else {
                                state.select(Some(selected + 1));
                            }
                        } else {
                            state.select(Some(selected + 1));
                        }
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if selected > 0 {
                        let target = visible[selected - 1];
                        if focus_active {
                            let old_top = find_top_dir(&nodes, real_idx);
                            let new_top = find_top_dir(&nodes, target);
                            if old_top != new_top {
                                if let Some(ot) = old_top {
                                    nodes[ot].expanded = false;
                                }
                                if let Some(nt) = new_top {
                                    nodes[nt].expanded = true;
                                }
                                let new_vis = get_visible_nodes(&nodes);
                                if let Some(pos) = new_vis.iter().position(|&i| i == target) {
                                    state.select(Some(pos));
                                }
                            } else {
                                state.select(Some(selected - 1));
                            }
                        } else {
                            state.select(Some(selected - 1));
                        }
                    }
                }

                KeyCode::Char('l') | KeyCode::Right => {
                    if selected < visible.len() {
                        let idx = visible[selected];
                        if nodes[idx].is_dir && !nodes[idx].expanded {
                            nodes[idx].expanded = true;
                            focus_active = false;
                        }
                    }
                }

                KeyCode::Char('h') | KeyCode::Left => {
                    if selected < visible.len() {
                        let idx = visible[selected];
                        if nodes[idx].is_dir && nodes[idx].expanded {
                            nodes[idx].expanded = false;
                            focus_active = false;
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

                KeyCode::Char('f') => {
                    // Focus: toggle — collapse/expand top-level dirs around current
                    if real_idx < nodes.len() {
                        if focus_active {
                            // Restore
                            let current_top = find_top_dir(&nodes, real_idx);
                            for &(idx, was_expanded) in &pre_focus_expanded {
                                if idx < nodes.len() {
                                    nodes[idx].expanded = was_expanded;
                                }
                            }
                            let new_vis = get_visible_nodes(&nodes);
                            if let Some(pos) =
                                new_vis.iter().position(|&i| i == current_top.unwrap_or(0))
                            {
                                *state.offset_mut() = 0;
                                state.select(Some(pos));
                            }
                            focus_active = false;
                        } else {
                            // Save state
                            pre_focus_expanded = nodes
                                .iter()
                                .enumerate()
                                .filter(|(_, n)| n.is_dir && n.depth == 0)
                                .map(|(i, n)| (i, n.expanded))
                                .collect();
                            // Find top-level dir containing selection
                            let focused_top = find_top_dir(&nodes, real_idx);
                            for (i, node) in nodes.iter_mut().enumerate() {
                                if node.is_dir && node.depth == 0 {
                                    node.expanded = Some(i) == focused_top;
                                }
                            }
                            focus_active = true;
                        }
                    }
                }

                KeyCode::Char('v') => {
                    // Toggle view all / collapse all
                    let current_top = find_top_dir(&nodes, real_idx).unwrap_or(0);
                    let any_collapsed = nodes
                        .iter()
                        .any(|n| n.is_dir && n.depth == 0 && !n.expanded);
                    for node in nodes.iter_mut() {
                        if node.is_dir && node.depth == 0 {
                            node.expanded = any_collapsed;
                        }
                    }
                    let new_vis = get_visible_nodes(&nodes);
                    if let Some(pos) = new_vis.iter().position(|&i| i == current_top) {
                        *state.offset_mut() = 0;
                        state.select(Some(pos));
                    }
                    focus_active = false;
                }

                KeyCode::Char('/') => {
                    search_mode = true;
                    search_buffer.clear();
                    cursor_pos = 0;
                }

                KeyCode::Char('t') => {
                    flag_mode = true;
                }

                // Scroll preview
                KeyCode::Char('J') => {
                    preview_scroll = preview_scroll.saturating_add(1);
                }
                KeyCode::Char('K') => {
                    preview_scroll = preview_scroll.saturating_sub(1);
                }

                KeyCode::Char('?') => {
                    show_help = true;
                }

                _ => {}
            }
        }
    }

    // Save tags on exit — group by root directory
    let mut roots_tags: HashMap<PathBuf, HashMap<String, u8>> = HashMap::new();
    for node in &nodes {
        // Find the root this node belongs to by walking up
        let node_root = if node.depth == 0 {
            node.path.parent().unwrap_or(root).to_path_buf()
        } else {
            let mut cur = node.parent_idx;
            let mut top = node.path.clone();
            while let Some(p) = cur {
                top = nodes[p].path.clone();
                cur = nodes[p].parent_idx;
            }
            top.parent().unwrap_or(root).to_path_buf()
        };
        if node.flags != 0 {
            let rel = node
                .path
                .strip_prefix(&node_root)
                .unwrap_or(&node.path)
                .to_string_lossy()
                .to_string();
            roots_tags
                .entry(node_root)
                .or_default()
                .insert(rel, node.flags);
        }
    }
    for (r, tags) in &roots_tags {
        save_tags(r, tags);
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

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
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
                flags: 0,
                scope_icon: "",
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
                flags: 0,
                scope_icon: "",
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

/// Find the top-level (depth 0) directory containing the given node.
/// Aggregate flags from children onto parent directories.
/// Recomputes directory flags as the OR of their direct children's flags.
/// Must reassign (not OR) so that bits drop when a child loses a tag — the old
/// `|=` left stale dots stuck on parent directories forever.
fn derive_dir_flags(nodes: &mut Vec<TreeNode>) {
    let len = nodes.len();
    for i in (0..len).rev() {
        if !nodes[i].is_dir {
            continue;
        }
        let mut agg: u8 = 0;
        for j in (i + 1)..len {
            if nodes[j].parent_idx == Some(i) {
                agg |= nodes[j].flags;
            }
            if nodes[j].depth <= nodes[i].depth && j > i + 1 {
                break;
            }
        }
        nodes[i].flags = agg;
    }
}

fn node_matches(node: &TreeNode, text_query: &str, tag_sets: &[u8]) -> bool {
    if !text_query.is_empty() && !fuzzy_match(&node.name, text_query) {
        return false;
    }
    for set in tag_sets {
        if node.flags & set == 0 {
            return false;
        }
    }
    true
}

/// Mark every node that matches the filter and walk back up to include all of its
/// ancestors so the filter result still has tree context.
fn compute_node_filter_keep(nodes: &[TreeNode], text_query: &str, tag_sets: &[u8]) -> Vec<bool> {
    let n = nodes.len();
    let mut keep = vec![false; n];
    for i in 0..n {
        if node_matches(&nodes[i], text_query, tag_sets) {
            keep[i] = true;
        }
    }
    for i in 0..n {
        if !keep[i] {
            continue;
        }
        let mut cur = nodes[i].parent_idx;
        while let Some(p) = cur {
            if keep[p] {
                break;
            }
            keep[p] = true;
            cur = nodes[p].parent_idx;
        }
    }
    keep
}

/// Visible-list builder used by both the render closure and the keyboard handler
/// so cursor positions and rendered rows stay in sync when a filter is active.
fn compute_visible_with_filter(nodes: &[TreeNode], search_buffer: &str) -> Vec<usize> {
    if search_buffer.trim().is_empty() {
        return get_visible_nodes(nodes);
    }
    let (text_q, tag_sets) = parse_filter_sets(search_buffer);
    if text_q.is_empty() && tag_sets.is_empty() {
        return get_visible_nodes(nodes);
    }
    let keep = compute_node_filter_keep(nodes, &text_q, &tag_sets);
    let mut v = get_visible_nodes(nodes);
    v.retain(|&i| keep[i]);
    v
}

fn find_top_dir(nodes: &[TreeNode], idx: usize) -> Option<usize> {
    if nodes[idx].depth == 0 {
        return Some(idx);
    }
    let mut cur = nodes[idx].parent_idx;
    while let Some(p) = cur {
        if nodes[p].depth == 0 {
            return Some(p);
        }
        cur = nodes[p].parent_idx;
    }
    None
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
