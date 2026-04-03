# notez

![License](https://img.shields.io/github/license/Gaurgle/notez-cli)
![Rust](https://img.shields.io/badge/rust-stable-dea584)
![Optional](https://img.shields.io/badge/optional-yazi%20·%20fzf%20·%20rg-blue)

A local-first CLI note-taking tool. Notes live with your projects, mirrored to a home directory for a unified view.

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

**Local-first:** All commands default to `./notez/` in your current directory. Your notes live alongside your project.

**Home mirror:** Every local note is symlinked into `~/notez/` under a project directory (auto-created, named after your git repo). Browse all your notes from one place.

**Global mode:** Use `-g` before the subcommand to target `~/notez/` directly for notes that aren't tied to a project.

```
~/Repos/my-project/
  notez/                            ← same layout as global
    00_quick-notes/                 ← notez add
      2026-04-02-api-design.md
    01_daily-logs/                  ← notez log
      2026-04-02-daily-log.md
    02_research/                    ← notez mkdir
    TODO.md                         ← notez todo

~/notez/                            ← unified home view
  00_quick-notes/                   ← global quick notes (-g)
  01_daily-logs/                    ← global daily logs (-g)
  02_my-project/                    ← symlinks to project dirs
    00_quick-notes/               → ~/Repos/my-project/notez/00_quick-notes/
    01_daily-logs/                → ~/Repos/my-project/notez/01_daily-logs/
    02_research/                  → ~/Repos/my-project/notez/02_research/
    TODO.md                       → ~/Repos/my-project/notez/TODO.md
  TODO.md                           ← global todos (-g)
```

## Commands

All commands default to the local `./notez/` directory. Use `-g` before the subcommand for global `~/notez/`.

### Notes

| Command | Description |
|---|---|
| `notez add [title]` | Create a note and open in editor |
| `notez add [title] "body text"` | Create a note with content (no editor) |
| `notez add [title] --in` | Create a note in a subdirectory (fzf picker) |
| `notez add [title] --in dir` | Create a note in a named subdirectory |
| `notez edit [term]` | Open an existing note (fuzzy search / fzf picker) |

Multi-word titles work without quotes: `notez add my cool idea`

### Daily Logs

| Command | Description |
|---|---|
| `notez log <message>` | Append a timestamped entry to today's log |
| `notez logz` / `notez logs` | Browse daily logs |

### Todos

| Command | Description |
|---|---|
| `notez todo` | Interactive todo manager |
| `notez todo "item"` | Quick-add a todo item |
| `notez todoz` | Alias for `notez todo` |
| `notez -g todo` | View all todos across every project, grouped |

**Todo keybindings:**

| Key | Action |
|---|---|
| `x` / `space` / `Enter` | Check/uncheck (on parent: toggles all subtasks) |
| `a` / `/` | Almost done `[/]` |
| `n` | New todo |
| `s` | Add subtask |
| `e` | Edit text |
| `d` | Delete (y/n confirm) |
| `h` / `l` | Collapse / expand subtasks |
| `j` / `k` | Navigate |
| `q` / `Esc` / `Ctrl+C` / `:wq` | Quit |

**Todo states:** `[ ]` unchecked → `[/]` almost done → `[x]` checked

**Subtasks:** One level deep. Parent state auto-derived from subtask completion.

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

### Global Mode

Use `-g` **before** the subcommand:

```bash
notez -g add personal thought       # → ~/notez/00_quick-notes/
notez -g log "reminder for later"   # → ~/notez/01_daily-logs/
notez -g tree                       # shows all projects
notez -g todo                       # todos from every project
```

### Shortcuts

Built-in shortcut subcommands:

| Command | Same as |
|---|---|
| `notez zlog <message>` | `notez log` |
| `notez zlogs` | `notez logz` |
| `notez znote [title]` | `notez add` |
| `notez todoz` | `notez todo` |

For standalone access, add shell aliases (`noglob` prevents zsh glob expansion on `?` `*` etc.):

```bash
alias zlog='noglob notez zlog'
alias zlogs='notez zlogs'
alias logz='notez logz'
alias znote='noglob notez znote'
alias todoz='notez todoz'
```

### Setup & Config

| Command | Description |
|---|---|
| `notez setup` | Interactive setup wizard |
| `notez completions <shell>` | Generate shell completions (zsh, bash, fish) |
| `notez -h` | Styled help with keybinding reference |

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
