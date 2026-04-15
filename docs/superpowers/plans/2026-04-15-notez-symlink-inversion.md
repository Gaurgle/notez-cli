# Notez Symlink Inversion — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Invert the notez symlink direction so that `~/notez/` holds all private notes as real files, and each project's `.notez/` is a single whole-directory symlink back to the corresponding `~/notez/<NN>_<project>/`. Initialize `~/notez/` as a private GitHub repo so push/pull syncs notes content (not symlink strings) across machines.

**Architecture:** Single private git repo at `~/notez/` is source of truth for global + project-private notes. Each project has exactly one symlink (`<project>/.notez -> ~/notez/<NN>_<project>`). Public project notes (`<project>/notez/`) are unchanged and continue to sync via the project's own repo. The `notez-cli` Rust source is unchanged in this plan; writes through `.notez/` transparently land in `~/notez/` via the symlink.

**Tech Stack:** zsh, GNU coreutils + macOS BSD coreutils, git, gh CLI, `notez-cli` (Rust binary, used unmodified).

**Spec reference:** `docs/superpowers/specs/2026-04-15-notez-symlink-inversion-design.md`

---

## Phase 1 — Backup and repo setup

### Task 1: Backup current state

**Files:**
- Create: `~/notez.backup-pre-restructure/` (full copy of `~/notez/`)
- Create: `~/notez-project-snapshots-2026-04-15.tar.gz` (tarball of all project `.notez/` dirs)

- [ ] **Step 1: Snapshot `~/notez/`**

```bash
cp -RL ~/notez ~/notez.backup-pre-restructure
```

`-L` dereferences symlinks so the backup contains real content even where the current layout has per-file symlinks.

- [ ] **Step 2: Tar each project's `.notez/`**

```bash
cd ~ && tar -czf notez-project-snapshots-2026-04-15.tar.gz \
  Repos/Sigma/.notez 2>/dev/null \
  Repos/Sigma/app2/.notez 2>/dev/null \
  Repos/Sigma/app2/docs/.notez 2>/dev/null \
  Repos/notez-cli/.notez 2>/dev/null \
  Repos/repoz/.notez 2>/dev/null \
  Repos/J24-examen/.notez 2>/dev/null \
  Repos/noiz/.notez 2>/dev/null
```

- [ ] **Step 3: Verify both backups exist and are non-trivial**

```bash
du -sh ~/notez.backup-pre-restructure ~/notez-project-snapshots-2026-04-15.tar.gz
```

Expected: both have a non-zero size (each at least a few KB).

---

### Task 2: Add `.gitignore` to `~/notez/`

**Files:**
- Create: `~/notez/.gitignore`

- [ ] **Step 1: Write the gitignore**

```bash
cat > ~/notez/.gitignore <<'EOF'
.DS_Store
*.swp
*.swo
*.tmp
EOF
```

- [ ] **Step 2: Verify**

```bash
cat ~/notez/.gitignore
```

Expected: 4 lines printed.

---

### Task 3: Initialize `~/notez/` as a git repo and capture current state

**Files:**
- Modify: `~/notez/.git/` (created)

- [ ] **Step 1: Init repo**

```bash
cd ~/notez && git init -b main
```

Expected: `Initialized empty Git repository in /Users/at-a/notez/.git/`.

- [ ] **Step 2: Stage everything (real files + the cross-symlinks as they are now)**

```bash
cd ~/notez && git add -A
```

- [ ] **Step 3: Commit initial snapshot**

```bash
cd ~/notez && git commit -m "chore: initial import (pre-restructure snapshot)"
```

Expected: a single commit listing the imported tree. Note: cross-symlinks like `13_tobii-prep` will be stored as symlink strings in this commit — that's fine, we'll fix them in a later task.

---

### Task 4: Create the private GitHub repo and push

**Files:**
- Modify: GitHub account (creates `Gaurgle/notez` private repo)

- [ ] **Step 1: Create the private remote via `gh`**

```bash
gh repo create Gaurgle/notez --private --description "Personal notes — synced via notez-cli"
```

Expected: confirmation that `Gaurgle/notez` was created (no auto-push because we already have local commits).

- [ ] **Step 2: Add the remote**

```bash
cd ~/notez && git remote add origin git@github.com:Gaurgle/notez.git
```

- [ ] **Step 3: Push**

```bash
cd ~/notez && git push -u origin main
```

Expected: branch `main` set up to track `origin/main`.

- [ ] **Step 4: Verify on GitHub**

```bash
gh repo view Gaurgle/notez
```

Expected: shows the repo with at least one commit.

---

## Phase 2 — Per-project restructure

Each project below uses the same procedure: copy real content into `~/notez/<NN>_<project>/`, replace the project's `.notez/` with a single whole-directory symlink, verify, commit. The `cp -RL` flag dereferences any per-file symlinks so we end up with real files in `~/notez/`.

The order intentionally proceeds from least-risk (small / clean projects) to higher-risk (`app2` which has multiple `.notez/` candidates). Skip a task only if you're sure that project has no private notes worth preserving.

### Task 5: Audit which `~/notez/<NN>_*` dirs are real projects

**Files:** none modified — diagnostic only.

- [ ] **Step 1: Print contents and project mapping side-by-side**

```bash
for d in ~/notez/*/; do
  base=$(basename "$d")
  files=$(find "$d" -type f -o -type l | wc -l | tr -d ' ')
  echo "$base ($files entries)"
done
echo "---"
cat ~/.config/notez/projects
```

- [ ] **Step 2: For each `~/notez/<NN>_*` dir, decide and write down**

For each of `02_sigma`, `03_notez-cli`, `04_my-project`, `05_repoz`, `06_j24-examen`, `07_notes`, `08_jobbansokningar`, `09_repos`, `10_spotify`, `11_app2`, `12_noiz`, `13_tobii-prep`, decide one of:
  - **PROJECT** — has real project at `<path>`; will be restructured (Tasks 6-N)
  - **GLOBAL** — has notes but no real project repo; leave as a real dir under `~/notez/` (already correct, no action)
  - **DELETE** — empty or stale; remove

Write the decision into a scratch file: `~/notez-decisions.txt` with one line per dir, format `<dirname> <decision> [<project-path>]`. Refer back to it for subsequent tasks.

- [ ] **Step 3: Commit the decisions file** (kept out of the notez repo on purpose)

The file lives in `$HOME`, not `~/notez/`. No git action needed; it's a working note for the rest of this plan.

---

### Task 6: Restructure `02_sigma` (template task — full procedure shown)

**Files:**
- Read: `~/Repos/Sigma/.notez/` (any private notes there, may not exist)
- Modify: `~/notez/02_sigma/` (becomes a real dir with all content)
- Delete: `~/Repos/Sigma/.notez/` (replaced with symlink)
- Create: `~/Repos/Sigma/.notez` (whole-dir symlink)

- [ ] **Step 1: Inventory the source `.notez/` (if any)**

```bash
ls -la ~/Repos/Sigma/.notez 2>/dev/null
ls -la ~/notez/02_sigma/
```

If `~/Repos/Sigma/.notez/` does not exist, skip Step 2 — `~/notez/02_sigma/` is already the only source of truth.

- [ ] **Step 2: Merge any project-side content into `~/notez/02_sigma/`**

Only run if `~/Repos/Sigma/.notez/` exists with content not already in `~/notez/02_sigma/`:

```bash
cp -RLn ~/Repos/Sigma/.notez/. ~/notez/02_sigma/
```

`-n` means "no-clobber" — won't overwrite anything already in `~/notez/02_sigma/`. `-L` dereferences symlinks. After this, `~/notez/02_sigma/` holds all content from both sides.

- [ ] **Step 3: Verify content count makes sense**

```bash
find ~/notez/02_sigma -type f | wc -l
```

Expected: at least the count from before. Spot-check a couple of files: `cat ~/notez/02_sigma/00_quick-notez/<some-file>.md`.

- [ ] **Step 4: Remove the project-side `.notez/` (the per-file-symlinked or real version)**

```bash
rm -rf ~/Repos/Sigma/.notez
```

- [ ] **Step 5: Create the whole-dir symlink**

```bash
ln -s ~/notez/02_sigma ~/Repos/Sigma/.notez
```

- [ ] **Step 6: Verify symlink works in both directions**

```bash
ls -la ~/Repos/Sigma/.notez
ls ~/Repos/Sigma/.notez/    # should show same files as ~/notez/02_sigma
test -e ~/Repos/Sigma/.notez/00_quick-notez && echo "OK" || echo "MISSING"
```

Expected: `~/Repos/Sigma/.notez` shown as `lrwxr-xr-x ... -> /Users/at-a/notez/02_sigma`, and `OK` printed.

- [ ] **Step 7: Test write through the symlink**

```bash
echo "test write $(date)" > ~/Repos/Sigma/.notez/_restructure_test.md
ls ~/notez/02_sigma/_restructure_test.md && rm ~/notez/02_sigma/_restructure_test.md
```

Expected: the test file shows up in `~/notez/02_sigma/` (proves writes through the symlink land in the real location), then is cleaned up.

- [ ] **Step 8: Commit**

```bash
cd ~/notez && git add -A && git commit -m "feat(02_sigma): restructure as real dir; project .notez is now a symlink"
```

---

### Task 7: Restructure `03_notez-cli`

**Files:**
- Read: `~/Repos/notez-cli/.notez/`
- Modify: `~/notez/03_notez-cli/`
- Delete: `~/Repos/notez-cli/.notez/`
- Create: `~/Repos/notez-cli/.notez` (symlink)

- [ ] **Step 1: Inventory**

```bash
ls -la ~/Repos/notez-cli/.notez 2>/dev/null
ls -la ~/notez/03_notez-cli/
```

- [ ] **Step 2: Merge if needed**

```bash
[[ -e ~/Repos/notez-cli/.notez ]] && cp -RLn ~/Repos/notez-cli/.notez/. ~/notez/03_notez-cli/
```

- [ ] **Step 3: Verify**

```bash
find ~/notez/03_notez-cli -type f | wc -l
```

- [ ] **Step 4: Remove project-side dir**

```bash
rm -rf ~/Repos/notez-cli/.notez
```

- [ ] **Step 5: Create symlink**

```bash
ln -s ~/notez/03_notez-cli ~/Repos/notez-cli/.notez
```

- [ ] **Step 6: Verify**

```bash
ls -la ~/Repos/notez-cli/.notez
test -e ~/Repos/notez-cli/.notez/00_quick-notez && echo "OK" || echo "EMPTY (acceptable if dir had no quick-notes)"
```

- [ ] **Step 7: Test write**

```bash
echo "test write $(date)" > ~/Repos/notez-cli/.notez/_restructure_test.md
ls ~/notez/03_notez-cli/_restructure_test.md && rm ~/notez/03_notez-cli/_restructure_test.md
```

- [ ] **Step 8: Commit**

```bash
cd ~/notez && git add -A && git commit -m "feat(03_notez-cli): restructure as real dir; project .notez is now a symlink"
```

---

### Task 8: Delete stale `04_my-project`

`04_my-project` is the leftover from running `notez demo` once. Per `~/.config/notez/projects`, it points at `/private/var/folders/.../notez-demo/my-project` — a temp dir.

**Files:**
- Delete: `~/notez/04_my-project/`

- [ ] **Step 1: Confirm it's the demo leftover**

```bash
grep "my-project" ~/.config/notez/projects
ls ~/notez/04_my-project/
```

Expected: project mapping shows `/private/var/folders/...`, the dir contains demo content.

- [ ] **Step 2: Remove**

```bash
rm -rf ~/notez/04_my-project
```

- [ ] **Step 3: Commit**

```bash
cd ~/notez && git add -A && git commit -m "chore: remove stale 04_my-project (demo leftover)"
```

---

### Task 9: Restructure `05_repoz`

**Files:**
- Read: `~/Repos/repoz/.notez/`
- Modify: `~/notez/05_repoz/`
- Delete: `~/Repos/repoz/.notez/`
- Create: `~/Repos/repoz/.notez` (symlink)

- [ ] **Step 1: Inventory**

```bash
ls -la ~/Repos/repoz/.notez 2>/dev/null
ls -la ~/notez/05_repoz/
```

- [ ] **Step 2: Merge if needed**

```bash
[[ -e ~/Repos/repoz/.notez ]] && cp -RLn ~/Repos/repoz/.notez/. ~/notez/05_repoz/
```

- [ ] **Step 3: Verify**

```bash
find ~/notez/05_repoz -type f | wc -l
```

- [ ] **Step 4: Remove project-side dir**

```bash
rm -rf ~/Repos/repoz/.notez
```

- [ ] **Step 5: Create symlink**

```bash
ln -s ~/notez/05_repoz ~/Repos/repoz/.notez
```

- [ ] **Step 6: Verify**

```bash
ls -la ~/Repos/repoz/.notez
```

- [ ] **Step 7: Test write**

```bash
echo "test write $(date)" > ~/Repos/repoz/.notez/_restructure_test.md
ls ~/notez/05_repoz/_restructure_test.md && rm ~/notez/05_repoz/_restructure_test.md
```

- [ ] **Step 8: Commit**

```bash
cd ~/notez && git add -A && git commit -m "feat(05_repoz): restructure as real dir; project .notez is now a symlink"
```

---

### Task 10: Restructure `06_j24-examen`

**Files:**
- Read: `~/Repos/J24-examen/.notez/`
- Modify: `~/notez/06_j24-examen/`
- Delete: `~/Repos/J24-examen/.notez/`
- Create: `~/Repos/J24-examen/.notez` (symlink)

- [ ] **Step 1: Inventory**

```bash
ls -la ~/Repos/J24-examen/.notez 2>/dev/null
ls -la ~/notez/06_j24-examen/
```

- [ ] **Step 2: Merge if needed**

```bash
[[ -e ~/Repos/J24-examen/.notez ]] && cp -RLn ~/Repos/J24-examen/.notez/. ~/notez/06_j24-examen/
```

- [ ] **Step 3: Verify**

```bash
find ~/notez/06_j24-examen -type f | wc -l
```

- [ ] **Step 4: Remove project-side dir**

```bash
rm -rf ~/Repos/J24-examen/.notez
```

- [ ] **Step 5: Create symlink**

```bash
ln -s ~/notez/06_j24-examen ~/Repos/J24-examen/.notez
```

- [ ] **Step 6: Verify**

```bash
ls -la ~/Repos/J24-examen/.notez
```

- [ ] **Step 7: Test write**

```bash
echo "test write $(date)" > ~/Repos/J24-examen/.notez/_restructure_test.md
ls ~/notez/06_j24-examen/_restructure_test.md && rm ~/notez/06_j24-examen/_restructure_test.md
```

- [ ] **Step 8: Commit**

```bash
cd ~/notez && git add -A && git commit -m "feat(06_j24-examen): restructure as real dir; project .notez is now a symlink"
```

---

### Task 11: Decide and act on `07_notes`, `08_jobbansokningar`, `09_repos`, `10_spotify`

These dirs are not in `~/.config/notez/projects` as projects. They likely hold global-style notes that just live in `~/notez/<NN>_<topic>/`. Per the audit in Task 5, they should be marked **GLOBAL** (no action needed) or **DELETE** (if empty/stale).

**Files:** Possibly delete one or more of `~/notez/07_notes`, `~/notez/08_jobbansokningar`, `~/notez/09_repos`, `~/notez/10_spotify`.

- [ ] **Step 1: Cross-check against the decisions file**

```bash
grep -E "07_notes|08_jobbansokningar|09_repos|10_spotify" ~/notez-decisions.txt
```

- [ ] **Step 2: For each marked DELETE, remove**

```bash
# Example, run only for those marked DELETE in Step 1:
# rm -rf ~/notez/07_notes
# rm -rf ~/notez/09_repos
```

- [ ] **Step 3: Commit if anything changed**

```bash
cd ~/notez && git status
# If there are deletions to commit:
cd ~/notez && git add -A && git commit -m "chore: clean up non-project dirs in ~/notez (per Task 5 audit)"
```

---

### Task 12: Restructure `11_app2` (special case — three candidate `.notez/` locations)

`app2` has three plausible private-notes directories:
- `~/Repos/Sigma/app2/.notez/` (mtime 9 Apr)
- `~/Repos/Sigma/app2/docs/.notez/` (where `~/notez/11_app2/TODO.md` symlinks to today)
- `~/Repos/Sigma/app2/notez/` (this is the **public** one — leave alone)

We pick exactly one as canonical for the new `~/notez/11_app2/`.

**Files:**
- Read: `~/Repos/Sigma/app2/.notez/` and `~/Repos/Sigma/app2/docs/.notez/`
- Modify: `~/notez/11_app2/`
- Delete: `~/Repos/Sigma/app2/.notez/`, `~/Repos/Sigma/app2/docs/.notez/`
- Create: `~/Repos/Sigma/app2/.notez` (symlink)

- [ ] **Step 1: Compare the two private candidates**

```bash
echo "=== app2/.notez ==="
find ~/Repos/Sigma/app2/.notez -type f -o -type l 2>/dev/null
echo "=== app2/docs/.notez ==="
find ~/Repos/Sigma/app2/docs/.notez -type f -o -type l 2>/dev/null
echo "=== ~/notez/11_app2 ==="
find ~/notez/11_app2 -type f -o -type l 2>/dev/null
```

Read the file contents to figure out which dir holds the actual notes and which is empty/leftover.

- [ ] **Step 2: Merge BOTH candidates' real content into `~/notez/11_app2/`**

Use `-n` (no-clobber) so we don't lose anything; manual conflict resolution if filenames overlap.

```bash
cp -RLn ~/Repos/Sigma/app2/.notez/. ~/notez/11_app2/ 2>/dev/null
cp -RLn ~/Repos/Sigma/app2/docs/.notez/. ~/notez/11_app2/ 2>/dev/null
```

- [ ] **Step 3: Spot-check that nothing critical was overwritten or skipped**

```bash
ls -la ~/notez/11_app2/
cat ~/notez/11_app2/TODO.md 2>/dev/null
```

- [ ] **Step 4: Delete BOTH project-side `.notez/` directories**

```bash
rm -rf ~/Repos/Sigma/app2/.notez
rm -rf ~/Repos/Sigma/app2/docs/.notez
```

- [ ] **Step 5: Create the canonical symlink at the project root**

```bash
ln -s ~/notez/11_app2 ~/Repos/Sigma/app2/.notez
```

- [ ] **Step 6: Verify**

```bash
ls -la ~/Repos/Sigma/app2/.notez
ls ~/Repos/Sigma/app2/.notez/
```

- [ ] **Step 7: Test write**

```bash
echo "test write $(date)" > ~/Repos/Sigma/app2/.notez/_restructure_test.md
ls ~/notez/11_app2/_restructure_test.md && rm ~/notez/11_app2/_restructure_test.md
```

- [ ] **Step 8: Commit**

```bash
cd ~/notez && git add -A && git commit -m "feat(11_app2): restructure as real dir; consolidate two private .notez/ candidates"
```

---

### Task 13: Restructure `12_noiz`

**Files:**
- Read: `~/Repos/noiz/.notez/`
- Modify: `~/notez/12_noiz/`
- Delete: `~/Repos/noiz/.notez/`
- Create: `~/Repos/noiz/.notez` (symlink)

- [ ] **Step 1: Inventory**

```bash
ls -la ~/Repos/noiz/.notez 2>/dev/null
ls -la ~/notez/12_noiz/
```

- [ ] **Step 2: Merge if needed**

```bash
[[ -e ~/Repos/noiz/.notez ]] && cp -RLn ~/Repos/noiz/.notez/. ~/notez/12_noiz/
```

- [ ] **Step 3: Verify**

```bash
find ~/notez/12_noiz -type f | wc -l
```

- [ ] **Step 4: Remove project-side dir**

```bash
rm -rf ~/Repos/noiz/.notez
```

- [ ] **Step 5: Create symlink**

```bash
ln -s ~/notez/12_noiz ~/Repos/noiz/.notez
```

- [ ] **Step 6: Verify**

```bash
ls -la ~/Repos/noiz/.notez
```

- [ ] **Step 7: Test write**

```bash
echo "test write $(date)" > ~/Repos/noiz/.notez/_restructure_test.md
ls ~/notez/12_noiz/_restructure_test.md && rm ~/notez/12_noiz/_restructure_test.md
```

- [ ] **Step 8: Commit**

```bash
cd ~/notez && git add -A && git commit -m "feat(12_noiz): restructure as real dir; project .notez is now a symlink"
```

---

### Task 14: Resolve `13_tobii-prep` cross-symlink

`13_tobii-prep` is a symlink to `~/notes/tobii-prep` (the OLD notes repo). Two options per the spec:
- **Option A (recommended):** Copy content into `~/notez/13_tobii-prep/` as a real dir now. Sync travels with `notez` repo immediately.
- **Option B:** Defer to the future `~/notes/` migration phase. Leave the cross-symlink as-is; the `notez` repo stores it as a symlink string and it works only on this machine until migration.

**Files:**
- Modify: `~/notez/13_tobii-prep` (becomes real dir, was symlink)

- [ ] **Step 1: Choose A or B**

If A, proceed to Step 2. If B, skip the rest of this task.

- [ ] **Step 2: Replace symlink with real copy of content (Option A)**

```bash
rm ~/notez/13_tobii-prep
cp -RL ~/notes/tobii-prep ~/notez/13_tobii-prep
```

- [ ] **Step 3: Verify**

```bash
ls -la ~/notez/13_tobii-prep | head -5
test -d ~/notez/13_tobii-prep && echo "now a real dir" || echo "still a symlink"
```

- [ ] **Step 4: Commit (Option A only)**

```bash
cd ~/notez && git add -A && git commit -m "feat(13_tobii-prep): convert from cross-symlink to real dir"
```

---

## Phase 3 — Config cleanup and final verification

### Task 15: Clean up `~/.config/notez/projects`

**Files:**
- Modify: `~/.config/notez/projects`

- [ ] **Step 1: Show current content**

```bash
cat ~/.config/notez/projects
```

- [ ] **Step 2: Remove the demo leftover line**

```bash
sed -i '' '/^my-project=\/private\/var\/folders/d' ~/.config/notez/projects
```

- [ ] **Step 3: Fix the `app2` mapping (drop the `/docs` suffix)**

```bash
sed -i '' 's|^app2=/Users/at-a/Repos/Sigma/app2/docs$|app2=/Users/at-a/Repos/Sigma/app2|' ~/.config/notez/projects
```

- [ ] **Step 4: Verify**

```bash
cat ~/.config/notez/projects
```

Expected: no `my-project` line; `app2` line ends in `/app2` not `/app2/docs`.

---

### Task 16: Push everything to GitHub

**Files:** none locally — pushes accumulated commits to `origin/main`.

- [ ] **Step 1: Push all restructure commits**

```bash
cd ~/notez && git push
```

Expected: multiple commits pushed (one per restructured project plus cleanup commits).

- [ ] **Step 2: Verify on GitHub**

```bash
gh repo view Gaurgle/notez --json pushedAt,defaultBranchRef
```

Expected: recent `pushedAt`, branch `main` exists.

---

### Task 17: End-to-end smoke test (write through symlink → push → verify)

This proves the new layout actually works the way the design intends.

**Files:** temporary test note created and removed.

- [ ] **Step 1: From a project, create a private note via the CLI**

```bash
cd ~/Repos/Sigma/app2 && notez add restructure smoke test
```

The editor will open. Type one line ("smoke test 2026-04-15"), save and quit.

- [ ] **Step 2: Confirm the file landed in `~/notez/11_app2/`, not elsewhere**

```bash
find ~/notez/11_app2 -name "*restructure-smoke-test*"
```

Expected: a path under `~/notez/11_app2/00_quick-notez/`.

- [ ] **Step 3: Confirm `notez tree -g` shows it**

```bash
notez -g tree
```

Expected: navigate to `11_app2` and see the new note.

- [ ] **Step 4: Confirm git sees it as a new file in the `notez` repo**

```bash
cd ~/notez && git status
```

Expected: one new untracked file under `11_app2/00_quick-notez/`.

- [ ] **Step 5: Commit and push**

```bash
cd ~/notez && git add -A && git commit -m "test: end-to-end smoke test note (will remove)"
git push
```

- [ ] **Step 6: Remove the smoke test note locally and push**

```bash
rm ~/notez/11_app2/00_quick-notez/restructure-smoke-test.md
cd ~/notez && git add -A && git commit -m "test: remove smoke test note"
git push
```

---

### Task 18: Cross-machine verification (after first sync to work computer)

This is **deferred** — runs on the work computer next time you're there. Listed here so it isn't forgotten.

**On the work computer:**

- [ ] **Step 1: Clone the new repo into `~/notez/`**

```bash
cd ~ && git clone git@github.com:Gaurgle/notez.git notez
```

If `~/notez/` already exists on work, back it up first (`mv ~/notez ~/notez.work-old`) and reconcile any work-only content manually.

- [ ] **Step 2: For each project that exists on work too, restore the symlink**

```bash
# Example for sigma:
rm -rf ~/Repos/Sigma/.notez
ln -s ~/notez/02_sigma ~/Repos/Sigma/.notez
# repeat per project that exists on this machine
```

- [ ] **Step 3: Verify TUI**

```bash
notez -g tree
```

Expected: shows everything synced from home, plus any work-local-only notes that exist outside `~/notez/`.

---

## Self-review checklist

The plan covers each spec section:

- **Spec §Architecture / Target layout** → Tasks 6–13 (per-project restructure) + Tasks 2–4 (repo init).
- **Spec §Architecture / Invariants** → Verified in each project task's "Verify" steps.
- **Spec §Components A (new repo)** → Tasks 2–4.
- **Spec §Components B (per-project restructure)** → Tasks 5–14.
- **Spec §Components C (cleanups)** → Task 15 (config), Task 14 (cross-symlink), Task 8 (stale `04_my-project`), Task 11 (other non-project dirs). The "00_quick-notez vs 00_quick-notes" naming inconsistency is a doc-only fix and is intentionally deferred (mentioned in spec but not blocking sync); revisit when next touching the README.
- **Spec §Components D (notez-cli code)** → Deferred per spec; smoke test in Task 17 will surface any issue.
- **Spec §Execution order** → Phase 1 → Phase 2 → Phase 3 mirrors the spec's safe sequence.
- **Spec §Risks and edge cases** → Backup task (1) addresses recoverability; `cp -RL` used everywhere addresses the per-file-symlink dereferencing risk; smoke test (17) addresses the CLI mirror-symlink behavior risk.
- **Spec §Open decisions** → Task 12 step 1 prompts the canonical-`.notez` choice for `app2`; Task 14 step 1 prompts the `13_tobii-prep` decision.
