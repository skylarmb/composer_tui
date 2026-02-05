# composer_tui - Next Development Phases

> **Note:** This document describes phases 6-12, continuing from the completed MVP (phases 1-5). See `IMPLEMENTATION_PLAN.md` for the original phases.

## Summary

Three features to implement, building on the completed MVP:
1. **State Persistence** - Save/load workspace state across sessions
2. **Git Worktree** - Create/delete workspaces backed by git worktrees
3. **Embedded Terminal** - Real terminal emulation in main panel

**Implementation Order:** Persistence → Worktree → Terminal (each depends on the prior)

---

## Phase 6: State Persistence - Foundation

### Goal
Establish config directory structure and basic save/load infrastructure.

### Tasks
1. Add dependencies: `directories`, `serde`, `toml`
2. Create `src/config.rs`:
   - `Config` struct (serde-serializable)
   - `config_dir()` → `~/.config/composer_tui/`
   - `Config::load()` / `Config::save()`
3. Initial config field: `worktree_base_dir: Option<PathBuf>`
4. Handle missing/corrupt config (use defaults, don't crash)

### Success Criteria
- [ ] Config dir created at `~/.config/composer_tui/` on first run
- [ ] Missing config returns defaults
- [ ] Corrupt TOML logs warning, returns defaults
- [ ] `cargo test` + `cargo clippy` pass

### Handoff
Write `.context/handoff-phase-6.md`

---

## Phase 7: State Persistence - Workspace State

### Goal
Persist workspace list and selection across sessions.

### Tasks
1. Create `src/state.rs` with `AppState` struct:
   - `workspaces: Vec<WorkspaceState>` (id, name, worktree_path)
   - `selected_index: usize`
2. `AppState::load()` / `AppState::save()`
3. Load state in `App::new()`, save on quit
4. Handle format migration

### Success Criteria
- [ ] State saved to `~/.config/composer_tui/state.toml` on quit
- [ ] Workspace list + selection preserved across restarts
- [ ] Human-readable TOML format

### Handoff
Write `.context/handoff-phase-7.md`

---

## Phase 8: Git Worktree - Core Infrastructure

### Goal
Add git worktree creation capability with `git2` crate.

### Tasks
1. Add dependency: `git2`
2. Create `src/worktree.rs`:
   - `WorktreeManager::new(repo_path)`
   - `WorktreeManager::create_worktree(name, branch)`
   - `WorktreeManager::delete_worktree(name)`
   - `WorktreeManager::list_worktrees()`
3. Worktree location: `{worktree_base_dir}/{workspace_name}`
4. Error handling: not a git repo, name exists, branch conflicts

### Success Criteria
- [ ] Can create worktree from existing or new branch
- [ ] Error returned if not in git repo or name exists
- [ ] Worktree created in configured base dir

### Handoff
Write `.context/handoff-phase-8.md`

---

## Phase 9: Git Worktree - UI Integration

### Goal
Wire keybindings to create/delete workspaces.

### Tasks
1. Extend `Workspace`: `worktree_path`, `branch_name`
2. `n` key → create new workspace:
   - Text input prompt for name
   - Create git worktree
   - Add to list, save state
3. `d` key → delete workspace:
   - Confirmation prompt
   - Delete worktree + remove from list
4. Simple input mode (modal state in App)
5. Show branch name in sidebar

### Success Criteria
- [ ] `n` opens name input, creates worktree on confirm
- [ ] `d` with confirmation deletes worktree
- [ ] Worktree dirs actually created/removed on disk
- [ ] Graceful error handling (show error, don't crash)

### Handoff
Write `.context/handoff-phase-9.md`

---

## Phase 10: Embedded Terminal - PTY Infrastructure

### Goal
Set up PTY allocation and basic I/O.

### Tasks
1. Add dependency: `portable-pty`
2. Create `src/terminal.rs`:
   - `Terminal::spawn(cwd, shell)` - allocate PTY, spawn shell
   - `Terminal::read()` - non-blocking output read
   - `Terminal::write(data)` - send input
   - `Terminal::resize(cols, rows)`
   - `Terminal::kill()` + `Drop` cleanup

### Success Criteria
- [ ] Can spawn shell attached to PTY
- [ ] Can read output, write commands
- [ ] PTY resize works
- [ ] No zombie processes on drop

### Handoff
Write `.context/handoff-phase-10.md`

---

## Phase 11: Embedded Terminal - Screen Buffer

### Goal
Parse escape sequences and maintain screen buffer.

### Tasks
1. Add dependency: `vte` (escape sequence parser)
2. Create `src/terminal/screen.rs`:
   - `ScreenBuffer` - 2D grid of `Cell` (char, colors, attrs)
   - `ScreenBuffer::write(data)` - parse and apply escapes
3. Support: cursor movement, clear screen/line, colors, scrolling

### Success Criteria
- [ ] Buffer stores multi-line content
- [ ] ANSI colors parsed (16+ color)
- [ ] Cursor movement works
- [ ] `ls --color` renders correctly

### Handoff
Write `.context/handoff-phase-11.md`

---

## Phase 12: Embedded Terminal - UI Integration

### Goal
Render terminal in main panel and handle input when focused.

### Tasks
1. Workspace owns `Terminal` (lazy init)
2. `main_panel.rs` renders `ScreenBuffer` with colors
3. Event loop: forward keys to PTY when main panel focused
4. Special keys: Ctrl+C → SIGINT, Ctrl+O → escape to sidebar
5. Terminal started with worktree path as cwd
6. Handle terminal exit gracefully

### Input Mode Design
```
FocusArea::Sidebar → j/k navigate, Enter focuses main
FocusArea::Main    → All input to PTY, Ctrl+O escapes
FocusArea::Header  → Reserved for future command palette
```

### Success Criteria
- [ ] Selecting workspace shows live terminal
- [ ] Can type commands, see colored output
- [ ] Ctrl+C sends interrupt
- [ ] Ctrl+O escapes back to sidebar
- [ ] Each workspace has independent terminal
- [ ] Terminal uses worktree cwd

### Handoff
Write `.context/handoff-phase-12.md`

---

## New Dependencies

```toml
# Persistence
directories = "5"
serde = { version = "1", features = ["derive"] }
toml = "0.8"

# Git
git2 = "0.19"

# Terminal
portable-pty = "0.9"
vte = "0.13"
```

---

## New/Modified Files

### New Modules
```
src/
├── config.rs            # Configuration management
├── state.rs             # Persistent state
├── worktree.rs          # Git worktree operations
└── terminal/
    ├── mod.rs           # PTY management
    └── screen.rs        # Screen buffer
```

### Modified Files
- `Cargo.toml` - new dependencies
- `src/lib.rs` - export new modules
- `src/workspace.rs` - add `worktree_path`, `terminal` fields
- `src/app.rs` - input modes, worktree manager, terminal lifecycle
- `src/main.rs` - state load/save, terminal I/O
- `src/ui/main_panel.rs` - render terminal buffer
- `src/ui/sidebar.rs` - show branch info

---

## Dependency Graph

```
Phase 6 (Persistence Foundation)
         │
         ▼
Phase 7 (Workspace State)
         │
         ▼
Phase 8 (Worktree Core)
         │
         ▼
Phase 9 (Worktree UI) ◄── Can partially parallel with Phase 10
         │
         ▼
Phase 10 (PTY Infrastructure)
         │
         ▼
Phase 11 (Screen Buffer)
         │
         ▼
Phase 12 (Terminal UI Integration)
```

---

## Verification

After each phase:
1. `direnv exec . cargo build` - compiles
2. `direnv exec . cargo test` - tests pass
3. `direnv exec . cargo clippy` - no warnings
4. `direnv exec . cargo fmt --check` - formatted
5. Manual testing per phase success criteria

### End-to-End Test (after Phase 12)
1. Start app, press `n`, enter "test-workspace"
2. Worktree created, terminal opens in worktree dir
3. Type `pwd`, see worktree path
4. Type `ls`, see colored output
5. Ctrl+O to sidebar, `d` to delete workspace
6. Quit with `q`, restart, verify remaining state
