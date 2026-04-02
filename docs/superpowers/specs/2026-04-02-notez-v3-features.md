# notez v3 — Three New Features Spec

## Feature 1: Interactive Tree (`notez tree`)

Replaces the static tree output with a ratatui TUI. First view shows the same depth as the current static view (numbered dirs with note counts). Users navigate with vim motions, expand directories, and open files directly.

### Commands
- `notez tree` — interactive TUI, local `./notez/`
- `notez tree -g` — interactive TUI, global `~/notez/`

### TUI Behavior
- **Initial view:** numbered dirs with file counts (same info as current static tree)
- **Expanding a dir:** shows its contents (files and subdirs) indented below
- **Opening a file:** launches `$EDITOR`, returns to tree after editor closes
- **Colors:** Catppuccin Mocha palette (same as rest of notez)

### Keybindings

| Key | Action |
|---|---|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `l` / `→` / `Enter` on dir | Expand / enter directory |
| `h` / `←` | Collapse / go up to parent |
| `Enter` / `o` on file | Open in `$EDITOR`, return to tree |
| `/` | Search/filter |
| `q` / `Esc` | Quit |
| `:wq` / `:qa` | Quit (vim style) |

### Dependencies
- `ratatui` — TUI framework
- `crossterm` — terminal backend for ratatui

---

## Feature 2: Interactive Todo (`notez todo`)

A quick todo system stored as markdown. Quick-add from the command line, or open an interactive TUI to view and manage todos.

### Commands
- `notez todo` — interactive TUI showing todos with check/uncheck
- `notez todo "fix the search"` — quick-add a todo (no TUI, appends and confirms)
- `notez todo -g` — global TODO.md

### Storage
File: `./notez/TODO.md` (local) or `~/notez/TODO.md` (global)

Format:
```markdown
# TODO

- [ ] fix the search
- [x] add tree view
- [ ] write documentation
```

Standard markdown checkboxes. Readable in any editor or renderer.

### Quick-Add Behavior
`notez todo "fix the search"` appends `- [ ] fix the search` to `TODO.md`, creates the file with a `# TODO` header if it doesn't exist. Prints confirmation. No TUI.

### TUI Behavior
- Shows all todos with checkboxes
- Checked items shown with strikethrough or dimmed color
- Navigate and toggle items
- Add, edit, delete inline
- Changes saved to TODO.md on quit

### Keybindings

| Key | Action |
|---|---|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `space` / `x` / `Enter` | Toggle check/uncheck |
| `a` | Add new todo (inline prompt) |
| `d` | Delete todo |
| `e` | Edit todo text |
| `q` / `Esc` | Save and quit |
| `:wq` / `:qa` | Save and quit (vim style) |

### Colors
- Unchecked: sapphire text
- Checked: overlay (dimmed) with strikethrough if terminal supports it
- Same Catppuccin Mocha palette

### Dependencies
Shares `ratatui` + `crossterm` with tree feature.

---

## Feature 3: Edit/Open Existing Note (`notez edit`)

Open an existing note quickly from the command line. Fuzzy-match by filename or browse with a picker.

### Commands
- `notez edit` — fzf picker of all `.md` files in local `./notez/`, open selected in `$EDITOR`
- `notez edit search-term` — fuzzy match filename, open directly if one match, picker if multiple
- `notez edit -g` — picks from global `~/notez/`

### Behavior
1. Scan the notez directory recursively for `.md` files
2. If no search term: launch fzf (or dialoguer fallback) with all files
3. If search term: filter files by fuzzy match on filename
   - One match → open directly
   - Multiple matches → show picker
   - No matches → error message
4. Open selected file in `$EDITOR`

### Dependencies
No new dependencies. Uses existing fzf/dialoguer infrastructure.

---

## Project Structure Changes

```
src/
  commands/
    tree.rs            # REWRITE: ratatui interactive tree
    todo.rs            # CREATE: todo TUI + quick-add
    edit.rs            # CREATE: edit/open existing notes
    mod.rs             # MODIFY: add todo, edit modules
  main.rs              # MODIFY: add Todo, Edit subcommands
  tui/                 # CREATE: shared TUI utilities
    mod.rs             # shared ratatui setup/teardown, vim command mode
    theme.rs           # Catppuccin Mocha ratatui styles
```

### Shared TUI Module (`src/tui/`)
Both tree and todo use ratatui. Extract shared concerns:
- Terminal setup/teardown (enter raw mode, alternate screen)
- Catppuccin Mocha color theme for ratatui widgets
- Vim command mode (`:wq`, `:qa` handling)
- `$EDITOR` launch + return pattern

### New Dependencies
```toml
ratatui = "0.29"
crossterm = "0.28"
```
