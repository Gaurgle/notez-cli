# notez

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

**Global mode:** Use `-g` to target `~/notez/` directly for notes that aren't tied to a project.

```
~/Repos/my-project/
  notez/                        ← your notes live here
    2026-04-02-api-design.md
    2026-04-02-daily-log.md

~/notez/                        ← unified home view
  00_quick-notes/               ← global quick notes (-g)
  01_daily-logs/                ← global daily logs (-g)
  02_my-project/                ← symlinks to project notes
    2026-04-02-api-design.md  → ~/Repos/my-project/notez/...
    2026-04-02-daily-log.md   → ~/Repos/my-project/notez/...
```

## Commands

All commands default to the local `./notez/` directory. Add `-g` for global `~/notez/`.

### Notes

| Command | Description |
|---|---|
| `notez add [title]` | Create a note and open in editor |
| `notez add [title] "body text"` | Create a note with content (no editor) |
| `notez add [title] --in` | Create a note in a specific subdirectory (fzf picker) |
| `notez add [title] --in dir` | Create a note in a named subdirectory |

Multi-word titles work without quotes: `notez add my cool idea`

### Daily Logs

| Command | Description |
|---|---|
| `notez log <message>` | Append a timestamped entry to today's log |
| `notez logz` | Browse daily logs |
| `notez logs` | Alias for `logz` |

### Browsing & Search

| Command | Description |
|---|---|
| `notez` | Browse local notes in yazi |
| `notez search <term>` | Search notes with rg + fzf |
| `notez tree` | Show directory structure with note counts |

### Organization

| Command | Description |
|---|---|
| `notez mkdir <name>` | Create a new numbered subdirectory |
| `notez setup` | Run the setup wizard |
| `notez completions <shell>` | Generate shell completions (zsh, bash, fish) |

### Global Mode

Add `-g` before or after any command to target `~/notez/` instead of `./notez/`:

```bash
notez -g add personal thought       # → ~/notez/00_quick-notes/
notez -g log "reminder for later"   # → ~/notez/01_daily-logs/
notez -g tree                       # shows all projects
```

### Shortcuts

Built-in shortcut subcommands:

| Command | Same as |
|---|---|
| `notez zlog <message>` | `notez log` |
| `notez zlogs` | `notez logz` |
| `notez znote [title]` | `notez add` |

For even shorter access, add shell aliases (`noglob` prevents zsh from treating `?` `*` etc. as wildcards):

```bash
alias zlog='noglob notez zlog'
alias zlogs='notez zlogs'
alias logz='notez logz'
alias znote='noglob notez znote'
```

## Tab Completions

Generate and install zsh completions:

```bash
mkdir -p ~/.zfunc
notez completions zsh > ~/.zfunc/_notez
```

Add to your `.zshrc` (if not already there):
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
