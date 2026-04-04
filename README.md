# notez

![License](https://img.shields.io/github/license/Gaurgle/notez-cli)
![Rust](https://img.shields.io/badge/rust-stable-dea584)
![Optional](https://img.shields.io/badge/optional-yazi%20·%20fzf%20·%20rg-blue)

A local-first CLI note-taking tool. Notes live with your projects, mirrored to a home directory for a unified view.

![notez tree — local project view](pictures/notez-local.png)

## Install

```bash
git clone https://github.com/Gaurgle/notez-cli.git
cd notez-cli
./install.sh
```

Requires the Rust toolchain (`cargo`). Optional tools detected during setup: `yazi`, `fzf`, `rg` — built-in fallbacks if missing.

## Setup

```bash
notez setup
```

A friendly step-by-step wizard that configures your home notes folder, directory names, and detects available tools.

## How It Works

**Private by default:** All commands write to `.notez/` (hidden, auto-gitignored). Your notes stay private and never get committed to the project repo.

**Public with `-p`:** Add `-p` to any command to write to `notez/` instead — these notes travel with the project and can be committed.

**Home mirror:** Private notes are symlinked into `~/notez/` under a project directory (auto-created, named after your git repo). Push `~/notez/` as a private repo to sync notes across machines.

**Global mode (`-g`):** Target `~/notez/` directly for notes that aren't tied to a project.

```
~/Repos/my-project/
  .notez/                           ← private (default, gitignored)
    00_quick-notes/
    01_daily-logs/
    TODO.md
  notez/                            ← public (-p flag, committed)
    00_quick-notes/
    TODO.md

~/notez/                            ← unified home view
  00_quick-notes/                   ← global quick notes (-g)
  01_daily-logs/                    ← global daily logs (-g)
  02_my-project/                    ← symlinks from .notez/ (private)
  TODO.md                           ← global todos (-g)
```

## Commands

All commands default to private `.notez/`. Add `-p` for public `notez/`, or `-g` for global `~/notez/`.

### Notes

| Command | Description |
|---|---|
| `notez add [title]` | Create a private note, open in editor |
| `notez add -p [title]` | Create a public note |
| `notez add [title] "body text"` | Create a note with content (no editor) |
| `notez add [title] --in` | Create a note in a subdirectory (fzf picker) |
| `notez edit [term]` | Open an existing note (fuzzy search / fzf picker) |

Multi-word titles work without quotes: `notez add my cool idea`

### Daily Logs

| Command | Description |
|---|---|
| `notez log <message>` | Append a timestamped entry to today's log |
| `notez logz` / `notez logs` | Browse daily logs |

### Browse & Organize

| Command | Description |
|---|---|
| `notez` | Browse notes in yazi |
| `notez tree` | Interactive tree navigator |
| `notez search <term>` | Search note content (rg + fzf) |
| `notez mkdir <name>` | Create a numbered subdirectory |

**Tree keybindings:**

| Key | Action |
|---|---|
| `j` / `k` | Navigate |
| `l` / `→` | Expand directory |
| `h` / `←` | Collapse / go to parent |
| `o` / `Enter` | Open file in editor |
| `q` / `Esc` / `Ctrl+C` / `:wq` | Quit |

![notez tree — global view across all projects](pictures/notez-global.png)

### Todos

| Command | Description |
|---|---|
| `todoz` | Interactive todo manager (private) |
| `todoz -p` | Interactive todo manager (public) |
| `todoz "item"` | Quick-add a private todo |
| `todoz -g` | All todos across every project (private + public) |

![todoz — local project todos with subtasks](pictures/todoz-local.png)

**Todo keybindings:**

| Key | Action |
|---|---|
| `x` / `space` / `Enter` | Check/uncheck (on parent: toggles all subtasks) |
| `a` / `/` | Almost done `[/]` |
| `n` | New todo |
| `s` | Add subtask |
| `e` | Edit text |
| `d` | Delete (y/n confirm) |
| `h` / `l` | Collapse / expand (subtasks + project sections) |
| `v` | Toggle view all / collapse all |
| `j` / `k` / mouse scroll | Navigate |
| `q` / `Esc` / `Ctrl+C` / `:wq` | Quit |

**Todo states:** `[ ]` unchecked → `[/]` almost done → `[x]` checked

**Subtasks:** One level deep. Parent state auto-derived from subtask completion.

**Code TODOs:** Automatically scans project source for `// TODO`, `# TODO`, `-- TODO`, `/* TODO`, `<!-- TODO` comments. Displayed as a read-only section with file path and line number — visible alongside your todos but non-interactive.

**Global view (`todoz -g`):** Projects start collapsed — expand with `l`. Shows scroll position when the list exceeds the viewport. Aggregates private + public todos from all projects, each with a lock/globe indicator.

![todoz -g — global view with all projects and subtasks](pictures/todoz-global.png)

### Scope Flags

| Flag | Scope | Directory |
|---|---|---|
| _(default)_ | Private | `.notez/` (gitignored) |
| `-p` | Public | `notez/` (committed with project) |
| `-g` | Global | `~/notez/` (all projects) |

```bash
notez add my idea                   # → .notez/  (private)
notez add -p shared docs           # → notez/   (public)
notez -g add personal thought      # → ~/notez/ (global)
todoz -g                           # all todos, private + public
```

### Standalone Commands

Installed automatically as symlinks — no aliases needed:

| Command | Same as |
|---|---|
| `todoz` | `notez todo` |
| `todoz -g` | `notez -g todo` |
| `zlog <message>` | `notez log` |
| `zlogs` | `notez logz` |
| `logz` | `notez logz` |
| `znote [title]` | `notez add` |

**Zsh users:** if you use `?` or `*` in messages, add these to `.zshrc` to prevent glob expansion:

```bash
alias zlog='noglob zlog'
alias znote='noglob znote'
```

### Setup & Config

| Command | Description |
|---|---|
| `notez setup` | Interactive setup wizard |
| `notez completions <shell>` | Generate shell completions (zsh, bash, fish) |
| `notez -h` | Styled help with keybinding reference |
| `notez demo` | Create demo project for screenshots |

## Tab Completions

```bash
mkdir -p ~/.zfunc
notez completions zsh > ~/.zfunc/_notez
```

Add to `.zshrc`:
```bash
fpath=(~/.zfunc $fpath)
autoload -Uz compinit && compinit
```

## Optional Tools

| Tool | Used for | Fallback |
|---|---|---|
| `yazi` | Browsing notes | Opens in `$EDITOR` |
| `fzf` | Directory picker, search UI | Numbered list selection |
| `rg` | Fast content search | `grep -r` |

## Config

Stored at `~/.config/notez/config`. Project mappings at `~/.config/notez/projects`. Re-run `notez setup` to reconfigure.

## Also

Check out [repoz](https://github.com/Gaurgle/repoz) — see which repos need pulling, pushing, or have uncommitted work. One command.
