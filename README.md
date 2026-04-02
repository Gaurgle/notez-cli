# notez

A CLI note-taking tool with numbered directories and quick capture.

## Install

```bash
git clone https://github.com/Gaurgle/notez.git
cd notez
./install.sh
```

Requires: Rust toolchain (`cargo`). Optional: `yazi`, `fzf`, `rg` (detected during setup, fallbacks built in).

## Setup

```bash
notez setup
```

Interactive wizard configures your notes root, directory names, detects tools, and offers shell aliases.

## Commands

| Command | Description |
|---|---|
| `notez` | Browse notes root in yazi |
| `notez add [title]` | Create a quick note |
| `notez add [title] --in` | Create a note in a specific directory (fzf picker) |
| `notez add [title] --in dir` | Create a note in a named directory |
| `notez log <message>` | Append to today's daily log |
| `notez logz` | Browse daily logs directory |
| `notez logs` | Alias for `logz` |
| `notez mkdir <name>` | Create a new numbered subdirectory |
| `notez search <term>` | Search notes with rg + fzf |
| `notez tree` | Show directory structure |
| `notez setup` | Run setup wizard |

## Directory Structure

```
~/notes/
  00_quick-notes/
  01_daily-logs/
  02_project-ideas/
  03_recipes/
  random-stuff/        (not managed by notez)
```

Numbered directories (00-99) are managed by notez. Everything else is left untouched.

## Shell Aliases

Offered during setup:

| Alias | Command |
|---|---|
| `zlog` | `notez log` |
| `zlogs` | `notez logz` |
| `logz` | `notez logz` |
| `znote` | `notez add` |

## Config

Stored at `~/.config/notez/config`. Re-run `notez setup` to reconfigure.
