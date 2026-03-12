# Agent Guide

Instructions for AI coding agents working on this project. Read this before starting any task.

## Project Overview

`composer_tui` is a Rust terminal UI for orchestrating parallel AI coding agents, using git worktrees to isolate each agent's work. Built with [ratatui](https://ratatui.rs/) and crossterm.

## Build & Verify

**Always run these before considering work complete:**

```bash
cargo fmt        # Format code
cargo clippy     # Lint — must be warning-free
cargo test       # All tests must pass
cargo build      # Verify it compiles
```

If `cargo` is not available in your shell, use the Nix dev environment via `direnv exec .` or `nix develop`:

```bash
direnv exec . cargo fmt
# or
nix develop --command cargo fmt
```

If any check fails, fix the issue before committing.

## Architecture

```
src/
├── main.rs              # Terminal setup, event loop, key/mouse dispatch
├── app.rs               # Core state machine: workspaces, focus, modals, tick loop
├── workspace.rs         # Workspace model: owns Vec<Tab>, delegates terminal I/O to active tab
├── tab.rs               # Single terminal tab: PTY lifecycle, scrollback, exit status
├── config.rs            # Load/save ~/.config/composer_tui/config.toml, defaults
├── state.rs             # Persistent state (state.toml): serialize/deserialize app state
├── terminal.rs          # PTY spawning, background reader thread, output channel
├── terminal/screen.rs   # 2D screen buffer, ANSI/VT100 parsing, scrollback deque
├── git_status.rs        # Background thread: polls git dirty/clean status (~7s interval)
├── gh_status.rs         # Background thread: polls PR/CI status via `gh` CLI (~45s interval)
├── worktree.rs          # Git worktree CRUD via libgit2
├── lib.rs               # Module re-exports
└── ui/
    ├── mod.rs           # Main render fn, layout (header/sidebar/main/status bar)
    ├── header.rs        # Top bar: workspace name, branch, git/PR status
    ├── sidebar.rs       # Left panel: workspace list with status indicators
    ├── main_panel.rs    # Right panel: terminal output, tab bar
    └── status_bar.rs    # Bottom: context-sensitive keybinding hints
```

### Key Design Patterns

- **UI-independent core**: App logic in `app.rs` is testable without ratatui. UI modules are pure rendering functions that read from `&App`.
- **Background polling**: Git and PR status fetchers run on background threads with mpsc channels. The main thread never blocks on I/O — it drains updates via `try_recv()` each tick.
- **Modal state machine**: `InputMode` enum controls UI state (Normal, CreateWorkspace, ConfirmDelete, etc). Each mode has its own rendering and input handling.
- **Workspace → Tab delegation**: `Workspace` owns `Vec<Tab>` but exposes terminal methods that delegate to the active tab. This keeps the API uniform whether single or multi-tab.
- **Graceful degradation**: Missing `gh` CLI, missing config, or failed polls never crash the app — they fall back to sensible defaults.

### Key Types

| Type | Location | Role |
|------|----------|------|
| `App` | app.rs | Root state: workspaces, selection, focus, config, status fetchers |
| `Workspace` | workspace.rs | Owns tabs, branch info, git/PR status |
| `Tab` | tab.rs | Single PTY terminal with screen buffer and scroll state |
| `ScreenBuffer` | terminal/screen.rs | 2D char grid + scrollback + ANSI parser |
| `Config` | config.rs | User settings (shell, theme, sidebar width, etc) |
| `AppState` | state.rs | Serializable snapshot for persistence |
| `FocusArea` | app.rs | Enum: Header, Sidebar, Main |
| `InputMode` | app.rs | Enum: Normal, CreateWorkspace, ConfirmDelete, etc |

### Control Flow

```
main.rs event loop:
  1. Load config + persisted state
  2. Each tick:
     a. app.tick(cols, rows) — spawn terminals, poll I/O, drain status updates
     b. ui::render(frame, &app) — draw all panels
     c. Poll crossterm events (16ms timeout)
     d. Dispatch key/mouse events to app methods
  3. On quit: save state to disk
```

## Coding Conventions

### Rust Style
- **No `as any` equivalents** — use strong types everywhere
- **Error handling**: non-fatal errors → `InputMode::Error` modal. Missing files → return defaults, never panic
- **Naming**: `selected_*` = highlighted item, `active_*` = focused item, `*_state()` = returns enum, `*_status()` = returns optional status
- **Comments**: comment non-obvious logic. Don't over-comment trivial code
- **Debug logging**: use `eprintln!` (stdout is owned by ratatui)

### Adding New Features
1. **Domain logic** goes in `app.rs`, `workspace.rs`, or a new module — never in UI code
2. **UI rendering** goes in `src/ui/` — these are pure functions taking `&App` and `Frame`
3. **New input handling** goes in `main.rs` key dispatch
4. **New modals** need a variant in `InputMode` enum + render function + key handler
5. **Background work** should follow the fetcher pattern: background thread + mpsc channel + non-blocking drain in `app.tick()`
6. **State persistence**: if it should survive restart, add it to `AppState`/`WorkspaceState` in `state.rs`

### Testing

**All new code must have unit test coverage.** Aim for 100% coverage where possible — there will always be edge cases that can't be practically unit tested (e.g. PTY I/O, raw terminal mode), but all logic, state transitions, and data transformations should be tested.

**Prefer a TDD approach:**
1. Write failing tests that define the expected behavior
2. Implement the minimum code to make them pass
3. Refactor with confidence that tests catch regressions

**Test guidelines:**
- Tests live inline in each module via `#[cfg(test)]`
- Use `test_app()` helper (creates app with state saving disabled)
- Test app logic without UI framework — don't test rendering, test state transitions
- Tests must not depend on external tools (git repos, `gh` CLI, filesystem)
- Cover both happy paths and error/edge cases
- ~91 existing tests across all modules — new features should maintain or improve this ratio

### What NOT to Do
- Don't add `unsafe` code
- Don't add new crate dependencies without justification — the project intentionally keeps deps minimal
- Don't put business logic in UI rendering functions
- Don't block the main thread on I/O (use background threads + channels)
- Don't use `unwrap()` on user-facing paths — handle errors gracefully
- Don't skip clippy warnings — fix them

## Git & PR Workflow

- Create a feature branch from `main`
- One logical change per commit
- Run all checks (fmt, clippy, test, build) before pushing
- Write a handoff summary in `.context/` if the work spans multiple sessions

### Conventional Commits

All commit messages and PR titles **must** follow the [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <short summary>
```

**Types:**
- `feat` — new feature or capability
- `fix` — bug fix
- `refactor` — code change that neither fixes a bug nor adds a feature
- `test` — adding or updating tests
- `docs` — documentation only
- `chore` — build, CI, tooling, or other non-code changes
- `perf` — performance improvement

**Scope** is optional but encouraged — use the module or area affected (e.g. `sidebar`, `git`, `config`, `ui`, `workspace`, `tabs`).

**Examples:**
```
feat(sidebar): add git dirty indicator per workspace
fix(terminal): prevent scroll snap when reviewing history
refactor(app): extract modal handling into separate method
test(worktree): add coverage for branch conflict detection
docs: add AGENTS.md with architecture and conventions
chore(ci): add GitHub Actions workflow for lint and test
```

PR titles follow the same format. The PR title becomes the squash-merge commit message, so it should be a clear, concise summary of the change.

## Config & State Locations

| File | Purpose |
|------|---------|
| `~/.config/composer_tui/config.toml` | User settings (shell, theme, sidebar width, auto_spawn_command, scrollback_limit) |
| `~/.config/composer_tui/state.toml` | Persisted app state (workspaces, tabs, selection) |

## Reference Docs

- `docs/DESIGN.md` — Original MVP architecture (context only, partially outdated)
- `docs/IMPLEMENTATION_PLAN_V3.md` — Phases 13-18 plan (phases 13-17 complete)
- `.context/handoff-phase-*.md` — Per-phase summaries of what was built and decisions made
