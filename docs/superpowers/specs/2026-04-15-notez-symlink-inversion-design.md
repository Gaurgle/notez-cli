# Notez Symlink Inversion — Design

**Date:** 2026-04-15
**Status:** Approved (design phase complete; implementation plan to follow)

## Problem

The current `notez-cli` data model treats the project's `.notez/` directory as the source of truth for project-private notes, and creates **per-file symlinks** in `~/notez/<NN>_<project>/` for the unified home view. This produces three real problems:

1. **Symlink drift.** Any file created in `.notez/` outside the CLI (e.g., via the editor, file manager, or `mv`) does not get a corresponding mirror symlink in `~/notez/`. Over time, the unified view diverges from the project state.
2. **Cross-machine sync is impossible as designed.** The README says to "push `~/notez/` as a private repo to sync notes across machines." But because git stores symlinks as symlink strings (not as the content they point to), pushing `~/notez/` only carries the per-file symlink names — not the underlying note content. On a second machine, the symlinks resolve to paths that don't exist.
3. **Multiple plausible sources of truth per project.** For at least one project (`app2`), three `.notez/`-shaped directories coexist (`app2/.notez/`, `app2/notez/`, `app2/docs/.notez/`). The CLI sees only the one its config points at. Drift becomes silent data loss.

## Goals

- A single, working source of truth for **all** private notes (global and project-scoped) at `~/notez/`.
- Cross-machine sync that actually carries content, achieved by pushing `~/notez/` as a single private git repo (`Gaurgle/notez`).
- The existing `notez add`, `notez tree`, `todoz`, `notez log` commands continue to work without code changes (CLI patches deferred until proven necessary).
- The public/private/global scope model (`-p`, `-g`, default) is preserved exactly as today.

## Non-goals

- Migrating content from `~/notes/` (the older repo) into `~/notez/`. Handled in a future phase once the new layout is solid.
- Removing `~/notes/`. It stays as an archive for now.
- Patching `notez-cli` Rust source. Only undertaken if the current mirror-symlink step produces noisy or broken behavior after restructure.
- Adding new commands or features.
- Changing the public/private/global model itself.

## Architecture

### Target layout

```
~/notez/                              ← NEW git repo, single source of truth
├── .git/
├── .gitignore                        ← .DS_Store, *.swp, etc.
├── 00_quick-notez/                   ← global private notes (real dir)
├── 01_daily-logs/                    ← global daily logs (real dir)
├── 02_sigma/                         ← project-private notes (real dir)
├── 03_notez-cli/                     ← project-private notes (real dir)
├── ...
├── 11_app2/                          ← real dir
├── 12_noiz/                          ← real dir
└── TODO.md                           ← global todos (real file)

~/Repos/Sigma/app2/
├── .notez -> ~/notez/11_app2         ← whole-dir SYMLINK back into the repo
├── notez/                            ← public, lives with project (UNCHANGED)
└── ... (project source)
```

### Invariants

- Every project that has private notes has **exactly one symlink**: `<project>/.notez -> ~/notez/<NN>_<project>`. No per-file symlinks anywhere.
- The source of truth for any private note (global or project-scoped) is **always** the file under `~/notez/`.
- Public project notes (`<project>/notez/`, no leading dot) remain real files in the project's own repo, committed and pushed with the project as today. They do **not** live in `~/notez/`.
- `notez add`, `notez tree`, `todoz`, and `notez log` keep working unmodified — their writes through `.notez/` transparently land inside `~/notez/<project>/` via the symlink.

### Three scopes (unchanged from today)

| Scope   | Flag       | Lives in                         | Synced via       |
|---------|------------|----------------------------------|------------------|
| Private | _(default)_ | `<project>/.notez/` (→ symlink) | `notez` repo     |
| Public  | `-p`       | `<project>/notez/`              | project's repo   |
| Global  | `-g`       | `~/notez/` (top level)          | `notez` repo     |

All three remain visible together in `notez tree -g` and `todoz -g`.

## Components and changes

### A. New private GitHub repo

- Create `Gaurgle/notez` (private).
- `cd ~/notez && git init && git add . && git commit -m "initial import" && git push -u origin main`.
- Add `.gitignore`: `.DS_Store`, `*.swp`, `*.swo`, OS junk.

### B. Per-project restructure (one-time, repeated for each project)

For each project currently in `~/notez/` (`02_sigma` … `12_noiz`):

1. Identify the **canonical** source `.notez/` directory for that project. For most projects this is `<project>/.notez/`. For `app2`, choose between three candidates (see Open Decisions below).
2. Copy real content (dereferencing any per-file symlinks) into `~/notez/<NN>_<project>/`:
   `cp -RL <project>/.notez/* ~/notez/<NN>_<project>/` (or equivalent).
3. After verifying the copy, delete the project's existing `.notez/`.
4. Create the whole-directory symlink: `ln -s ~/notez/<NN>_<project> <project>/.notez`.
5. Verify both paths show identical contents and that `notez tree -g` still includes the project.
6. Commit + push from `~/notez/`.

### C. Cleanups

- `~/.config/notez/projects` — remove stale `my-project=/private/var/folders/.../notez-demo/...`.
- `~/.config/notez/projects` — fix `app2=/Users/at-a/Repos/Sigma/app2/docs` to `app2=/Users/at-a/Repos/Sigma/app2`.
- `~/notez/13_tobii-prep` — currently a cross-symlink to `~/notes/tobii-prep`. Either copy the content into `~/notez/13_tobii-prep/` as a real dir and delete the symlink, or defer to the `~/notes/` migration phase.
- `~/notez/00_quick-notez` (`Z`) is canonical (matches `~/.config/notez/config`). Update README and any other docs that say `00_quick-notes` (`S`).
- Move or `.gitignore` loose files in `~/notez/`: `.DS_Store`, `spotify-offert.csv`.

### D. notez-cli code patch (`tree.rs`)

`src/commands/tree.rs:70-74` previously detected private vs. public by checking whether the canonical path of a depth-0 node contains `/.notez/`. That heuristic worked under the old layout (where `~/notez/<proj>/` was full of per-file symlinks resolving into `<project>/.notez/`), but under the inverted layout the canonical path is just `~/notez/<NN>_<project>` and the substring is absent — so every project was misclassified as public.

Fix: since the global tree is built by walking `notez_root` (which IS the private store under this design), every depth-0 dir in that walk is private by definition. The existing second loop in the same function (lines 76–128) already adds public entries from `~/.config/notez/projects` as separate sibling nodes, with the public icon set explicitly, so the categorization remains correct.

The patch is one line:
```rust
// Before:
node.scope_icon = if resolved.contains("/.notez") { project::ICON_PRIVATE } else { project::ICON_PUBLIC };
// After:
node.scope_icon = project::ICON_PRIVATE;
```

`todoz -g` (`src/commands/todo.rs:442-484`) already assumed everything in `~/notez/` is private and added public entries from project mappings separately, so it required no change.

### E. `_public` symlinks not needed

An earlier draft of this spec called for an explicit `~/notez/<NN>_<project>/_public -> <project>/notez/` symlink to keep public notes visible in the global tree. That turned out to be redundant: the existing second loop in `tree.rs` already scans project mappings for public `notez/` dirs and adds them as siblings. Adding `_public` symlinks would either cause public content to render twice (once nested under the private project, once as a separate sibling) or — depending on the second loop's "already_public" check — silently suppress the cleaner sibling rendering. The recommended layout has **no `_public` symlinks**.

## Execution order (safe sequence)

1. **Backup.** `cp -R ~/notez ~/notez.backup-pre-restructure`. Tar each project's `.notez/`.
2. **Init `notez` repo.** `git init`, `.gitignore`, first commit (capture current state, even with cross-symlinks; cleaned up in subsequent commits).
3. **Push to GitHub.** Create `Gaurgle/notez` private repo, `git push -u origin main`.
4. **Per-project restructure**, one project at a time, in numbered order. Commit + push after each. If anything breaks, only that project is affected; backup is recoverable.
5. **Config cleanups** in `~/.config/notez/`.
6. **Cross-symlink decisions** (`13_tobii-prep`).
7. **End-to-end test.** `notez add foo` from a project, verify file lands at `~/notez/<NN>_<project>/00_quick-notez/foo.md`, verify visible in `notez tree -g`, push, pull on the other machine.

## Risks and edge cases

- **CLI mirror-symlink behavior post-restructure.** If `notez add` produces noisy stderr or broken symlinks, escalate to the deferred Rust patch. Worst-case impact: stderr noise; the file itself still lands correctly because the CLI writes through `.notez/` first.
- **Symlink dereferencing on copy.** Use `cp -RL` (or equivalent) when copying out of any project that already has per-file symlinks, to ensure real files (not dangling symlinks) end up in `~/notez/`.
- **Project `.gitignore`.** Existing `.gitignore` already excludes `.notez/` (per the README design), so the new whole-directory symlink is also excluded — no project repo pollution.
- **macOS Finder + symlinks.** Following symlinks works as expected; no special handling needed.
- **Editor-created files.** With the inversion, files created via `nvim`, `code`, or any other editor inside `<project>/.notez/` automatically land in `~/notez/<project>/` via the symlink — no CLI involvement required. This eliminates the original drift bug.

## Open decisions (require user input during execution)

- **Canonical `.notez/` for `app2`.** Three candidates exist:
  - `~/Repos/Sigma/app2/.notez/`
  - `~/Repos/Sigma/app2/docs/.notez/` (where `~/notez/11_app2/TODO.md` symlinks to today)
  - `~/Repos/Sigma/app2/notez/` (this is the public one, not a candidate for private)

  User must inspect contents and pick one.
- **`13_tobii-prep` content.** Copy into the new `notez` repo now, or leave the cross-symlink to `~/notes/tobii-prep` until the `~/notes/` migration phase.
