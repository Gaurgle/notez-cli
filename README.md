# notez

A CLI note-taking tool with numbered directories and quick capture.

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

A friendly step-by-step wizard that configures your notes folder, directory names, and detects available tools.

## Commands

### Notes

| Command | Description |
|---|---|
| `notez add [title]` | Create a note and open in editor |
| `notez add [title] "body text"` | Create a note with content (no editor) |
| `notez add [title] --in` | Create a note in a specific directory (fzf picker) |
| `notez add [title] --in dir` | Create a note in a named directory |

Multi-word titles work without quotes: `notez add my cool idea`

### Daily Logs

| Command | Description |
|---|---|
| `notez log <message>` | Append a timestamped entry to today's log |
| `notez logz` | Browse daily logs directory |
| `notez logs` | Alias for `logz` |

### Browsing & Search

| Command | Description |
|---|---|
| `notez` | Browse notes root in yazi |
| `notez search <term>` | Search notes with rg + fzf |
| `notez tree` | Show directory structure with note counts |

### Organization

| Command | Description |
|---|---|
| `notez mkdir <name>` | Create a new numbered subdirectory |
| `notez setup` | Run the setup wizard |

### Shortcuts

Built-in shortcut subcommands — same as the full versions:

| Command | Same as |
|---|---|
| `notez zlog <message>` | `notez log` |
| `notez zlogs` | `notez logz` |
| `notez znote [title]` | `notez add` |

For even shorter access, add shell aliases:

```bash
alias zlog='notez zlog'
alias zlogs='notez zlogs'
alias logz='notez logz'
alias znote='notez znote'
```

## Directory Structure

```
~/notez/
  00_quick-notes/           # Quick capture notes
  01_daily-logs/            # Daily log files
  02_project-ideas/         # User-created (notez mkdir)
    backend/                # Subdirs are shown in tree
    frontend/
  03_recipes/               # User-created (notez mkdir)
  random-stuff/             # Not managed by notez
```

Numbered directories (`00`-`99`) are managed by notez. Everything else is left untouched.

## Optional Tools

| Tool | Used for | Fallback |
|---|---|---|
| `yazi` | Browsing notes | Opens in `$EDITOR` |
| `fzf` | Directory picker, search UI | Numbered list selection |
| `rg` | Fast content search | `grep -r` |

## Config

Stored at `~/.config/notez/config`. Re-run `notez setup` to reconfigure.
