# composer_tui - Phases 13-18 Implementation Plan

> **Note:** This document describes phases 13-18, continuing from the completed terminal integration (phases 6-12). See `IMPLEMENTATION_PLAN.md` for phases 1-5 (MVP) and `IMPLEMENTATION_PLAN_V2.md` for phases 6-12.

## Overview

Six phases to evolve from working prototype to polished daily-driver. Each phase is independently useful and builds on the prior.

**Order**: Polish → Scrollback → Settings → Tabs → Sidebar Status/Git → Command Palette

---

## Phase 13: Polish Pass

**Goal**: Status bar, mouse support, fullscreen toggle.

### Tasks
1. **Status bar** (`src/ui/status_bar.rs`) - context-sensitive keybinding hints at bottom
   - Add `Constraint::Length(1)` row to vertical layout in `ui/mod.rs`
   - Show different hints per focus mode and input mode
   - Remove inline hint lines from `main_panel.rs`
   - Update `main_panel_terminal_size()` to account for status bar row

2. **Mouse support** - click to select/focus
   - Enable `EnableMouseCapture`/`DisableMouseCapture` in `main.rs`
   - Handle `Event::Mouse` clicks: sidebar area → select workspace + focus sidebar, main panel → focus main
   - Add `App::set_selected_index(usize)` with bounds checking
   - Recompute layout rects in event handler for hit-testing (cheap, avoids mutable state in render)

3. **Fullscreen toggle** - `z` from sidebar hides sidebar
   - Add `fullscreen: bool` to `App`
   - `z` in sidebar toggles; `Ctrl+O` from terminal exits fullscreen + returns to sidebar
   - Skip sidebar in layout when fullscreen; give main panel full width
   - Update `main_panel_terminal_size()` for fullscreen width

### Files
- Create: `src/ui/status_bar.rs`
- Modify: `src/ui/mod.rs`, `src/ui/main_panel.rs`, `src/app.rs`, `src/main.rs`

### Success Criteria
- [ ] Status bar visible with context-appropriate hints
- [ ] Click sidebar items to select, click main panel to focus
- [ ] `z` toggles fullscreen, `Ctrl+O` exits fullscreen
- [ ] All existing tests pass + new tests for `set_selected_index`, `toggle_fullscreen`
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-13.md`

---

## Phase 14: Terminal Scrollback

**Goal**: Scrollback buffer so users can review terminal history.

### Tasks
1. **Extend `ScreenBuffer`** with `scrollback: VecDeque<Vec<Cell>>` and configurable limit (default 1000)
   - When lines scroll off top, push to scrollback deque
   - Add `ScreenBuffer::new_with_scrollback(cols, rows, limit)`
   - Add `ScreenBuffer::viewport_rows(scroll_offset) -> impl Iterator<Item = &[Cell]>` for unified scrollback+visible access

2. **Add scroll state** to `WorkspaceTerminal`
   - `scroll_offset: usize` (0 = live bottom)
   - Methods: `scroll_up()`, `scroll_down()`, `scroll_to_bottom()`, `is_scrolled()`
   - New output while scrolled: keep offset stable (don't snap to bottom)
   - Any keypress while scrolled: snap to bottom (standard terminal behavior)

3. **Scroll keybindings**: `Shift+PageUp`/`Shift+PageDown` in main focus
   - Intercept before terminal passthrough (shifted page keys aren't normally sent to PTY)

4. **Visual indicator** when scrolled: show `[+N lines]` in status bar or main panel border

5. **Hide cursor** when scrolled (only visible at live bottom)

### Files
- Modify: `src/terminal/screen.rs`, `src/workspace.rs`, `src/ui/main_panel.rs`, `src/ui/status_bar.rs`, `src/main.rs`

### Success Criteria
- [ ] History accumulates in scrollback buffer
- [ ] `Shift+PageUp`/`Shift+PageDown` scroll the view
- [ ] Visual indicator when not at live bottom
- [ ] Typing snaps back to bottom; cursor hidden when scrolled
- [ ] Tests for scrollback accumulation, viewport computation
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-14.md`

---

## Phase 15: Settings & Preferences

**Goal**: Expand config system, add auto-spawn command, provide settings editing.

### Tasks
1. **Expand `Config` struct** with new `Option` fields (all `#[serde(default)]` for backward compat):
   - `default_shell: Option<String>`
   - `auto_spawn_command: Option<String>` (e.g. `"claude"`)
   - `scrollback_limit: Option<usize>` (default 1000)
   - `sidebar_width: Option<u16>` (default 20, up from 12)
   - `theme: Option<ThemeConfig>` (focused_border_color, selected_bg_color)

2. **Store `Config` on `App`** - add `App::config()` accessor, wire through to terminal spawning and rendering

3. **Wire settings into behavior**:
   - `default_shell` → terminal spawn
   - `scrollback_limit` → ScreenBuffer creation
   - `sidebar_width` → layout constraint in `ui/mod.rs`
   - Theme colors → border styles in all UI components

4. **Auto-spawn command**: after terminal starts, if configured, write `command\r` as initial input

5. **Settings editing**: `S` from sidebar opens config.toml in `$EDITOR`
   - Temporarily leave raw mode + alternate screen, spawn editor, re-enter on exit
   - Reload config after editor closes

6. **Config reload**: `R` from sidebar reloads config from disk with brief status message

### Files
- Modify: `src/config.rs`, `src/app.rs`, `src/workspace.rs`, `src/main.rs`
- Modify: `src/ui/mod.rs`, `src/ui/header.rs`, `src/ui/sidebar.rs`, `src/ui/main_panel.rs`, `src/ui/status_bar.rs`

### Success Criteria
- [ ] New config fields backward-compatible with existing config.toml
- [ ] Auto-spawn command runs in new workspace terminals
- [ ] `S` opens editor, `R` reloads config
- [ ] Theme colors applied from config
- [ ] Tests for config round-trip with new fields
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-15.md`

---

## Phase 16: Workspace Tabs (Multi-terminal)

**Goal**: Each workspace gets multiple tabs with independent terminals. This is the biggest structural change.

### Data Model Change
```
Current:  Workspace → Option<WorkspaceTerminal>     (1:1)
New:      Workspace → Vec<Tab> + active_tab_index   (1:N)
```

### Tasks
1. **Create `Tab` model** (`src/tab.rs`)
   - Extract terminal fields from `Workspace` into `Tab` (terminal, screen, scroll_offset, exit_status, error)
   - Move terminal methods to `Tab`: `ensure_terminal_started`, `poll_terminal`, `write_input`, `resize`, `terminal_screen`, `terminal_state`

2. **Refactor `Workspace`** to own `Vec<Tab>` + `active_tab_index`
   - Delegate existing terminal methods to active tab (backward-compatible API)
   - Add: `add_tab()`, `close_tab()`, `select_tab()`, `next_tab()`, `prev_tab()`, `tab_count()`

3. **Update `App` tick logic** to poll all tabs across all workspaces

4. **State persistence**: add `tabs: Option<Vec<TabState>>` to `WorkspaceState`
   - Migration: old state without `tabs` → single default tab

5. **Tab bar UI** (`src/ui/tab_bar.rs`)
   - Horizontal tab labels between header and main panel (only shown when >1 tab)
   - Active tab highlighted

6. **Tab keybindings**
   - `Alt+1`-`Alt+9` switch tabs (works from any focus mode)
   - `Ctrl+T` from sidebar creates new tab
   - `Ctrl+W` from sidebar closes active tab

7. **Auto-spawn** creates first tab with configured command

### Files
- Create: `src/tab.rs`, `src/ui/tab_bar.rs`
- Modify: `src/workspace.rs`, `src/app.rs`, `src/state.rs`, `src/main.rs`, `src/ui/mod.rs`, `src/ui/main_panel.rs`, `src/lib.rs`

### Success Criteria
- [ ] Each workspace supports 1+ tabs with independent terminals
- [ ] Tab bar visible when >1 tab
- [ ] `Alt+N` switches tabs, `Ctrl+T` creates, `Ctrl+W` closes
- [ ] Old state.toml loads correctly with migration
- [ ] Tests for tab CRUD, state migration, multi-tab independence
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-16.md`

---

## Phase 17: Sidebar Status & Git Integration

**Goal**: Enrich sidebar with live process status, git dirty/clean, PR/CI status.

### Tasks
1. **Process status indicators** in sidebar
   - Colored dots: green=running, red=exited, gray=not started, yellow=failed
   - Unicode `●` (U+25CF) with appropriate foreground color
   - Access active tab's `terminal_state()` during sidebar render

2. **Git dirty/clean indicator** (background polling via `git2`)
   - Add `git_status: Option<GitWorkspaceStatus>` to `Workspace`
   - `GitStatusFetcher` (`src/git_status.rs`): background thread + mpsc channel, polls every 5-10s
   - Display: `*` after branch name for dirty, nothing for clean

3. **PR/CI status via `gh` CLI** (background polling)
   - `GhStatusFetcher` (`src/gh_status.rs`): runs `gh pr view <branch> --json state,title,statusCheckRollup`
   - Polls every 30-60s; handles missing `gh` gracefully
   - Display: compact badge like `PR` with color (green=passing, yellow=pending, red=failing)

4. **Widen sidebar** to 20 cols default (configurable via `sidebar_width` from Phase 15)
   - New format: `● name (branch) * PR`

5. **Rename `App::tick_terminals` → `App::tick`** since it now manages status fetchers too

### Files
- Create: `src/git_status.rs`, `src/gh_status.rs`
- Modify: `src/workspace.rs`, `src/app.rs`, `src/ui/sidebar.rs`, `src/ui/mod.rs`, `src/main.rs`, `src/lib.rs`

### Success Criteria
- [ ] Colored process status dots per workspace
- [ ] Git dirty/clean updates every few seconds
- [ ] PR/CI status when `gh` available, graceful fallback when not
- [ ] All status fetching non-blocking
- [ ] Tests for status parsing, sidebar rendering with various states
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-17.md`

---

## Phase 18: Command Palette & Power Features

**Goal**: Discoverability and power-user features as capstone.

### Tasks
1. **Command palette** (`src/ui/command_palette.rs`)
   - `Ctrl+P` opens centered modal with text input + filtered command list
   - All keybindings registered as named commands with fuzzy substring matching
   - Enter executes, Esc closes

2. **Broadcast input mode**
   - `B` from sidebar toggles broadcast: all typed input goes to all workspace terminals
   - Red status bar indicator when active
   - Useful for running same command across all agents

3. **Workspace reordering**
   - `Shift+J`/`Shift+K` in sidebar moves selected workspace up/down

4. **Quick-switch overlay**
   - `/` from sidebar: type-to-filter workspace list for fast jumping

### Files
- Create: `src/ui/command_palette.rs`
- Modify: `src/app.rs`, `src/main.rs`, `src/ui/mod.rs`, `src/ui/status_bar.rs`

### Success Criteria
- [ ] Command palette works with fuzzy search
- [ ] Broadcast mode sends to all terminals with visual indicator
- [ ] Workspace reordering works and persists
- [ ] Quick-switch filters workspaces by name
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-18.md`

---

## New Dependencies

```toml
# No new crate dependencies expected for phases 13-18.
# All features build on existing deps: ratatui, crossterm, git2, portable-pty, vte, serde, toml, directories.
# gh CLI integration (phase 17) shells out to `gh` rather than adding HTTP deps.
```

---

## New/Modified Files

### New Modules
```
src/
├── tab.rs                   # Tab model (phase 16)
├── git_status.rs            # Background git status fetcher (phase 17)
├── gh_status.rs             # Background gh CLI PR/CI fetcher (phase 17)
└── ui/
    ├── status_bar.rs        # Context-sensitive keybinding hints (phase 13)
    ├── tab_bar.rs           # Tab labels between header and main (phase 16)
    └── command_palette.rs   # Fuzzy command search modal (phase 18)
```

### Modified Files (across all phases)
- `Cargo.toml` - no new deps expected
- `src/lib.rs` - re-export new modules
- `src/main.rs` - event handling for mouse, scrollback, tabs, settings, command palette
- `src/app.rs` - fullscreen, config, tab CRUD, tick refactor, broadcast mode
- `src/workspace.rs` - scroll state, tab ownership refactor, git/PR status fields
- `src/config.rs` - expanded settings
- `src/state.rs` - tab persistence, state migration
- `src/terminal/screen.rs` - scrollback buffer
- `src/ui/mod.rs` - layout changes (status bar row, tab bar row, configurable sidebar width)
- `src/ui/sidebar.rs` - status indicators, wider layout
- `src/ui/main_panel.rs` - scrollback-aware rendering, remove inline hints
- `src/ui/header.rs` - theme colors

---

## Dependency Graph

```
Phase 13 (Polish)
    │
    ▼
Phase 14 (Scrollback) ── uses status bar for scroll indicator
    │
    ▼
Phase 15 (Settings) ── uses scrollback_limit, sidebar_width
    │
    ▼
Phase 16 (Tabs) ── uses auto_spawn, biggest structural change
    │
    ▼
Phase 17 (Git/Status) ── uses tab model, wider sidebar
    │
    ▼
Phase 18 (Command Palette) ── ties everything together
```

---

## Verification

After each phase:
1. `direnv exec . cargo build` - compiles
2. `direnv exec . cargo test` - tests pass
3. `direnv exec . cargo clippy` - no warnings
4. `direnv exec . cargo fmt --check` - formatted
5. Manual testing per phase success criteria

### End-to-End Test (after Phase 18)
1. Start app, press `n`, create workspace "test-agent"
2. Worktree created, terminal opens in worktree dir
3. Type `ls --color`, see colored output with scrollback
4. `Shift+PageUp` to scroll back, see history
5. `z` to toggle fullscreen, `Ctrl+O` to return
6. `Ctrl+T` to add a shell tab, `Alt+2` to switch to it
7. Click sidebar with mouse to switch workspaces
8. See git status indicator update after making changes
9. `Ctrl+P` to open command palette, search for "broadcast"
10. `Shift+K` to reorder workspaces
11. `S` to open settings, configure auto_spawn_command
12. Quit with `q`, restart, verify all state preserved
