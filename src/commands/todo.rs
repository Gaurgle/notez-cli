use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use crate::colors::Colors;
use crate::config::Config;
use crate::project;
use crate::tui::{self, theme, VimCommandMode};

// --- Data Model ---

#[derive(Debug, Clone, PartialEq)]
enum CheckState {
    Unchecked, // [ ]
    Half,      // [/]
    Checked,   // [x]
}

// Flag bitfield constants
pub const FLAG_IMPORTANT: u8 = 1 << 0;
pub const FLAG_PRIO: u8 = 1 << 1;
pub const FLAG_LONGTERM: u8 = 1 << 2;
pub const FLAG_IDEA: u8 = 1 << 3;
pub const FLAG_BLOCKED: u8 = 1 << 4;

pub const FLAG_COLORS: [Color; 5] = [
    Color::Rgb(243, 139, 168), // red
    Color::Rgb(250, 179, 135), // orange/peach
    Color::Rgb(249, 226, 175), // yellow
    Color::Rgb(116, 199, 236), // blue
    Color::Rgb(203, 166, 247), // purple
];

pub const FLAG_SHAPES: [&str; 5] = ["●", "●", "●", "●", "●"];

pub const FLAG_DEFS: [(u8, &str, &str, &str); 5] = [
    (FLAG_IMPORTANT, "important", "●", "important"),
    (FLAG_PRIO, "prio", "●", "priority"),
    (FLAG_LONGTERM, "longterm", "●", "long-term"),
    (FLAG_IDEA, "idea", "●", "idea"),
    (FLAG_BLOCKED, "blocked", "●", "blocked"),
];

/// Move a byte cursor left by one char in `s`. Always lands on a UTF-8 boundary,
/// so callers can safely pass the result to `split_at` / `insert` / `remove`.
pub fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut i = pos - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Move a byte cursor right by one char in `s`. Always lands on a UTF-8 boundary.
pub fn next_char_boundary(s: &str, pos: usize) -> usize {
    let len = s.len();
    if pos >= len {
        return len;
    }
    let mut i = pos + 1;
    while i < len && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn parse_flags(text: &str) -> (String, u8) {
    let mut flags: u8 = 0;
    let mut clean = text.to_string();
    for &(bit, tag, _, _) in &FLAG_DEFS {
        let marker = format!("#{}", tag);
        if clean.contains(&marker) {
            flags |= bit;
            clean = clean.replace(&marker, "");
        }
    }
    (clean.trim().to_string(), flags)
}

fn serialize_flags(flags: u8) -> String {
    let mut out = String::new();
    for &(bit, tag, _, _) in &FLAG_DEFS {
        if flags & bit != 0 {
            out.push_str(&format!(" #{}", tag));
        }
    }
    out
}

/// Render fixed-position flag slots (5 chars). Active flags show their shape, inactive show "·".
/// When `hover_dot` is `Some(i)`, an unset slot at index `i` previews the tag in its color (dim).
pub fn flags_slots_with_hover(flags: u8, hover_dot: Option<u8>) -> Vec<Span<'static>> {
    let bits = [
        FLAG_IMPORTANT,
        FLAG_PRIO,
        FLAG_LONGTERM,
        FLAG_IDEA,
        FLAG_BLOCKED,
    ];
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ")];
    for (i, &bit) in bits.iter().enumerate() {
        let is_set = flags & bit != 0;
        let is_hovered = hover_dot == Some(i as u8);
        if is_set {
            spans.push(Span::styled(
                FLAG_SHAPES[i].to_string(),
                Style::default().fg(FLAG_COLORS[i]),
            ));
        } else if is_hovered {
            spans.push(Span::styled(
                FLAG_SHAPES[i].to_string(),
                Style::default()
                    .fg(FLAG_COLORS[i])
                    .add_modifier(ratatui::style::Modifier::DIM),
            ));
        } else {
            spans.push(Span::styled(
                "·",
                Style::default().fg(Color::Rgb(50, 50, 65)),
            ));
        }
    }
    spans.push(Span::raw(" "));
    spans
}

pub fn flags_slots(flags: u8) -> Vec<Span<'static>> {
    flags_slots_with_hover(flags, None)
}

/// Resolve a single `#word` token to the set of tags it matches:
/// - `#` (no name) → all tags
/// - `#1`..`#5` (any all-digit string) → the tags at those 1-based indices
///   (so `#13` = tag 1 ∪ tag 3, `#12345` = all). Out-of-range digits are ignored.
/// - `#name` → all tags whose name starts with `name` (case-insensitive prefix)
/// - non-`#` words or unknown prefixes → 0
pub fn match_tag_prefix(word: &str) -> u8 {
    let Some(name) = word.strip_prefix('#') else {
        return 0;
    };
    if name.is_empty() {
        return FLAG_DEFS.iter().fold(0u8, |a, &(b, _, _, _)| a | b);
    }
    if name.chars().all(|c| c.is_ascii_digit()) {
        let mut set: u8 = 0;
        for c in name.chars() {
            if let Some(d) = c.to_digit(10) {
                if (1..=FLAG_DEFS.len() as u32).contains(&d) {
                    set |= FLAG_DEFS[(d - 1) as usize].0;
                }
            }
        }
        return set;
    }
    let name_lower = name.to_lowercase();
    let mut set: u8 = 0;
    for &(bit, tag_name, _, _) in &FLAG_DEFS {
        if tag_name.to_lowercase().starts_with(&name_lower) {
            set |= bit;
        }
    }
    set
}

/// Display-side parse: returns (text query, OR of all matched tag bits).
/// Used to know "which dots should light up" — losing the per-token grouping
/// is fine for display since the dots are a flat indicator.
pub fn parse_filter(buffer: &str) -> (String, u8) {
    let mut text_parts: Vec<&str> = Vec::new();
    let mut tags: u8 = 0;
    for word in buffer.split_whitespace() {
        let m = match_tag_prefix(word);
        if m != 0 {
            tags |= m;
        } else {
            text_parts.push(word);
        }
    }
    (text_parts.join(" "), tags)
}

/// Matching-side parse: returns (text query, list of tag-set requirements).
/// Each entry in `tag_sets` represents one `#token`'s candidate tags — the item
/// must have AT LEAST ONE tag from each set. So `#prio #i` requires `prio` AND
/// (`important` OR `idea`).
pub fn parse_filter_sets(buffer: &str) -> (String, Vec<u8>) {
    let mut text_parts: Vec<&str> = Vec::new();
    let mut tag_sets: Vec<u8> = Vec::new();
    for word in buffer.split_whitespace() {
        let m = match_tag_prefix(word);
        if m != 0 {
            tag_sets.push(m);
        } else {
            text_parts.push(word);
        }
    }
    (text_parts.join(" "), tag_sets)
}

/// Subsequence (fuzzy) match: every char of `query` appears in `text` in order,
/// not necessarily contiguously. Case-insensitive, Unicode-aware.
pub fn fuzzy_match(text: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let text_l = text.to_lowercase();
    let query_l = query.to_lowercase();
    let mut q_iter = query_l.chars().peekable();
    for c in text_l.chars() {
        match q_iter.peek() {
            Some(&qc) if c == qc => {
                q_iter.next();
            }
            Some(_) => {}
            None => return true,
        }
    }
    q_iter.peek().is_none()
}

fn item_matches(item: &TodoItem, text_query: &str, tag_sets: &[u8]) -> bool {
    if !text_query.is_empty() && !fuzzy_match(&item.text, text_query) {
        return false;
    }
    // AND-of-OR: item must have ≥1 tag from each set in `tag_sets`.
    for set in tag_sets {
        if item.flags & set == 0 {
            return false;
        }
    }
    true
}

/// Build the visible-items list, applying the filter if one is active.
/// This must match the render closure exactly so keyboard navigation and
/// selection state stay in sync with what the user sees.
fn compute_visible(items: &[TodoItem], search_buffer: &str) -> Vec<usize> {
    if search_buffer.trim().is_empty() {
        return get_visible_indices(items);
    }
    let (text_q, tag_sets) = parse_filter_sets(search_buffer);
    if text_q.is_empty() && tag_sets.is_empty() {
        return get_visible_indices(items);
    }
    let keep = compute_filter_keep(items, &text_q, &tag_sets);
    let mut v = get_visible_indices(items);
    v.retain(|&i| keep[i]);
    v
}

/// Returns a per-item keep mask. An item is kept if it matches directly OR if any
/// descendant matches — ancestor chains are included so filtered results retain
/// their section / parent context.
fn compute_filter_keep(items: &[TodoItem], text_query: &str, tag_sets: &[u8]) -> Vec<bool> {
    let n = items.len();
    let mut keep = vec![false; n];
    for i in 0..n {
        if item_matches(&items[i], text_query, tag_sets) {
            keep[i] = true;
        }
    }
    // Walk back from each match to mark all ancestors (parent todos + section header).
    for i in 0..n {
        if !keep[i] {
            continue;
        }
        let mut needed_depth = items[i].depth;
        let mut j = i;
        while j > 0 {
            j -= 1;
            if items[j].is_header {
                keep[j] = true;
                break;
            }
            if items[j].depth < needed_depth {
                keep[j] = true;
                needed_depth = items[j].depth;
                if needed_depth == 0 {
                    // Continue walking back to find the enclosing section header.
                    for k in (0..j).rev() {
                        if items[k].is_header {
                            keep[k] = true;
                            break;
                        }
                    }
                    break;
                }
            }
        }
    }
    keep
}

/// Map a screen X coordinate to a dot index (0..=4) if the click landed on the
/// flag-slot row of the item. Returns `None` outside the slot region.
///
/// `list_area_x` is the x of the inner list rect (no borders/padding). Each row
/// starts with the highlight_symbol indent (4 cols), then a flag leading space
/// (1 col), then the 5 dots — so dot 0 sits at `list_area_x + 5`.
fn mouse_x_to_dot(mouse_col: u16, list_area_x: u16) -> Option<u8> {
    let dot_start = list_area_x.saturating_add(5);
    let dot_end = dot_start + 4;
    if mouse_col >= dot_start && mouse_col <= dot_end {
        Some((mouse_col - dot_start) as u8)
    } else {
        None
    }
}

/// Same dot-column mapping but for the filter strip, which has no highlight_symbol
/// indent — the dots sit directly after a 5-col padding (matching the visual offset
/// of dot 0 in list rows: 4 highlight cols + 1 flag-leading space).
pub fn mouse_x_to_filter_dot(mouse_col: u16, strip_x: u16) -> Option<u8> {
    let dot_start = strip_x.saturating_add(5);
    let dot_end = dot_start + 4;
    if mouse_col >= dot_start && mouse_col <= dot_end {
        Some((mouse_col - dot_start) as u8)
    } else {
        None
    }
}

/// Darken an RGB color by ~3x. Used for the inactive ("dim") tag dots in the
/// filter strip — relying on the DIM modifier alone is too subtle in many
/// terminals, so we just compute a darker concrete color instead.
pub fn dim_color(c: Color) -> Color {
    match c {
        Color::Rgb(r, g, b) => Color::Rgb(r / 3, g / 3, b / 3),
        other => other,
    }
}

/// Toggle a `#tagname` token in the filter buffer. Adds it if not present, removes
/// it if it is. Whitespace is normalized so the buffer stays clean.
pub fn toggle_filter_tag(buffer: &mut String, tag_name: &str) {
    let marker = format!("#{}", tag_name);
    let mut found = false;
    let mut new_words: Vec<String> = Vec::new();
    for word in buffer.split_whitespace() {
        if word.eq_ignore_ascii_case(&marker) {
            found = true;
        } else {
            new_words.push(word.to_string());
        }
    }
    if !found {
        new_words.push(marker);
    }
    *buffer = new_words.join(" ");
}

#[derive(Debug, Clone)]
struct TodoItem {
    text: String,
    state: CheckState,
    source: PathBuf,
    project: String,
    is_header: bool,
    depth: u8, // 0 = top-level, 1 = subtask, 2 = sub-subtask
    has_subtasks: bool,
    collapsed: bool,
    is_code_todo: bool,
    flags: u8,
}

/// Nerdfont icon for code TODOs
const ICON_CODE: &str = "\u{f121}"; // </> code icon

// --- Parsing ---

fn parse_todos_from_content(content: &str) -> Vec<(String, CheckState, u8, u8)> {
    content
        .lines()
        .filter_map(|line| {
            let depth = if line.starts_with("    - ") || line.starts_with("    -\t") {
                2
            } else if line.starts_with("  - ") || line.starts_with("  -\t") {
                1
            } else {
                0
            };
            let trimmed = line.trim();
            let (raw_text, state) = if trimmed.starts_with("- [ ] ") {
                (trimmed[6..].to_string(), CheckState::Unchecked)
            } else if trimmed.starts_with("- [/] ") {
                (trimmed[6..].to_string(), CheckState::Half)
            } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
                (trimmed[6..].to_string(), CheckState::Checked)
            } else {
                return None;
            };
            let (text, flags) = parse_flags(&raw_text);
            Some((text, state, depth, flags))
        })
        .collect()
}

fn serialize_todos_for_file(items: &[TodoItem], source: &Path) -> String {
    let mut out = String::from("# TODO\n\n");
    for item in items {
        if item.is_header || item.is_code_todo || item.source != source {
            continue;
        }
        let checkbox = match item.state {
            CheckState::Unchecked => "[ ]",
            CheckState::Half => "[/]",
            CheckState::Checked => "[x]",
        };
        let indent = "  ".repeat(item.depth as usize);
        let flag_tags = serialize_flags(item.flags);
        out.push_str(&format!(
            "{}- {} {}{}\n",
            indent, checkbox, item.text, flag_tags
        ));
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

    for (text, state, depth, flags) in parsed {
        if depth > 0 {
            // Mark the nearest shallower item as having subtasks
            if let Some(parent) = items
                .iter_mut()
                .rev()
                .find(|i| !i.is_header && i.depth < depth)
            {
                parent.has_subtasks = true;
            }
        }
        items.push(TodoItem {
            text,
            state,
            source: path.to_path_buf(),
            project: project_name.to_string(),
            is_header: false,
            depth,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: false,
            flags,
        });
    }

    // Derive parent states from subtasks
    derive_parent_states(&mut items);
    items
}

/// Returns the exclusive end index of the block starting at `idx`: the item plus
/// all consecutive descendants (depth strictly greater). Stops at headers or items
/// at the same/shallower depth. Used by reorder so a parent moves with its children.
fn block_end(items: &[TodoItem], idx: usize) -> usize {
    let depth = items[idx].depth;
    let mut end = idx + 1;
    while end < items.len() && !items[end].is_header && items[end].depth > depth {
        end += 1;
    }
    end
}

/// True iff `start` and `target` are valid endpoints for a drag-to-reorder:
/// same depth, same section (no header between), same parent (no shallower item
/// between). Headers and code-todos can never be dragged.
fn can_drag(items: &[TodoItem], start: usize, target: usize) -> bool {
    if start >= items.len() || target >= items.len() {
        return false;
    }
    if items[start].is_header || items[target].is_header {
        return false;
    }
    if items[start].is_code_todo || items[target].is_code_todo {
        return false;
    }
    if items[start].depth != items[target].depth {
        return false;
    }
    let depth = items[start].depth;
    let (lo, hi) = if start <= target {
        (start, target)
    } else {
        (target, start)
    };
    for i in lo..=hi {
        if items[i].is_header {
            return false;
        }
        if items[i].depth < depth {
            return false;
        }
    }
    true
}

/// Move the block starting at `start` to land at `target`'s position. Assumes
/// `can_drag(items, start, target)` is true. Returns the new start index of the
/// moved block.
fn perform_drag_move(items: &mut [TodoItem], start: usize, target: usize) -> usize {
    let start_end = block_end(items, start);
    let start_len = start_end - start;
    if start < target {
        let target_end = block_end(items, target);
        items[start..target_end].rotate_left(start_len);
        start + (target_end - start_end)
    } else if target < start {
        items[target..start_end].rotate_right(start_len);
        target
    } else {
        start
    }
}

/// Map a screen Y coordinate to the `real_idx` of the item under it, accounting
/// for list scroll offset and per-item rendered row count (wrap-aware).
/// `list_area` is the inner list rect (no borders/padding/filter strip).
fn mouse_y_to_real_idx(
    mouse_row: u16,
    list_area: Rect,
    state_offset: usize,
    visible: &[usize],
    row_counts: &[u16],
) -> Option<usize> {
    if mouse_row < list_area.y || mouse_row >= list_area.y.saturating_add(list_area.height) {
        return None;
    }
    let mut list_row = (mouse_row - list_area.y) as usize;
    for vis_idx in state_offset..visible.len() {
        let rows = row_counts.get(vis_idx).copied().unwrap_or(1) as usize;
        if list_row < rows {
            return Some(visible[vis_idx]);
        }
        list_row -= rows;
    }
    None
}

/// Recalculate `has_subtasks` and parent check states based on direct children.
/// Runs every render so an item that loses all its children is correctly demoted
/// back to a regular leaf (no expand arrow, state untouched).
fn derive_parent_states(items: &mut Vec<TodoItem>) {
    let len = items.len();
    for i in (0..len).rev() {
        if items[i].is_header {
            continue;
        }
        let parent_depth = items[i].depth;
        let mut total = 0;
        let mut checked = 0;
        for j in (i + 1)..len {
            if items[j].depth <= parent_depth {
                break;
            }
            if items[j].depth == parent_depth + 1 {
                total += 1;
                if items[j].state == CheckState::Checked {
                    checked += 1;
                }
            }
        }
        items[i].has_subtasks = total > 0;
        if total > 0 {
            items[i].state = if checked == 0 {
                CheckState::Unchecked
            } else if checked == total {
                CheckState::Checked
            } else {
                CheckState::Half
            };
        }
    }
}

/// Aggregate flags from children onto headers.
fn derive_header_flags(items: &mut Vec<TodoItem>) {
    let len = items.len();
    for i in 0..len {
        if !items[i].is_header {
            continue;
        }
        let mut agg: u8 = 0;
        for j in (i + 1)..len {
            if items[j].is_header {
                break;
            }
            agg |= items[j].flags;
        }
        items[i].flags = agg;
    }
}

// --- Code TODO Scanner ---

fn scan_code_todos() -> Vec<TodoItem> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut results = Vec::new();

    // Use rg if available, fall back to grep
    let output = std::process::Command::new("rg")
        .args([
            "--line-number",
            "--no-heading",
            "--glob",
            "!.notez/**",
            "--glob",
            "!notez/**",
            "--glob",
            "!node_modules/**",
            "--glob",
            "!target/**",
            "--glob",
            "!.git/**",
            "--glob",
            "!*.md",
            r"(?://|#|--|/\*|<!--)\s*TODO\b",
        ])
        .current_dir(&cwd)
        .output();

    let lines = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => {
            // Fallback to grep
            let o = std::process::Command::new("grep")
                .args([
                    "-rn",
                    "--include=*.rs",
                    "--include=*.ts",
                    "--include=*.js",
                    "--include=*.py",
                    "--include=*.go",
                    "--include=*.java",
                    "--include=*.kt",
                    "--include=*.c",
                    "--include=*.cpp",
                    "--include=*.h",
                    "--include=*.sh",
                    "--include=*.toml",
                    "--include=*.yaml",
                    "--include=*.yml",
                    "TODO",
                ])
                .current_dir(&cwd)
                .output();
            match o {
                Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                Err(_) => return results,
            }
        }
    };

    for line in lines.lines() {
        // Format: file:line:content
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() < 3 {
            continue;
        }
        let file = parts[0];
        let line_num = parts[1];
        let content = parts[2].trim();

        // Extract the TODO text after the comment marker
        let todo_text = content
            .trim_start_matches("//")
            .trim_start_matches('#')
            .trim_start_matches("--")
            .trim_start_matches("/*")
            .trim_start_matches("<!--")
            .trim()
            .trim_start_matches("TODO")
            .trim_start_matches(':')
            .trim();

        let short_file = std::path::Path::new(file)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| file.to_string());
        let truncated = if todo_text.len() > 60 {
            format!("{}…", &todo_text[..60].trim_end())
        } else {
            todo_text.to_string()
        };
        let display = if truncated.is_empty() {
            format!("{}:{}", short_file, line_num)
        } else {
            format!("{}:{} {}", short_file, line_num, truncated)
        };

        results.push(TodoItem {
            text: display,
            state: CheckState::Unchecked,
            source: PathBuf::from(file),
            project: "code".to_string(),
            is_header: false,
            depth: 0,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: true,
            flags: 0,
        });
    }

    results
}

// --- Loading ---

fn load_local_todos(_config: &Config, public: bool) -> Vec<TodoItem> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let project_name = project::detect_project_name(&cwd);
    let mut result = Vec::new();

    if public {
        // Only show public
        let path = project::local_public_dir().join("TODO.md");
        result.push(TodoItem {
            text: format!("{} {}", project::ICON_PUBLIC, project_name),
            state: CheckState::Unchecked,
            source: path.clone(),
            project: project_name.clone(),
            is_header: true,
            depth: 0,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: false,
            flags: 0,
        });
        result.extend(load_single_todo(&path, &project_name));
    } else {
        // Show both private and public
        let private_path = project::local_private_dir().join("TODO.md");
        let public_path = project::local_public_dir().join("TODO.md");

        let private_items = load_single_todo(&private_path, &project_name);
        let public_items = load_single_todo(&public_path, &project_name);

        if !private_items.is_empty() {
            result.push(TodoItem {
                text: format!("{} {}", project::ICON_PRIVATE, project_name),
                state: CheckState::Unchecked,
                source: private_path.clone(),
                project: project_name.clone(),
                is_header: true,
                depth: 0,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            });
            result.extend(private_items);
        }

        if !public_items.is_empty() {
            result.push(TodoItem {
                text: format!("{} {}", project::ICON_PUBLIC, project_name),
                state: CheckState::Unchecked,
                source: public_path.clone(),
                project: project_name.clone(),
                is_header: true,
                depth: 0,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            });
            result.extend(public_items);
        }

        // If neither had items, still show a private header so the TUI isn't empty
        if result.is_empty() {
            result.push(TodoItem {
                text: format!("{} {}", project::ICON_PRIVATE, project_name),
                state: CheckState::Unchecked,
                source: private_path,
                project: project_name.clone(),
                is_header: true,
                depth: 0,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            });
        }
    }

    // Scan code TODOs
    let code_todos = scan_code_todos();
    if !code_todos.is_empty() {
        result.push(TodoItem {
            text: format!("{} code TODOs  ({} found)", ICON_CODE, code_todos.len()),
            state: CheckState::Unchecked,
            source: PathBuf::new(),
            project: "code".to_string(),
            is_header: true,
            depth: 0,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: true,
            flags: 0,
        });
        result.extend(code_todos);
    }

    result
}

fn load_global_todos(config: &Config) -> Vec<TodoItem> {
    let root = config.root_path();
    let mut all_items = Vec::new();
    // Track canonical paths to avoid loading the same physical file twice
    // (e.g. when a numbered dir symlinks to a project that also has a mirrored entry)
    let mut seen_canonical: Vec<PathBuf> = Vec::new();

    // Scan home notez dir (private, symlinked)
    let Ok(entries) = fs::read_dir(&root) else {
        return all_items;
    };

    let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
    dirs.sort_by_key(|e| e.file_name());

    for entry in dirs {
        let todo_path = entry.path().join("TODO.md");
        if !todo_path.exists() {
            continue;
        }
        let canonical = todo_path
            .canonicalize()
            .unwrap_or_else(|_| todo_path.clone());
        if seen_canonical.contains(&canonical) {
            continue;
        }
        seen_canonical.push(canonical);
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let items = load_single_todo(&todo_path, &dir_name);
        if items.is_empty() {
            continue;
        }

        all_items.push(TodoItem {
            text: format!("{} {}", project::ICON_PRIVATE, dir_name),
            state: CheckState::Unchecked,
            source: todo_path.clone(),
            project: dir_name,
            is_header: true,
            depth: 0,
            has_subtasks: false,
            collapsed: true,
            is_code_todo: false,
            flags: 0,
        });
        all_items.extend(items);
    }

    // Scan _todos/<category>/TODO.md — a sub-namespace under ~/notez/ for global
    // todo categories that don't share the top-level numbering with notes/projects
    // (e.g. ~/notez/_todos/work/TODO.md, ~/notez/_todos/general/TODO.md).
    // These are kept in a separate vec so we can insert them right after "global"
    // in the final result (rather than after project mirrors).
    let mut todo_categories: Vec<TodoItem> = Vec::new();
    let todos_root = root.join("_todos");
    if let Ok(todo_entries) = fs::read_dir(&todos_root) {
        let mut todo_dirs: Vec<_> = todo_entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .collect();
        todo_dirs.sort_by_key(|e| e.file_name());
        for entry in todo_dirs {
            let todo_path = entry.path().join("TODO.md");
            if !todo_path.exists() {
                continue;
            }
            let canonical = todo_path
                .canonicalize()
                .unwrap_or_else(|_| todo_path.clone());
            if seen_canonical.contains(&canonical) {
                continue;
            }
            seen_canonical.push(canonical);
            let cat_name = entry.file_name().to_string_lossy().to_string();
            let items = load_single_todo(&todo_path, &cat_name);
            // Note: do NOT skip when items.is_empty(); we always show category
            // headers so the user sees the category exists and can add to it.
            todo_categories.push(TodoItem {
                // Uppercase display visually distinguishes the three top "general"
                // categories (GLOBAL, GENERAL, WORK) from project mirrors below.
                text: format!("{} {}", project::ICON_PRIVATE, cat_name.to_uppercase()),
                state: CheckState::Unchecked,
                source: todo_path.clone(),
                project: cat_name,
                is_header: true,
                depth: 0,
                has_subtasks: false,
                collapsed: true,
                is_code_todo: false,
                flags: 0,
            });
            todo_categories.extend(items);
        }
    }

    // Also scan project paths for public notez/TODO.md
    let mapping = project::ProjectMapping::load();
    for (name, path) in &mapping.projects {
        let public_todo = std::path::PathBuf::from(path).join("notez").join("TODO.md");
        if !public_todo.exists() {
            continue;
        }
        let canonical = public_todo
            .canonicalize()
            .unwrap_or_else(|_| public_todo.clone());
        if seen_canonical.contains(&canonical) {
            continue;
        }
        seen_canonical.push(canonical);
        let items = load_single_todo(&public_todo, name);
        if items.is_empty() {
            continue;
        }

        all_items.push(TodoItem {
            text: format!("{} {}", project::ICON_PUBLIC, name),
            state: CheckState::Unchecked,
            source: public_todo,
            project: name.clone(),
            is_header: true,
            depth: 0,
            has_subtasks: false,
            collapsed: true,
            is_code_todo: false,
            flags: 0,
        });
        all_items.extend(items);
    }

    // Global section at top
    let root_todo = root.join("TODO.md");
    let global_items = load_single_todo(&root_todo, "global");
    let mut result = vec![TodoItem {
        text: format!("{} GLOBAL", project::ICON_PRIVATE),
        state: CheckState::Unchecked,
        source: root_todo,
        project: "global".to_string(),
        is_header: true,
        depth: 0,
        has_subtasks: false,
        collapsed: true,
        is_code_todo: false,
        flags: 0,
    }];
    result.extend(global_items);
    result.extend(todo_categories);
    result.extend(all_items);

    result
}

// --- Saving ---

fn save_all_todos(items: &[TodoItem]) {
    let mut sources: Vec<PathBuf> = Vec::new();
    let mut seen_canonical: Vec<PathBuf> = Vec::new();
    for item in items {
        if !item.is_code_todo && !sources.contains(&item.source) {
            // Skip if another source path already resolves to the same physical file
            let canonical = item
                .source
                .canonicalize()
                .unwrap_or_else(|_| item.source.clone());
            if seen_canonical.contains(&canonical) {
                continue;
            }
            seen_canonical.push(canonical);
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

pub fn run_todo(global: bool, public: bool, item: Option<String>) {
    let config = Config::require();

    if global {
        crate::commands::sync::reconcile(&config);
    } else if !public {
        project::ensure_gitignore();
    }

    match item {
        Some(text) => {
            let root = project::resolve_notez_dir(&config, global, public);
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
                depth: 0,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
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
            let scope_icon = if public {
                project::ICON_PUBLIC
            } else {
                project::ICON_PRIVATE
            };
            let real_count = items.iter().filter(|i| !i.is_header).count();
            println!(
                "  {} {} added to TODO ({} items)",
                colors.green.apply_to("✓"),
                colors.overlay.apply_to(scope_icon),
                real_count
            );
        }
        None => {
            let (items, tui_title, tui_path) = if global {
                let path = config.notez_root.replacen(
                    &dirs::home_dir().unwrap().to_string_lossy().to_string(),
                    "~",
                    1,
                );
                (
                    load_global_todos(&config),
                    "todoz (global)".to_string(),
                    path,
                )
            } else {
                let cwd = std::env::current_dir().unwrap_or_default();
                let name = project::detect_project_name(&cwd);
                let icon = if public {
                    project::ICON_PUBLIC
                } else {
                    project::ICON_PRIVATE
                };
                let dir_name = if public { "./notez" } else { "./.notez" };
                (
                    load_local_todos(&config, public),
                    format!("{} todoz ({})", icon, name),
                    dir_name.to_string(),
                )
            };
            let updated = run_todo_tui(items, global, &tui_title, &tui_path, &config);
            if global {
                save_all_todos(&updated);
            } else if public {
                let root = project::resolve_notez_dir(&config, false, true);
                let path = root.join("TODO.md");
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).ok();
                }
                fs::write(&path, serialize_todos_for_file(&updated, &path))
                    .expect("failed to write TODO.md");
                let home_dir = project::ensure_home_project_dir(&config);
                project::mirror_file_to_home(&path, &home_dir);
            } else {
                // Local mode shows both private + public, save each to its own file
                save_all_todos(&updated);

                let private_path = project::local_private_dir().join("TODO.md");
                if private_path.exists() {
                    let home_dir = project::ensure_home_project_dir(&config);
                    project::mirror_file_to_home(&private_path, &home_dir);
                }
            }
        }
    }
}

// --- TUI ---

/// Get indices of visible items (skip items under collapsed headers/parents).
fn get_visible_indices(items: &[TodoItem]) -> Vec<usize> {
    let mut visible = Vec::new();
    let mut skip_section = false;
    let mut collapse_depth: Option<u8> = None; // skip items deeper than this

    for (i, item) in items.iter().enumerate() {
        if item.is_header {
            visible.push(i);
            skip_section = item.collapsed;
            collapse_depth = None;
            continue;
        }

        if skip_section {
            continue;
        }

        if let Some(cd) = collapse_depth {
            if item.depth > cd {
                continue;
            }
            collapse_depth = None;
        }

        visible.push(i);

        if item.has_subtasks && item.collapsed {
            collapse_depth = Some(item.depth);
        }
    }
    visible
}

fn run_todo_tui(
    mut items: Vec<TodoItem>,
    global: bool,
    tui_title: &str,
    tui_path: &str,
    config: &Config,
) -> Vec<TodoItem> {
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
    let mut flag_mode = false;
    let mut search_mode = false;
    let mut search_buffer = String::new();
    let mut cursor_pos: usize = 0;
    let mut confirm_delete = false;
    let mut focus_active = false;
    let mut pre_focus_collapsed: Vec<(usize, bool)> = Vec::new(); // (index, was_collapsed)
    let mut show_help = false;
    let mut category_mode = false;
    let mut category_error: Option<String> = None; // shown in title bar while category prompt is open

    // Mouse-drag reorder state. We separate "click candidate" (set on Down) from
    // "drag active" (set true once a Drag event fires). On Up: if drag became
    // active → perform reorder; if not → it's a click, toggle collapse for
    // headers and subtask-having parents.
    let mut drag_candidate: Option<usize> = None;
    let mut drag_active: bool = false;
    let mut drag_start: Option<usize> = None; // visual highlight during drag
    let mut drag_target: Option<usize> = None; // visual highlight of drop target
    let mut list_area: Rect = Rect::default();
    let mut filter_strip_area: Rect = Rect::default();
    let mut visible_for_mouse: Vec<usize> = Vec::new();
    let mut row_counts_for_mouse: Vec<u16> = Vec::new();
    // Tag dot under the cursor (real_idx, dot_index) — drives hover preview in flag slots.
    let mut hover_flag: Option<(usize, u8)> = None;
    // Tracks the previous filter so we can auto-expand sections/parents whenever the
    // filter changes — without this, filter results are hidden behind collapsed sections
    // and the user has to manually expand each one.
    let mut prev_filter: (String, u8) = (String::new(), 0);

    loop {
        // Derive parent states and header flags before each render
        derive_parent_states(&mut items);
        derive_header_flags(&mut items);

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

                let cur_filter = parse_filter(&search_buffer);
                let filter_active = !cur_filter.0.is_empty() || cur_filter.1 != 0;
                let filter_changed = cur_filter != prev_filter;

                // When the filter changes, auto-expand any section/parent that
                // contains a match. Manual collapses still work afterwards (until the
                // filter changes again). Clearing the filter doesn't auto-collapse —
                // anything the user opened stays open.
                if filter_active && filter_changed {
                    let (text_q, tag_sets) = parse_filter_sets(&search_buffer);
                    let keep = compute_filter_keep(&items, &text_q, &tag_sets);
                    for (i, k) in keep.iter().enumerate() {
                        if *k && (items[i].is_header || items[i].has_subtasks) {
                            items[i].collapsed = false;
                        }
                    }
                }
                prev_filter = cur_filter.clone();

                let visible: Vec<usize> = compute_visible(&items, &search_buffer);
                // Capture for the mouse handler — it needs to know exactly what's
                // visible (post-filter) and where on screen the list starts.
                list_area = chunks[0];
                visible_for_mouse = visible.clone();
                let mut list_items: Vec<ListItem> = visible
                    .iter()
                    .map(|&idx| {
                        let item = &items[idx];
                        if item.is_header {
                            let real_path =
                                fs::canonicalize(&item.source).unwrap_or(item.source.clone());
                            let path_display = real_path
                                .parent()
                                .map(|p| {
                                    let s = p.to_string_lossy().to_string();
                                    let home =
                                        dirs::home_dir().unwrap().to_string_lossy().to_string();
                                    s.replacen(&home, "~", 1)
                                })
                                .unwrap_or_default();
                            let header_color = if item.is_code_todo {
                                theme::YELLOW
                            } else {
                                theme::MAUVE
                            };
                            let collapse_icon = if item.collapsed { "▶ " } else { "▼ " };
                            let mut header_spans = Vec::new();
                            header_spans.extend(flags_slots(item.flags));
                            header_spans.push(Span::styled(
                                collapse_icon,
                                Style::default().fg(theme::SURFACE),
                            ));
                            // Split scope icon from project name
                            let header_text = &item.text;
                            if let Some(rest) = header_text
                                .strip_prefix(project::ICON_PRIVATE)
                                .or_else(|| header_text.strip_prefix(project::ICON_PUBLIC))
                            {
                                let icon = &header_text[..header_text.len() - rest.len()];
                                header_spans.push(Span::styled(
                                    format!("{} ", icon),
                                    Style::default().fg(theme::OVERLAY),
                                ));
                                header_spans.push(Span::styled(
                                    format!("{} ", rest.trim_start()),
                                    Style::default()
                                        .fg(header_color)
                                        .add_modifier(ratatui::style::Modifier::BOLD),
                                ));
                            } else {
                                header_spans.push(Span::styled(
                                    format!("{} ", header_text),
                                    Style::default()
                                        .fg(header_color)
                                        .add_modifier(ratatui::style::Modifier::BOLD),
                                ));
                            }
                            header_spans.push(Span::styled(
                                path_display,
                                Style::default().fg(theme::OVERLAY),
                            ));
                            ListItem::new(Line::from(header_spans))
                        } else if item.is_code_todo {
                            let mut spans = Vec::new();
                            spans.extend(flags_slots(0));
                            spans.push(Span::styled("   ", Style::default()));
                            spans.push(Span::styled(
                                item.text.clone(),
                                Style::default().fg(theme::OVERLAY),
                            ));
                            ListItem::new(Line::from(spans))
                        } else {
                            let (indent, collapse_icon) = if item.has_subtasks {
                                let pad = format!(" {}", "  ".repeat(item.depth as usize));
                                let icon = if item.collapsed { "▶ " } else { "▼ " };
                                (pad, icon)
                            } else {
                                let pad = format!(" {}", "  ".repeat(item.depth as usize + 1));
                                (pad, "")
                            };
                            let (mark, mark_color, bracket_color, style) = match item.state {
                                CheckState::Checked => (
                                    "x",
                                    theme::SAPPHIRE,
                                    theme::OVERLAY,
                                    Style::default().fg(theme::OVERLAY),
                                ),
                                CheckState::Half => (
                                    "/",
                                    theme::YELLOW,
                                    theme::OVERLAY,
                                    Style::default().fg(theme::SUBTEXT),
                                ),
                                CheckState::Unchecked => (
                                    " ",
                                    theme::SURFACE,
                                    match item.depth {
                                        0 => theme::SAPPHIRE,
                                        1 => Color::Rgb(86, 169, 206),
                                        _ => Color::Rgb(56, 139, 176),
                                    },
                                    Style::default().fg(theme::TEXT),
                                ),
                            };
                            let checkbox_spans = vec![
                                Span::styled("[", Style::default().fg(bracket_color)),
                                Span::styled(mark, Style::default().fg(mark_color)),
                                Span::styled("] ", Style::default().fg(bracket_color)),
                            ];
                            let prefix_len = indent.len() + collapse_icon.len() + 7 + 4; // 4 = "[x] " width
                            let text_width = (area.width as usize).saturating_sub(prefix_len + 8);

                            if text_width > 0 && item.text.chars().count() > text_width {
                                let mut lines = vec![];
                                let mut remaining = item.text.as_str();
                                let mut first = true;
                                while !remaining.is_empty() {
                                    // Walk char boundaries to land split_at on valid UTF-8.
                                    // Slicing on a byte index inside a multi-byte char (Å, ä, ö,
                                    // nerdfont icons, …) panics, which previously crashed the TUI
                                    // when terminal resize made text_width tiny.
                                    let split_at = remaining
                                        .char_indices()
                                        .nth(text_width)
                                        .map(|(i, _)| i)
                                        .unwrap_or(remaining.len());
                                    let split_at = if split_at < remaining.len() {
                                        remaining[..split_at]
                                            .rfind(' ')
                                            .map(|i| i + 1)
                                            .unwrap_or(split_at)
                                    } else {
                                        split_at
                                    };
                                    let (chunk, rest) = remaining.split_at(split_at);
                                    let rest = rest.trim_start();

                                    if first {
                                        let hover_dot = hover_flag.and_then(|(hi, d)| {
                                            if hi == idx {
                                                Some(d)
                                            } else {
                                                None
                                            }
                                        });
                                        let mut spans = Vec::new();
                                        spans.extend(flags_slots_with_hover(item.flags, hover_dot));
                                        spans.push(Span::styled(indent.clone(), Style::default()));
                                        spans.push(Span::styled(
                                            collapse_icon,
                                            Style::default().fg(theme::SURFACE),
                                        ));
                                        spans.extend(checkbox_spans.clone());
                                        spans.push(Span::styled(chunk.to_string(), style));
                                        lines.push(Line::from(spans));
                                        first = false;
                                    } else {
                                        let wrap_indent = " ".repeat(prefix_len);
                                        lines.push(Line::from(vec![
                                            Span::styled(wrap_indent, Style::default()),
                                            Span::styled(chunk.to_string(), style),
                                        ]));
                                    }
                                    remaining = rest;
                                }
                                ListItem::new(lines)
                            } else {
                                let hover_dot =
                                    hover_flag
                                        .and_then(|(hi, d)| if hi == idx { Some(d) } else { None });
                                let mut spans = Vec::new();
                                spans.extend(flags_slots_with_hover(item.flags, hover_dot));
                                spans.push(Span::styled(indent, Style::default()));
                                spans.push(Span::styled(
                                    collapse_icon,
                                    Style::default().fg(theme::SURFACE),
                                ));
                                spans.extend(checkbox_spans);
                                spans.push(Span::styled(item.text.clone(), style));
                                ListItem::new(Line::from(spans))
                            }
                        }
                    })
                    .collect();

                // Wrap-aware row count per visible item, used by mouse drag to
                // map screen Y to the right list entry even when entries wrap.
                row_counts_for_mouse = list_items.iter().map(|li| li.height() as u16).collect();

                // Drag visualization: dragged item gets a dim bg, drop target a brighter one.
                let style_at = |list_items: &mut Vec<ListItem>, real_idx: usize, style: Style| {
                    if let Some(pos) = visible.iter().position(|&i| i == real_idx) {
                        let placeholder = ListItem::new("");
                        let original = std::mem::replace(&mut list_items[pos], placeholder);
                        list_items[pos] = original.style(style);
                    }
                };
                if let Some(start) = drag_start {
                    style_at(&mut list_items, start, Style::default().bg(theme::SURFACE));
                }
                if let Some(target) = drag_target {
                    if drag_start != Some(target) {
                        style_at(&mut list_items, target, Style::default().bg(theme::OVERLAY));
                    }
                }

                let todo_count = items
                    .iter()
                    .filter(|i| {
                        !i.is_header
                            && i.depth == 0
                            && !i.is_code_todo
                            && i.state != CheckState::Checked
                    })
                    .count();
                let done_count = items
                    .iter()
                    .filter(|i| {
                        !i.is_header
                            && i.depth == 0
                            && !i.is_code_todo
                            && i.state == CheckState::Checked
                    })
                    .count();

                let mut title_spans = vec![
                    Span::styled(
                        format!(" {} ", tui_title),
                        Style::default()
                            .fg(theme::LAVENDER)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    ),
                    Span::styled("— ", Style::default().fg(theme::SURFACE)),
                    Span::styled(
                        format!("{} ", tui_path),
                        Style::default().fg(theme::OVERLAY),
                    ),
                    Span::styled("— ", Style::default().fg(theme::SURFACE)),
                    Span::styled(
                        format!("{} pending", todo_count),
                        Style::default().fg(theme::SAPPHIRE),
                    ),
                    Span::styled(" · ", Style::default().fg(theme::SURFACE)),
                    Span::styled(
                        format!("{} done ", done_count),
                        Style::default().fg(theme::GREEN),
                    ),
                ];
                let title = Line::from(title_spans);

                // Build the filter strip — a single row below the title with 5 tag dots
                // (dim if not in active filter, lit if active) followed by the search input
                // or hint. Dots are aligned with the dot column on regular todo rows so the
                // filter dots act as visual indicators of "what tags am I filtering by".
                let (_, active_tags) = parse_filter(&search_buffer);
                let mut filter_spans: Vec<Span> = Vec::new();
                filter_spans.push(Span::raw("     ")); // 4 cols highlight indent + 1 col flag-leading space
                let bits = [
                    FLAG_IMPORTANT,
                    FLAG_PRIO,
                    FLAG_LONGTERM,
                    FLAG_IDEA,
                    FLAG_BLOCKED,
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
                filter_spans.push(Span::raw("  ")); // trailing space + small gap
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
                    // While typing on an empty buffer, hint at the syntax — dimmer than the
                    // "filter" label so it doesn't compete visually.
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
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .padding(Padding::new(1, 1, 1, 0));

                // Split the block's inner area into [filter strip, divider, list].
                // Filter sits flush under the top border; a dim "─" divider separates
                // it from the list so the two regions read as distinct.
                let inner = block.inner(chunks[0]);
                let inner_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1), // filter strip
                        Constraint::Length(1), // divider
                        Constraint::Min(1),    // list
                    ])
                    .split(inner);
                let filter_strip_rect = inner_chunks[0];
                let divider_rect = inner_chunks[1];
                let list_inner_rect = inner_chunks[2];

                list_area = list_inner_rect;
                filter_strip_area = filter_strip_rect;

                frame.render_widget(block, chunks[0]);
                frame.render_widget(Paragraph::new(Line::from(filter_spans)), filter_strip_rect);
                let divider_line = Line::from(Span::styled(
                    "─".repeat(divider_rect.width as usize),
                    Style::default().fg(Color::Rgb(50, 50, 65)),
                ));
                frame.render_widget(Paragraph::new(divider_line), divider_rect);

                let list = List::new(list_items)
                    .highlight_style(theme::selected())
                    .highlight_symbol("  ▸ ");
                frame.render_stateful_widget(list, list_inner_rect, &mut state);

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
                } else if flag_mode {
                    let vis = compute_visible(&items, &search_buffer);
                    let vs = state.selected().unwrap_or(0);
                    let ri = vis.get(vs).copied().unwrap_or(0);
                    let cur_flags = if ri < items.len() { items[ri].flags } else { 0 };
                    let mut spans = vec![Span::styled(
                        " tags: ",
                        Style::default().fg(Color::Rgb(205, 152, 115)),
                    )];
                    for (idx, &(bit, _, _, label)) in FLAG_DEFS.iter().enumerate() {
                        let active = cur_flags & bit != 0;
                        let color = FLAG_COLORS[idx];
                        // Number: always tag color. Colon: always gray. Label: tag color when active, gray otherwise.
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
                } else if input_mode || subtask_mode || edit_mode || category_mode {
                    let (label, label_color) = if edit_mode {
                        (" edit: ", Color::Rgb(165, 133, 202))
                    } else if subtask_mode {
                        (" subtask: ", Color::Rgb(148, 157, 210))
                    } else if category_mode {
                        (" new category: ", Color::Rgb(136, 190, 132))
                    } else {
                        (" new: ", Color::Rgb(136, 190, 132))
                    };
                    let (before, after) = input_buffer.split_at(cursor_pos.min(input_buffer.len()));
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
                    let mut spans = vec![
                        Span::styled(label, Style::default().fg(label_color)),
                        Span::styled(before.to_string(), Style::default().fg(theme::TEXT)),
                        Span::styled(
                            cursor_char,
                            Style::default().fg(theme::BASE).bg(theme::SAPPHIRE),
                        ),
                        Span::styled(rest.to_string(), Style::default().fg(theme::TEXT)),
                    ];
                    if let Some(err) = &category_error {
                        spans.push(Span::styled(
                            format!("  ← {}", err),
                            Style::default().fg(theme::RED),
                        ));
                    }
                    Line::from(spans)
                } else if vim.active {
                    Line::from(vec![Span::styled(
                        vim.buffer.as_str(),
                        Style::default().fg(theme::MAUVE),
                    )])
                } else {
                    let width = chunks[1].width as usize;
                    let list_height = chunks[0].height.saturating_sub(4) as usize;
                    let scroll_info = if visible.len() > list_height {
                        let pos = state.selected().unwrap_or(0) + 1;
                        let total = visible.len();
                        format!(" {}/{} ", pos, total)
                    } else {
                        String::new()
                    };

                    // Minimal bar: "? help" on the left, "[pos/total] q quit" on the right.
                    // Full keybindings live in the help overlay (?).
                    let left_len = " ? help".len();
                    let right_len = " q quit ".len() + scroll_info.len();
                    let padding = width.saturating_sub(left_len + right_len);
                    Line::from(vec![
                        Span::styled(" ", Style::default()),
                        Span::styled("?", Style::default().fg(theme::YELLOW).add_modifier(bold)),
                        Span::styled(" help", Style::default().fg(theme::OVERLAY)),
                        Span::styled(" ".repeat(padding), Style::default()),
                        Span::styled(scroll_info, Style::default().fg(theme::OVERLAY)),
                        Span::styled(" ", Style::default()),
                        Span::styled("q", Style::default().fg(theme::PEACH).add_modifier(bold)),
                        Span::styled(" quit ", Style::default().fg(theme::OVERLAY)),
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
                            Span::styled("  x", Style::default().fg(theme::SAPPHIRE)),
                            Span::styled(
                                " / space / enter  check/uncheck",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  a", Style::default().fg(theme::YELLOW)),
                            Span::styled(
                                "                  almost done [/]",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  n", Style::default().fg(theme::GREEN)),
                            Span::styled(
                                "                  new todo",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  N", Style::default().fg(theme::GREEN)),
                            Span::styled(
                                "                  new category",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  s", Style::default().fg(theme::LAVENDER)),
                            Span::styled(
                                "                  add subtask",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  e", Style::default().fg(theme::MAUVE)),
                            Span::styled(
                                "                  edit text",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  d", Style::default().fg(theme::RED)),
                            Span::styled(
                                "                  delete",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  t", Style::default().fg(theme::PEACH)),
                            Span::styled(
                                "                  tags (1-5 to toggle)",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  f", Style::default().fg(theme::GREEN)),
                            Span::styled(
                                "                  focus section",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  /", Style::default().fg(theme::SAPPHIRE)),
                            Span::styled(
                                "                  filter — fuzzy text + #tagname, or click a dot",
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
                            Span::styled("  h/l", Style::default().fg(theme::TEXT)),
                            Span::styled(
                                "                collapse / expand",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  J/K", Style::default().fg(theme::TEXT)),
                            Span::styled(
                                "                move todo up / down",
                                Style::default().fg(theme::TEXT),
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("  drag", Style::default().fg(theme::TEXT)),
                            Span::styled(
                                "               mouse drag to reorder",
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
                    let help_w = 42_u16;
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

        // Mouse: scroll + drag-to-reorder
        if let Event::Mouse(mouse) = ev {
            let vis_sel = state.selected().unwrap_or(0);
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    if vis_sel + 1 < visible_for_mouse.len() {
                        state.select(Some(vis_sel + 1));
                    }
                }
                MouseEventKind::ScrollUp => {
                    if vis_sel > 0 {
                        state.select(Some(vis_sel - 1));
                    }
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    // Filter strip clicks: dot → toggle tag, anywhere else → enter search mode.
                    if mouse.row == filter_strip_area.y {
                        if let Some(d) = mouse_x_to_filter_dot(mouse.column, filter_strip_area.x) {
                            toggle_filter_tag(&mut search_buffer, FLAG_DEFS[d as usize].1);
                            cursor_pos = search_buffer.len();
                            continue;
                        }
                        // Clicking on the search field area activates typing mode.
                        search_mode = true;
                        cursor_pos = search_buffer.len();
                        continue;
                    }
                    if let Some(real_idx) = mouse_y_to_real_idx(
                        mouse.row,
                        list_area,
                        state.offset(),
                        &visible_for_mouse,
                        &row_counts_for_mouse,
                    ) {
                        if let Some(pos) = visible_for_mouse.iter().position(|&i| i == real_idx) {
                            state.select(Some(pos));
                        }
                        let dot_hit = mouse_x_to_dot(mouse.column, list_area.x);
                        let it = &items[real_idx];
                        if let Some(d) = dot_hit {
                            if !it.is_header && !it.is_code_todo {
                                items[real_idx].flags ^= FLAG_DEFS[d as usize].0;
                                continue; // skip drag candidate setup
                            }
                        }
                        drag_candidate = Some(real_idx);
                        drag_active = false;
                        drag_start = None;
                        drag_target = None;
                    }
                }
                MouseEventKind::Moved => {
                    // Update hover preview for tag dots. Only tracks while NOT dragging
                    // (during drag we leave the dots alone so the drag highlight is clean).
                    if drag_candidate.is_none() {
                        let new_hover = mouse_y_to_real_idx(
                            mouse.row,
                            list_area,
                            state.offset(),
                            &visible_for_mouse,
                            &row_counts_for_mouse,
                        )
                        .and_then(|idx| {
                            let it = &items[idx];
                            if it.is_header || it.is_code_todo {
                                return None;
                            }
                            mouse_x_to_dot(mouse.column, list_area.x).map(|d| (idx, d))
                        });
                        if new_hover != hover_flag {
                            hover_flag = new_hover;
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    if let Some(start) = drag_candidate {
                        // Headers and code-todos can't be dragged; ignore the motion.
                        if !items[start].is_header && !items[start].is_code_todo {
                            drag_active = true;
                            drag_start = Some(start);
                            if let Some(real_idx) = mouse_y_to_real_idx(
                                mouse.row,
                                list_area,
                                state.offset(),
                                &visible_for_mouse,
                                &row_counts_for_mouse,
                            ) {
                                drag_target = Some(real_idx);
                            }
                        }
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    if drag_active {
                        // Real drag — perform reorder if endpoints are compatible.
                        if let (Some(start), Some(target)) = (drag_start, drag_target) {
                            if start != target && can_drag(&items, start, target) {
                                let new_start = perform_drag_move(&mut items, start, target);
                                let new_vis = get_visible_indices(&items);
                                if let Some(pos) = new_vis.iter().position(|&i| i == new_start) {
                                    state.select(Some(pos));
                                }
                            }
                        }
                    } else if let Some(real_idx) = drag_candidate {
                        // Click without movement — toggle headers and subtask parents.
                        // Leaf todos: just leave the cursor where it landed on Down.
                        if items[real_idx].is_header || items[real_idx].has_subtasks {
                            items[real_idx].collapsed = !items[real_idx].collapsed;
                        }
                    }
                    drag_candidate = None;
                    drag_active = false;
                    drag_start = None;
                    drag_target = None;
                }
                _ => {}
            }
            continue;
        }

        if let Event::Key(key) = ev {
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

            // Confirm delete
            if confirm_delete {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Enter => {
                        let vis = get_visible_indices(&items);
                        let vs = state.selected().unwrap_or(0);
                        let ri = vis.get(vs).copied().unwrap_or(0);
                        if ri < items.len() && !items[ri].is_header {
                            if items[ri].has_subtasks {
                                let parent_depth = items[ri].depth;
                                let mut end = ri + 1;
                                while end < items.len() && items[end].depth > parent_depth {
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
                    _ => {
                        confirm_delete = false;
                    }
                }
                continue;
            }

            // Flag mode — stays open until `t` or Esc, so the user can tag several
            // tasks in one go (navigate with j/k between toggles, etc.).
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
                            let vis = compute_visible(&items, &search_buffer);
                            let vs = state.selected().unwrap_or(0);
                            let ri = vis.get(vs).copied().unwrap_or(0);
                            if ri < items.len() && !items[ri].is_header && !items[ri].is_code_todo {
                                items[ri].flags ^= FLAG_DEFS[idx].0;
                            }
                        }
                    }
                    KeyCode::Char('t') | KeyCode::Esc => {
                        flag_mode = false;
                    }
                    // `/` is a global shortcut — exit flag mode and let the search/filter
                    // handler pick the key up.
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
                // Reset selection when search changes
                state.select(Some(0));
                continue;
            }

            // Category mode (new top-level category — global view only)
            if category_mode {
                match key.code {
                    KeyCode::Enter => {
                        let name = input_buffer.trim().to_string();
                        if name.is_empty() {
                            category_error = Some("name cannot be empty".into());
                        } else if name.contains('/') || name.contains('\\') {
                            category_error = Some("name cannot contain '/' or '\\'".into());
                        } else {
                            let cat_dir =
                                PathBuf::from(&config.notez_root).join("_todos").join(&name);
                            if cat_dir.exists() {
                                category_error =
                                    Some(format!("category '{}' already exists", name));
                            } else {
                                fs::create_dir_all(&cat_dir).ok();
                                let cat_file = cat_dir.join("TODO.md");
                                fs::write(&cat_file, "# TODO\n\n").ok();
                                // Persist current in-memory edits, then reload so the
                                // new category appears in its alphabetical slot.
                                save_all_todos(&items);
                                items = load_global_todos(config);
                                input_buffer.clear();
                                cursor_pos = 0;
                                category_mode = false;
                                category_error = None;
                                if let Some(real_idx) =
                                    items.iter().position(|i| i.is_header && i.project == name)
                                {
                                    let new_vis = get_visible_indices(&items);
                                    if let Some(pos) = new_vis.iter().position(|&i| i == real_idx) {
                                        state.select(Some(pos));
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Esc => {
                        input_buffer.clear();
                        cursor_pos = 0;
                        category_mode = false;
                        category_error = None;
                    }
                    KeyCode::Left => {
                        cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
                    }
                    KeyCode::Right => {
                        cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
                    }
                    KeyCode::Backspace => {
                        if cursor_pos > 0 {
                            let prev = prev_char_boundary(&input_buffer, cursor_pos);
                            input_buffer.remove(prev);
                            cursor_pos = prev;
                        }
                    }
                    KeyCode::Char(c) => {
                        input_buffer.insert(cursor_pos, c);
                        cursor_pos += c.len_utf8();
                        category_error = None;
                    }
                    _ => {}
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
                            items.insert(
                                insert_at,
                                TodoItem {
                                    text: input_buffer.clone(),
                                    state: CheckState::Unchecked,
                                    source,
                                    project,
                                    is_header: false,
                                    depth: 0,
                                    has_subtasks: false,
                                    collapsed: false,
                                    is_code_todo: false,
                                    flags: 0,
                                },
                            );
                            let new_vis = get_visible_indices(&items);
                            if let Some(pos) = new_vis.iter().position(|&i| i == insert_at) {
                                state.select(Some(pos));
                            }
                        }
                        input_buffer.clear();
                        cursor_pos = 0;
                        input_mode = false;
                    }
                    KeyCode::Esc => {
                        input_buffer.clear();
                        cursor_pos = 0;
                        input_mode = false;
                    }
                    KeyCode::Left => {
                        cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
                    }
                    KeyCode::Right => {
                        cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
                    }
                    KeyCode::Backspace => {
                        if cursor_pos > 0 {
                            let prev = prev_char_boundary(&input_buffer, cursor_pos);
                            input_buffer.remove(prev);
                            cursor_pos = prev;
                        }
                    }
                    KeyCode::Char(c) => {
                        input_buffer.insert(cursor_pos, c);
                        cursor_pos += c.len_utf8();
                    }
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
                            let parent_idx = if items[ri].is_header || items[ri].depth >= 2 {
                                input_buffer.clear();
                                subtask_mode = false;
                                continue;
                            } else {
                                ri
                            };

                            let child_depth = items[parent_idx].depth + 1;
                            items[parent_idx].has_subtasks = true;
                            let source = items[parent_idx].source.clone();
                            let project = items[parent_idx].project.clone();

                            // Insert after the last child of this parent
                            let mut insert_at = parent_idx + 1;
                            while insert_at < items.len()
                                && items[insert_at].depth > items[parent_idx].depth
                            {
                                insert_at += 1;
                            }

                            items.insert(
                                insert_at,
                                TodoItem {
                                    text: input_buffer.clone(),
                                    state: CheckState::Unchecked,
                                    source,
                                    project,
                                    is_header: false,
                                    depth: child_depth,
                                    has_subtasks: false,
                                    collapsed: false,
                                    is_code_todo: false,
                                    flags: 0,
                                },
                            );
                            // Expand parent if collapsed
                            items[parent_idx].collapsed = false;
                            let new_vis = get_visible_indices(&items);
                            if let Some(pos) = new_vis.iter().position(|&i| i == insert_at) {
                                state.select(Some(pos));
                            }
                        }
                        input_buffer.clear();
                        cursor_pos = 0;
                        subtask_mode = false;
                    }
                    KeyCode::Esc => {
                        input_buffer.clear();
                        cursor_pos = 0;
                        subtask_mode = false;
                    }
                    KeyCode::Left => {
                        cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
                    }
                    KeyCode::Right => {
                        cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
                    }
                    KeyCode::Backspace => {
                        if cursor_pos > 0 {
                            let prev = prev_char_boundary(&input_buffer, cursor_pos);
                            input_buffer.remove(prev);
                            cursor_pos = prev;
                        }
                    }
                    KeyCode::Char(c) => {
                        input_buffer.insert(cursor_pos, c);
                        cursor_pos += c.len_utf8();
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
                        cursor_pos = 0;
                        edit_mode = false;
                    }
                    KeyCode::Esc => {
                        input_buffer.clear();
                        cursor_pos = 0;
                        edit_mode = false;
                    }
                    KeyCode::Left => {
                        cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
                    }
                    KeyCode::Right => {
                        cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
                    }
                    KeyCode::Backspace => {
                        if cursor_pos > 0 {
                            let prev = prev_char_boundary(&input_buffer, cursor_pos);
                            input_buffer.remove(prev);
                            cursor_pos = prev;
                        }
                    }
                    KeyCode::Char(c) => {
                        input_buffer.insert(cursor_pos, c);
                        cursor_pos += c.len_utf8();
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

            let visible = compute_visible(&items, &search_buffer);
            let vis_sel = state.selected().unwrap_or(0);
            let real_idx = visible.get(vis_sel).copied().unwrap_or(0);

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
                    if vis_sel + 1 < visible.len() {
                        let target_real = visible[vis_sel + 1];
                        if focus_active {
                            let new_header = (0..=target_real).rev().find(|&i| items[i].is_header);
                            let old_header = (0..=real_idx).rev().find(|&i| items[i].is_header);
                            if new_header != old_header {
                                if let Some(oh) = old_header {
                                    items[oh].collapsed = true;
                                }
                                if let Some(nh) = new_header {
                                    items[nh].collapsed = false;
                                }
                                // Recalculate visible and find target position
                                let new_vis = get_visible_indices(&items);
                                if let Some(pos) = new_vis.iter().position(|&i| i == target_real) {
                                    state.select(Some(pos));
                                }
                            } else {
                                state.select(Some(vis_sel + 1));
                            }
                        } else {
                            state.select(Some(vis_sel + 1));
                        }
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if vis_sel > 0 {
                        let target_real = visible[vis_sel - 1];
                        if focus_active {
                            let new_header = (0..=target_real).rev().find(|&i| items[i].is_header);
                            let old_header = (0..=real_idx).rev().find(|&i| items[i].is_header);
                            if new_header != old_header {
                                if let Some(oh) = old_header {
                                    items[oh].collapsed = true;
                                }
                                if let Some(nh) = new_header {
                                    items[nh].collapsed = false;
                                }
                                let new_vis = get_visible_indices(&items);
                                if let Some(pos) = new_vis.iter().position(|&i| i == target_real) {
                                    state.select(Some(pos));
                                }
                            } else {
                                state.select(Some(vis_sel - 1));
                            }
                        } else {
                            state.select(Some(vis_sel - 1));
                        }
                    }
                }

                KeyCode::Char('l') | KeyCode::Right => {
                    if real_idx < items.len() && items[real_idx].collapsed {
                        items[real_idx].collapsed = false;
                        focus_active = false;
                    }
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    if real_idx < items.len()
                        && !items[real_idx].collapsed
                        && (items[real_idx].has_subtasks || items[real_idx].is_header)
                    {
                        items[real_idx].collapsed = true;
                        focus_active = false;
                    }
                }

                KeyCode::Char('v') => {
                    // Find current section header
                    let current_header = (0..=real_idx)
                        .rev()
                        .find(|&i| items[i].is_header)
                        .unwrap_or(0);
                    let any_collapsed = items
                        .iter()
                        .any(|i| (i.is_header || i.has_subtasks) && i.collapsed);
                    for item in items.iter_mut() {
                        if item.is_header || item.has_subtasks {
                            item.collapsed = !any_collapsed;
                        }
                    }
                    // Reposition to the header of the section we were in
                    let new_vis = get_visible_indices(&items);
                    if let Some(pos) = new_vis.iter().position(|&i| i == current_header) {
                        // Scroll so selected header is near the bottom of the viewport
                        *state.offset_mut() = 0;
                        state.select(Some(pos));
                    }
                    focus_active = false;
                }

                KeyCode::Char(' ') | KeyCode::Char('x') | KeyCode::Enter => {
                    if real_idx < items.len()
                        && !items[real_idx].is_header
                        && !items[real_idx].is_code_todo
                    {
                        if items[real_idx].has_subtasks {
                            let parent_depth = items[real_idx].depth;
                            let all_checked = {
                                let mut j = real_idx + 1;
                                let mut all = true;
                                while j < items.len() && items[j].depth > parent_depth {
                                    if items[j].state != CheckState::Checked {
                                        all = false;
                                    }
                                    j += 1;
                                }
                                all
                            };
                            let new_state = if all_checked {
                                CheckState::Unchecked
                            } else {
                                CheckState::Checked
                            };
                            let mut j = real_idx + 1;
                            while j < items.len() && items[j].depth > parent_depth {
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

                KeyCode::Char('a') => {
                    if real_idx < items.len()
                        && !items[real_idx].is_header
                        && !items[real_idx].has_subtasks
                        && !items[real_idx].is_code_todo
                    {
                        items[real_idx].state = match items[real_idx].state {
                            CheckState::Half => CheckState::Unchecked,
                            _ => CheckState::Half,
                        };
                    }
                }

                KeyCode::Char('n') => {
                    input_mode = true;
                    input_buffer.clear();
                    cursor_pos = 0;
                }

                // New top-level category (only meaningful in global view, where
                // categories live as ~/notez/_todos/<name>/TODO.md).
                KeyCode::Char('N') => {
                    if global {
                        category_mode = true;
                        input_buffer.clear();
                        cursor_pos = 0;
                        category_error = None;
                    }
                }

                KeyCode::Char('s') => {
                    if real_idx < items.len()
                        && !items[real_idx].is_header
                        && !items[real_idx].is_code_todo
                        && items[real_idx].depth < 2
                    {
                        subtask_mode = true;
                        input_buffer.clear();
                        cursor_pos = 0;
                    }
                }

                KeyCode::Char('d') => {
                    if real_idx < items.len()
                        && !items[real_idx].is_header
                        && !items[real_idx].is_code_todo
                    {
                        confirm_delete = true;
                    }
                }

                KeyCode::Char('e') => {
                    if real_idx < items.len()
                        && !items[real_idx].is_header
                        && !items[real_idx].is_code_todo
                    {
                        edit_mode = true;
                        edit_idx = real_idx;
                        input_buffer = items[real_idx].text.clone();
                        cursor_pos = input_buffer.len();
                    }
                }

                KeyCode::Char('t') => {
                    // Always opens — toggles on a header are no-ops (handled inside flag_mode).
                    flag_mode = true;
                }

                KeyCode::Char('f') => {
                    if real_idx < items.len() {
                        if focus_active {
                            // Find the header of the section we're currently in
                            let current_header = (0..=real_idx)
                                .rev()
                                .find(|&i| items[i].is_header)
                                .unwrap_or(real_idx);
                            // Restore previous collapsed state
                            for &(idx, was_collapsed) in &pre_focus_collapsed {
                                if idx < items.len() {
                                    items[idx].collapsed = was_collapsed;
                                }
                            }
                            // Reposition selection to the header of the section we were in
                            let new_vis = get_visible_indices(&items);
                            if let Some(pos) = new_vis.iter().position(|&i| i == current_header) {
                                state.select(Some(pos));
                            }
                            focus_active = false;
                        } else {
                            // Save current collapsed state
                            pre_focus_collapsed = items
                                .iter()
                                .enumerate()
                                .filter(|(_, item)| item.is_header)
                                .map(|(i, item)| (i, item.collapsed))
                                .collect();
                            // Focus: collapse all except current section
                            let focused_header = (0..=real_idx).rev().find(|&i| items[i].is_header);
                            for i in 0..items.len() {
                                if items[i].is_header {
                                    items[i].collapsed = Some(i) != focused_header;
                                }
                            }
                            focus_active = true;
                        }
                    }
                }

                KeyCode::Char('/') => {
                    search_mode = true;
                    search_buffer.clear();
                    cursor_pos = 0;
                }

                // Move todo (and its subtree) down past the next sibling block
                KeyCode::Char('J') => {
                    if real_idx < items.len()
                        && !items[real_idx].is_header
                        && !items[real_idx].is_code_todo
                    {
                        let depth = items[real_idx].depth;
                        let a_end = block_end(&items, real_idx);
                        // Next sibling block must exist at the same depth, in the same section
                        if a_end < items.len()
                            && !items[a_end].is_header
                            && items[a_end].depth == depth
                        {
                            let b_end = block_end(&items, a_end);
                            let a_len = a_end - real_idx;
                            // Rotate [A..B] left by len(A) so order becomes [B..A]
                            items[real_idx..b_end].rotate_left(a_len);
                            let new_a_start = real_idx + (b_end - a_end);
                            let new_vis = get_visible_indices(&items);
                            if let Some(pos) = new_vis.iter().position(|&i| i == new_a_start) {
                                state.select(Some(pos));
                            }
                        }
                    }
                }

                // Move todo (and its subtree) up past the previous sibling block
                KeyCode::Char('K') => {
                    if real_idx < items.len()
                        && real_idx > 0
                        && !items[real_idx].is_header
                        && !items[real_idx].is_code_todo
                    {
                        let depth = items[real_idx].depth;
                        // Walk back to find the previous sibling at the same depth in this section
                        let mut prev_start: Option<usize> = None;
                        for i in (0..real_idx).rev() {
                            if items[i].is_header {
                                break;
                            }
                            if items[i].depth < depth {
                                break;
                            }
                            if items[i].depth == depth {
                                prev_start = Some(i);
                                break;
                            }
                        }
                        if let Some(prev_start) = prev_start {
                            let a_end = block_end(&items, real_idx);
                            let b_len = real_idx - prev_start;
                            // Rotate [B..A_end] left by len(B) so order becomes [A..B]
                            items[prev_start..a_end].rotate_left(b_len);
                            let new_vis = get_visible_indices(&items);
                            if let Some(pos) = new_vis.iter().position(|&i| i == prev_start) {
                                state.select(Some(pos));
                            }
                        }
                    }
                }

                KeyCode::Char('?') => {
                    show_help = true;
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
        let content = "- [ ] parent\n  - [ ] sub1\n  - [x] sub2\n    - [ ] subsub\n- [ ] other\n";
        let items = parse_todos_from_content(content);
        assert_eq!(items.len(), 5);
        assert_eq!(items[0].2, 0); // parent
        assert_eq!(items[1].2, 1); // subtask
        assert_eq!(items[2].2, 1); // subtask
        assert_eq!(items[3].2, 2); // sub-subtask
        assert_eq!(items[4].2, 0); // top-level
    }

    #[test]
    fn derive_parent_from_subtasks() {
        let source = PathBuf::from("/tmp/TODO.md");
        let mut items = vec![
            TodoItem {
                text: "parent".into(),
                state: CheckState::Unchecked,
                source: source.clone(),
                project: "t".into(),
                is_header: false,
                depth: 0,
                has_subtasks: true,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
            TodoItem {
                text: "sub1".into(),
                state: CheckState::Checked,
                source: source.clone(),
                project: "t".into(),
                is_header: false,
                depth: 1,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
            TodoItem {
                text: "sub2".into(),
                state: CheckState::Unchecked,
                source: source.clone(),
                project: "t".into(),
                is_header: false,
                depth: 1,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
        ];
        derive_parent_states(&mut items);
        assert_eq!(items[0].state, CheckState::Half); // 1 of 2 done
    }

    #[test]
    fn derive_parent_clears_has_subtasks_when_empty() {
        let source = PathBuf::from("/tmp/TODO.md");
        let mut items = vec![TodoItem {
            text: "parent".into(),
            state: CheckState::Checked,
            source: source.clone(),
            project: "t".into(),
            is_header: false,
            depth: 0,
            has_subtasks: true,
            collapsed: false,
            is_code_todo: false,
            flags: 0,
        }];
        derive_parent_states(&mut items);
        assert!(
            !items[0].has_subtasks,
            "has_subtasks must be cleared when no children remain"
        );
    }

    #[test]
    fn derive_parent_all_done() {
        let source = PathBuf::from("/tmp/TODO.md");
        let mut items = vec![
            TodoItem {
                text: "parent".into(),
                state: CheckState::Unchecked,
                source: source.clone(),
                project: "t".into(),
                is_header: false,
                depth: 0,
                has_subtasks: true,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
            TodoItem {
                text: "sub1".into(),
                state: CheckState::Checked,
                source: source.clone(),
                project: "t".into(),
                is_header: false,
                depth: 1,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
            TodoItem {
                text: "sub2".into(),
                state: CheckState::Checked,
                source: source.clone(),
                project: "t".into(),
                is_header: false,
                depth: 1,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
        ];
        derive_parent_states(&mut items);
        assert_eq!(items[0].state, CheckState::Checked); // all done
    }

    #[test]
    fn serialize_with_subtasks() {
        let source = PathBuf::from("/tmp/TODO.md");
        let items = vec![
            TodoItem {
                text: "parent".into(),
                state: CheckState::Half,
                source: source.clone(),
                project: "t".into(),
                is_header: false,
                depth: 0,
                has_subtasks: true,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
            TodoItem {
                text: "sub1".into(),
                state: CheckState::Checked,
                source: source.clone(),
                project: "t".into(),
                is_header: false,
                depth: 1,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
            TodoItem {
                text: "sub2".into(),
                state: CheckState::Unchecked,
                source: source.clone(),
                project: "t".into(),
                is_header: false,
                depth: 1,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
        ];
        let md = serialize_todos_for_file(&items, &source);
        assert!(md.contains("- [/] parent"));
        assert!(md.contains("  - [x] sub1"));
        assert!(md.contains("  - [ ] sub2"));
    }

    fn item(text: &str, depth: u8) -> TodoItem {
        TodoItem {
            text: text.into(),
            state: CheckState::Unchecked,
            source: PathBuf::from("/tmp/TODO.md"),
            project: "t".into(),
            is_header: false,
            depth,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: false,
            flags: 0,
        }
    }

    fn header(text: &str) -> TodoItem {
        TodoItem {
            is_header: true,
            ..item(text, 0)
        }
    }

    #[test]
    fn block_end_includes_descendants() {
        let items = vec![
            item("A", 0),
            item("A.1", 1),
            item("A.1.1", 2),
            item("A.2", 1),
            item("B", 0),
        ];
        assert_eq!(
            block_end(&items, 0),
            4,
            "block of A includes A.1, A.1.1, A.2"
        );
        assert_eq!(block_end(&items, 1), 3, "block of A.1 includes A.1.1");
        assert_eq!(block_end(&items, 4), 5, "block of B is just B");
    }

    #[test]
    fn can_drag_same_depth_same_section() {
        let items = vec![header("## A"), item("X", 0), item("Y", 0), item("Z", 0)];
        assert!(can_drag(&items, 1, 3), "X → Z within section");
        assert!(can_drag(&items, 3, 1), "Z → X (drag up)");
    }

    #[test]
    fn can_drag_rejects_cross_section() {
        let items = vec![header("## A"), item("X", 0), header("## B"), item("Y", 0)];
        assert!(!can_drag(&items, 1, 3), "X → Y crosses section header");
    }

    #[test]
    fn can_drag_rejects_cross_parent() {
        let items = vec![item("A", 0), item("A.1", 1), item("B", 0), item("B.1", 1)];
        assert!(!can_drag(&items, 1, 3), "A.1 → B.1 crosses parent boundary");
    }

    #[test]
    fn can_drag_rejects_different_depth() {
        let items = vec![item("X", 0), item("Y", 1)];
        assert!(!can_drag(&items, 0, 1));
    }

    #[test]
    fn perform_drag_move_down() {
        let mut items = vec![
            item("A", 0),
            item("A.1", 1),
            item("B", 0),
            item("B.1", 1),
            item("B.2", 1),
        ];
        // Drag A down past B
        let new_idx = perform_drag_move(&mut items, 0, 2);
        assert_eq!(
            items.iter().map(|i| i.text.clone()).collect::<Vec<_>>(),
            vec!["B", "B.1", "B.2", "A", "A.1"]
        );
        assert_eq!(new_idx, 3, "A's new position");
    }

    #[test]
    fn perform_drag_move_up() {
        let mut items = vec![item("A", 0), item("B", 0), item("B.1", 1), item("C", 0)];
        // Drag C up past B (B has subtasks, must go with it)
        let new_idx = perform_drag_move(&mut items, 3, 1);
        assert_eq!(
            items.iter().map(|i| i.text.clone()).collect::<Vec<_>>(),
            vec!["A", "C", "B", "B.1"]
        );
        assert_eq!(new_idx, 1);
    }

    #[test]
    fn block_end_stops_at_header() {
        let items = vec![
            item("A", 0),
            item("A.1", 1),
            header("## Section"),
            item("B", 0),
        ];
        assert_eq!(block_end(&items, 0), 2, "block stops before header");
    }

    #[test]
    fn char_boundaries_handle_swedish() {
        // 'å' is 2 bytes; the old cursor logic stepped by 1 and panicked when
        // split_at/insert/remove was called on a non-boundary index.
        let s = "å";
        assert_eq!(s.len(), 2);
        assert_eq!(
            next_char_boundary(s, 0),
            2,
            "right past 'å' lands on end-of-string"
        );
        assert_eq!(
            prev_char_boundary(s, 2),
            0,
            "left from end of 'å' lands on start"
        );
        assert_eq!(prev_char_boundary(s, 0), 0, "left from start clamps");
        assert_eq!(next_char_boundary(s, 2), 2, "right from end clamps");
    }

    #[test]
    fn char_boundaries_skip_into_multibyte() {
        // If somehow asked from an interior byte, we still land on a boundary.
        let s = "aåb"; // bytes: a(0) å(1..3) b(3)
        assert_eq!(
            prev_char_boundary(s, 3),
            1,
            "left from 'b' goes to start of 'å'"
        );
        assert_eq!(
            next_char_boundary(s, 1),
            3,
            "right from start of 'å' goes to 'b'"
        );
    }

    #[test]
    fn fuzzy_match_basic() {
        assert!(fuzzy_match("fix the test", "ftt"));
        assert!(fuzzy_match("fix the test", "fix"));
        assert!(fuzzy_match("fix the test", ""));
        assert!(!fuzzy_match("fix the test", "xyz"));
        assert!(!fuzzy_match("hello", "helloo"));
    }

    #[test]
    fn fuzzy_match_case_insensitive() {
        assert!(fuzzy_match("Fix The Test", "ftt"));
        assert!(fuzzy_match("Förslag", "FÖR"));
    }

    #[test]
    fn parse_filter_extracts_tags() {
        let (text, tags) = parse_filter("foo #prio bar #important");
        assert_eq!(text, "foo bar");
        assert_eq!(tags, FLAG_PRIO | FLAG_IMPORTANT);
    }

    #[test]
    fn parse_filter_unknown_tag_falls_back_to_text() {
        let (text, tags) = parse_filter("#foobar");
        assert_eq!(text, "#foobar");
        assert_eq!(tags, 0);
    }

    #[test]
    fn toggle_filter_tag_adds_and_removes() {
        let mut buf = String::new();
        toggle_filter_tag(&mut buf, "prio");
        assert_eq!(buf, "#prio");
        toggle_filter_tag(&mut buf, "important");
        assert_eq!(buf, "#prio #important");
        toggle_filter_tag(&mut buf, "prio"); // remove
        assert_eq!(buf, "#important");
    }

    #[test]
    fn toggle_filter_tag_preserves_text() {
        let mut buf = "bug fix".to_string();
        toggle_filter_tag(&mut buf, "prio");
        assert_eq!(buf, "bug fix #prio");
        toggle_filter_tag(&mut buf, "prio");
        assert_eq!(buf, "bug fix");
    }

    #[test]
    fn parse_filter_only_text() {
        let (text, tags) = parse_filter("hello world");
        assert_eq!(text, "hello world");
        assert_eq!(tags, 0);
    }

    #[test]
    fn item_matches_combines_text_and_tags() {
        let i = TodoItem {
            text: "fix login bug".into(),
            state: CheckState::Unchecked,
            source: PathBuf::from("/tmp/TODO.md"),
            project: "t".into(),
            is_header: false,
            depth: 0,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: false,
            flags: FLAG_PRIO,
        };
        assert!(item_matches(&i, "fix", &[FLAG_PRIO]));
        assert!(item_matches(&i, "flb", &[FLAG_PRIO])); // fuzzy
        assert!(!item_matches(&i, "fix", &[FLAG_IMPORTANT])); // wrong tag
        assert!(!item_matches(&i, "xyz", &[FLAG_PRIO])); // no text match
    }

    #[test]
    fn match_tag_prefix_digit_indices() {
        assert_eq!(match_tag_prefix("#1"), FLAG_IMPORTANT);
        assert_eq!(match_tag_prefix("#5"), FLAG_BLOCKED);
        assert_eq!(match_tag_prefix("#13"), FLAG_IMPORTANT | FLAG_LONGTERM);
        assert_eq!(
            match_tag_prefix("#12345"),
            FLAG_IMPORTANT | FLAG_PRIO | FLAG_LONGTERM | FLAG_IDEA | FLAG_BLOCKED,
        );
        // Out-of-range digits silently ignored.
        assert_eq!(match_tag_prefix("#0"), 0);
        assert_eq!(match_tag_prefix("#6"), 0);
        assert_eq!(match_tag_prefix("#19"), FLAG_IMPORTANT); // '9' ignored
    }

    #[test]
    fn match_tag_prefix_handles_partial_and_bare() {
        // bare # matches all tags
        let all = FLAG_DEFS.iter().fold(0u8, |a, &(b, _, _, _)| a | b);
        assert_eq!(match_tag_prefix("#"), all);
        // exact name
        assert_eq!(match_tag_prefix("#prio"), FLAG_PRIO);
        // unique prefix
        assert_eq!(match_tag_prefix("#impo"), FLAG_IMPORTANT);
        // ambiguous prefix matches multiple
        assert_eq!(match_tag_prefix("#i"), FLAG_IMPORTANT | FLAG_IDEA);
        // no match → 0
        assert_eq!(match_tag_prefix("#zzz"), 0);
        // not a tag at all → 0
        assert_eq!(match_tag_prefix("hello"), 0);
    }

    #[test]
    fn item_matches_prefix_or_semantics() {
        let i = TodoItem {
            text: "x".into(),
            state: CheckState::Unchecked,
            source: PathBuf::from("/tmp/TODO.md"),
            project: "t".into(),
            is_header: false,
            depth: 0,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: false,
            flags: FLAG_IDEA,
        };
        // #i = important | idea — item with idea passes (≥1 in set)
        assert!(item_matches(&i, "", &[FLAG_IMPORTANT | FLAG_IDEA]));
        // single-bit set (FLAG_IMPORTANT) — item lacks it
        assert!(!item_matches(&i, "", &[FLAG_IMPORTANT]));
    }

    #[test]
    fn filter_keep_includes_ancestors() {
        let items = vec![
            header("## Section"),
            item("parent", 0),
            TodoItem {
                text: "child #prio".into(),
                state: CheckState::Unchecked,
                source: PathBuf::from("/tmp/TODO.md"),
                project: "t".into(),
                is_header: false,
                depth: 1,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: FLAG_PRIO,
            },
            item("other", 0),
        ];
        let keep = compute_filter_keep(&items, "", &[FLAG_PRIO]);
        assert!(keep[0], "section header included");
        assert!(keep[1], "parent included as ancestor");
        assert!(keep[2], "matching child");
        assert!(!keep[3], "unrelated other excluded");
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
            TodoItem {
                text: "buy milk".into(),
                state: CheckState::Unchecked,
                source: source.clone(),
                project: "test".into(),
                is_header: false,
                depth: 0,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
            TodoItem {
                text: "fix bug".into(),
                state: CheckState::Checked,
                source: source.clone(),
                project: "test".into(),
                is_header: false,
                depth: 0,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
            },
        ];
        let md = serialize_todos_for_file(&items, &source);
        let parsed = parse_todos_from_content(&md);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, "buy milk");
        assert_eq!(parsed[1].1, CheckState::Checked);
    }
}
