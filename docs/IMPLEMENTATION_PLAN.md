# composer_tui - Implementation Plan

> **Note:** This document describes the implementation plan and is not updated after creation. Progress tracking and status updates belong in separate handoff documents or task tracking systems.

## Overview

This plan breaks the MVP into 5 sequential phases. Each phase:
- Has clear success criteria that can be verified independently
- Produces a working (though incomplete) artifact
- Ends with a handoff document written to `.context/handoff-phase-N.md`

Handoff documents are written by the implementing agent and contain:
- What was implemented
- Key decisions made
- Any deviations from the plan
- Gotchas or context for the next phase

---

## Phase 1: Project Scaffolding

### Goal
Set up the project structure so `cargo build` and `cargo run` succeed.

### Tasks
1. Create `Cargo.toml` with project metadata and dependencies (ratatui, crossterm)
2. Create `flake.nix` with Rust toolchain (stable + rust-analyzer)
3. Create `.envrc` with `use flake`
4. Update `.gitignore` for Rust/Nix artifacts
5. Create minimal `src/main.rs` that prints "Hello, composer_tui!"
6. Create `src/lib.rs` (empty, for future test organization)

### Success Criteria
- [ ] `direnv exec . cargo build` completes without errors
- [ ] `direnv exec . cargo run` prints "Hello, composer_tui!" and exits
- [ ] `direnv exec . cargo test` runs (even if no tests yet)
- [ ] `direnv exec . cargo clippy` passes with no warnings
- [ ] `direnv exec . cargo fmt --check` passes

### Handoff
Write `.context/handoff-phase-1.md`

---

## Phase 2: Terminal Setup & Event Loop

### Goal
Establish terminal control with crossterm and implement a basic event loop that handles quitting.

### Tasks
1. Set up crossterm alternate screen and raw mode in `main.rs`
2. Implement proper terminal cleanup (restore on exit, including panics)
3. Create basic event loop that polls for keyboard input
4. Handle `q` and `Esc` keys to quit
5. Ensure clean exit (terminal restored to normal state)

### Success Criteria
- [ ] App starts and takes over terminal (alternate screen, no cursor)
- [ ] Terminal shows blank screen (no rendering yet)
- [ ] Pressing `q` exits cleanly
- [ ] Pressing `Esc` exits cleanly
- [ ] After exit, terminal is restored (cursor visible, normal mode)
- [ ] If app panics, terminal is still restored (test by adding temporary `panic!()`)

### Handoff
Write `.context/handoff-phase-2.md`

---

## Phase 3: Core Data Structures

### Goal
Implement the core data model with unit tests, independent of UI.

### Tasks
1. Create `src/workspace.rs` with `Workspace` struct
2. Create `src/app.rs` with `App` struct
3. Implement `App` methods:
   - `new()` - creates app with hardcoded workspaces
   - `select_next()` - move selection down (wrap at bottom)
   - `select_previous()` - move selection up (wrap at top)
   - `quit()` - set should_quit flag
   - `should_quit()` - getter for quit flag
   - `workspaces()` - getter for workspace list
   - `selected_index()` - getter for current selection
4. Write unit tests for all state transitions
5. Export modules from `lib.rs`

### Success Criteria
- [ ] `direnv exec . cargo test` passes with tests for:
  - Initial state (3 workspaces, index 0, not quitting)
  - `select_next()` increments index
  - `select_next()` wraps from last to first
  - `select_previous()` decrements index
  - `select_previous()` wraps from first to last
  - `quit()` sets flag
- [ ] `direnv exec . cargo clippy` passes
- [ ] Code is documented with doc comments

### Handoff
Write `.context/handoff-phase-3.md`

---

## Phase 4: UI Layout & Rendering

### Goal
Implement the TUI layout matching the design mockup.

### Tasks
1. Create `src/ui/mod.rs` with main `render()` function
2. Create `src/ui/header.rs` - renders "composer_tui" title bar
3. Create `src/ui/sidebar.rs` - renders workspace list
4. Create `src/ui/main_panel.rs` - renders placeholder content area
5. Wire up rendering in `main.rs` event loop
6. Use ratatui `Layout` to split screen:
   - Vertical split: Header (1 row) | Body
   - Horizontal split of Body: Sidebar (fixed 12 cols) | MainPanel

### Success Criteria
- [ ] App renders layout matching ASCII mockup in DESIGN.md
- [ ] Header shows "composer_tui" centered
- [ ] Sidebar shows W1, W2, W3 as list items
- [ ] Main panel shows placeholder text (e.g., "Main Content Area")
- [ ] Layout has visible borders between sections
- [ ] Resizing terminal doesn't crash (ratatui handles this)

### Handoff
Write `.context/handoff-phase-4.md`

---

## Phase 5: Navigation & Integration

### Goal
Connect keyboard input to state, add visual selection feedback, complete the MVP.

### Tasks
1. Create `src/event.rs` for input handling (or handle in app.rs)
2. Wire `j`/`↓` to `app.select_next()`
3. Wire `k`/`↑` to `app.select_previous()`
4. Update sidebar rendering to highlight selected workspace
5. Final polish pass (consistent styling, no debug output)
6. Run full manual testing checklist

### Success Criteria
- [ ] Pressing `j` moves selection down
- [ ] Pressing `k` moves selection up
- [ ] Arrow keys also work (↓ and ↑)
- [ ] Selection visually highlighted in sidebar (different color/style)
- [ ] Selection wraps correctly at boundaries
- [ ] `q` and `Esc` still quit
- [ ] No warnings from `direnv exec . cargo clippy`
- [ ] Code is formatted (`direnv exec . cargo fmt --check`)
- [ ] All unit tests pass

### Final Checklist (from DESIGN.md)
- [ ] App starts without error
- [ ] Layout matches mockup
- [ ] j/k navigation works and wraps
- [ ] Selection highlight is visible
- [ ] q/Esc quits cleanly (terminal restored)

### Handoff
Write `.context/handoff-phase-5.md` (final summary, notes for future development)

---

## Dependency Graph

```
Phase 1 (Scaffolding)
    │
    ▼
Phase 2 (Terminal/Event Loop)
    │
    ▼
Phase 3 (Data Structures) ◄─── Can be developed in parallel with Phase 2
    │                           if interface is agreed upon
    ▼
Phase 4 (UI Rendering)
    │
    ▼
Phase 5 (Navigation/Integration)
```

---

## Context for Agents

Each phase agent should:

1. **Read first:**
   - `docs/DESIGN.md` - Full design specification
   - `docs/IMPLEMENTATION_PLAN.md` - This document (their phase section)
   - `.context/handoff-phase-{N-1}.md` - Previous phase handoff (if not Phase 1)

2. **Implement** the tasks for their phase

3. **Verify** all success criteria are met

4. **Write** `.context/handoff-phase-{N}.md` with:
   - Summary of what was implemented
   - Any decisions or deviations from plan
   - Gotchas or important context
   - Files added/modified

---

## File Checklist

By end of all phases, these files should exist:

```
composer_tui/
├── .envrc
├── .gitignore
├── Cargo.toml
├── Cargo.lock
├── flake.nix
├── flake.lock
├── README.md
├── docs/
│   ├── DESIGN.md
│   └── IMPLEMENTATION_PLAN.md
├── .context/
│   ├── handoff-phase-1.md
│   ├── handoff-phase-2.md
│   ├── handoff-phase-3.md
│   ├── handoff-phase-4.md
│   └── handoff-phase-5.md
└── src/
    ├── main.rs
    ├── lib.rs
    ├── app.rs
    ├── workspace.rs
    ├── event.rs (optional, may be in app.rs)
    └── ui/
        ├── mod.rs
        ├── header.rs
        ├── sidebar.rs
        └── main_panel.rs
```
