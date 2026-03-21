# composer_tui - Phases 19-26 Implementation Plan

> **Note:** This document describes phases 19-26, continuing from the polish/power-features phases (13-18). See `IMPLEMENTATION_PLAN_V3.md` for phases 13-18, `IMPLEMENTATION_PLAN_V2.md` for phases 6-12, and `IMPLEMENTATION_PLAN.md` for phases 1-5 (MVP).

## Overview

Eight phases evolving from a polished daily-driver into a full-featured AI-native development environment. Each phase is independently useful and builds on the prior.

**Order**: Git Workflow → PR Lifecycle → Workspace Organization → Navigation & Discovery → Review & Code Quality → Agent Collaboration & Context → MCP Integration → Linear & External Services

---

## Phase 19: Git Workflow

**Goal**: First-class git operations without leaving the TUI. Closes issues #16, #17, #18, #19, #23, #24.

### Tasks
1. **Changes panel** (#16) (`src/ui/changes_panel.rs`)
   - Display files grouped by uncommitted vs committed status
   - Show diff summary per file (insertions/deletions)
   - Keybinding: `g` from sidebar opens changes panel

2. **Target branch selection** (#17)
   - Allow changing the base branch mid-workspace without restart
   - Add `base_branch: Option<String>` to `Workspace`
   - `T` keybinding in sidebar opens branch picker overlay
   - Persist updated base branch in `WorkspaceState`

3. **Commit and push shortcut** (#18)
   - `C` from sidebar: stage all, commit with AI-generated or user-provided message, push
   - Inline message editor (single line) before commit
   - Show success/error in status bar

4. **Merge conflict detection** (#19)
   - Detect conflicted files via `git2` repository status
   - Surface list of conflicting files in the TUI
   - Prompt agent with conflict details for resolution guidance

5. **Rebase detection & preference** (#23)
   - Detect diverged branches on workspace open
   - Auto-detect user preference (merge vs rebase) from local git config
   - Prompt user if preference is ambiguous; remember choice per repo

6. **Custom git branch prefix** (#24)
   - `branch_prefix: Option<String>` in `Config`
   - Applied when auto-generating branch names for new workspaces
   - Configurable via `S` settings editor

### Files
- Create: `src/ui/changes_panel.rs`
- Modify: `src/config.rs`, `src/workspace.rs`, `src/app.rs`, `src/main.rs`, `src/ui/mod.rs`, `src/ui/status_bar.rs`

### Success Criteria
- [ ] Changes panel shows uncommitted vs committed files
- [ ] Target branch changeable mid-session and persisted
- [ ] Commit and push shortcut works end-to-end
- [ ] Merge conflicts surfaced with file list
- [ ] Rebase preference detected and remembered
- [ ] Custom branch prefix applied to new workspaces
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-19.md`

---

## Phase 20: PR Lifecycle

**Goal**: Create, review, and merge PRs without leaving the TUI. Closes issues #20, #21, #22.

### Tasks
1. **Create/merge PRs** (#20)
   - `P` from sidebar: open PR creation flow using `gh pr create`
   - Support customizable PR templates (load from `.github/pull_request_template.md`)
   - Editable description in a multi-line text area before submission
   - Merge action: `M` from sidebar runs `gh pr merge` with strategy selection

2. **PR status checks & GitHub Actions** (#21)
   - Extend `GhStatusFetcher` (from phase 17) with check run details
   - Show per-check status (pass/fail/pending) in a collapsible list
   - Display GitHub Actions logs inline for failed checks
   - Re-run button for failed checks (`gh run rerun`)
   - Poll interval: 30s active, 60s background

3. **Graphite stack support** (#22)
   - Detect Graphite stacks via `gt stack` command (if `gt` is installed)
   - Sidebar visualization: show stack level indicator next to workspace
   - Navigation: `[` / `]` to jump between stack levels
   - Graceful no-op when Graphite is not installed

### Files
- Create: `src/gh_checks.rs`, `src/ui/pr_panel.rs`, `src/ui/checks_panel.rs`
- Modify: `src/gh_status.rs`, `src/app.rs`, `src/main.rs`, `src/ui/mod.rs`, `src/ui/sidebar.rs`

### Success Criteria
- [ ] PR creation with template and editable description
- [ ] PR merge with strategy selection
- [ ] Per-check status with logs for failed checks
- [ ] Re-run button works
- [ ] Graphite stack indicator in sidebar when `gt` available
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-20.md`

---

## Phase 21: Workspace Organization

**Goal**: Rich workspace lifecycle management. Closes issues #28, #30, #31, #32, #34, #36, #38, #85.

### Tasks
1. **Create workspaces from PRs/branches** (#28)
   - `n` workspace creation flow: add option to seed from existing PR, branch, or Linear issue
   - `gh pr list` picker for PR-based creation; `git branch -a` picker for branches
   - Linear issue seeding deferred to Phase 26

2. **Workspace status grouping** (#30)
   - Add `WorkspaceStatus` enum: `Backlog | InProgress | InReview | Done`
   - `s` from sidebar cycles through statuses
   - Sidebar renders workspaces grouped by status with headers

3. **Group by repo and pin** (#31)
   - `group_by_repo: bool` config option (default false)
   - When enabled, sidebar shows repo name headers grouping workspaces
   - `p` from sidebar pins/unpins workspace to top of its group
   - Persist pin state in `WorkspaceState`

4. **Archive/unarchive with git state** (#32)
   - `a` archives workspace: saves git stash reference + branch name, hides from default view
   - `A` shows archive overlay to browse and restore archived workspaces
   - Restore: checkout branch, pop stash if present

5. **Auto-archive on PR merge** (#34)
   - `GhStatusFetcher` detects PR state change to `MERGED`
   - On detection, auto-archive the associated workspace
   - Show brief notification in status bar

6. **Fork workspace** (#36)
   - `F` from sidebar: creates new workspace from current, summarizing chat history
   - Summary generated by writing first N lines of terminal scrollback to new workspace context
   - New workspace opens on same branch

7. **Workspace search** (#38)
   - `\` from sidebar: opens search overlay filtering by branch name, repo, or PR number
   - Real-time filter as user types
   - Enter to jump; Esc to close

8. **Workspace-specific environment isolation** (#85)
   - Per-workspace `.env` override file in worktree root
   - `WorkspaceTerminal` merges workspace `.env` into PTY environment at spawn time
   - UI indicator when workspace has custom env vars set

### Files
- Create: `src/ui/workspace_search.rs`, `src/ui/archive_overlay.rs`
- Modify: `src/workspace.rs`, `src/app.rs`, `src/state.rs`, `src/config.rs`, `src/gh_status.rs`, `src/ui/sidebar.rs`, `src/ui/mod.rs`, `src/main.rs`

### Success Criteria
- [ ] Workspace creation from PR/branch picker
- [ ] Status grouping with kanban-style sidebar headers
- [ ] Repo grouping and pinning persisted
- [ ] Archive/unarchive with git state preserved
- [ ] Auto-archive fires on PR merge
- [ ] Fork creates new workspace with context summary
- [ ] Search filters workspaces in real time
- [ ] Environment isolation per workspace
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-21.md`

---

## Phase 22: Navigation & Discovery

**Goal**: Fast navigation across files, terminal history, and external tools. Closes issues #27, #29, #64, #78, #79.

### Tasks
1. **Terminal search** (#27) (`src/ui/terminal_search.rs`)
   - `Ctrl+F` in main focus: opens search bar at bottom of terminal panel
   - Forward/backward navigation (`n`/`N` or Enter/Shift+Enter)
   - Match highlighting in rendered terminal output
   - Esc to close and return to live view

2. **File picker** (#29)
   - `Ctrl+P` opens fuzzy file picker modal (walks worktree via `walkdir`)
   - Type to filter; Enter opens file in `$EDITOR` (same mechanism as `S` settings)
   - Respects `.gitignore`

3. **File tree explorer** (#64) (`src/ui/file_tree.rs`)
   - `e` from sidebar opens collapsible file tree side panel
   - Monorepo-aware: detects `package.json`/`Cargo.toml`/`go.mod` boundaries
   - Navigate with `j`/`k`/`h`/`l`; Enter opens file in `$EDITOR`
   - Toggle show/hide: `e` again

4. **"Open in" external editors** (#79)
   - `O` from sidebar: opens current workspace directory in configured external editor
   - `open_in: Option<Vec<String>>` config list (e.g. `["code", "cursor", "zed"]`)
   - If multiple configured, show picker; if one, open directly
   - Fallback to `$EDITOR` if none configured

5. **Zen mode** (#78)
   - `Z` (capital) hides all chrome: sidebar, status bar, tab bar — only terminal visible
   - Toggle back with `Z` or `Ctrl+O`
   - Distinct from lowercase `z` fullscreen (which only hides sidebar)

### Files
- Create: `src/ui/terminal_search.rs`, `src/ui/file_tree.rs`
- Modify: `src/config.rs`, `src/app.rs`, `src/main.rs`, `src/ui/mod.rs`, `src/ui/main_panel.rs`, `src/ui/status_bar.rs`

### Success Criteria
- [ ] Terminal search with match highlighting and navigation
- [ ] File picker opens files via `$EDITOR`
- [ ] File tree explores worktree with package boundary awareness
- [ ] "Open in" launches configured external editor
- [ ] Zen mode hides all chrome
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-22.md`

---

## Phase 23: Review & Code Quality

**Goal**: Code review tooling and CI feedback loop. Closes issues #49, #51, #61, #67, #68, #70.

### Tasks
1. **Integrated diff viewer** (#49) (`src/ui/diff_viewer.rs`)
   - `d` from sidebar opens full-screen diff panel showing `git diff HEAD`
   - Syntax-highlighted hunk display with line numbers
   - Incremental context expansion: `+`/`-` to show more/fewer context lines
   - Per-file navigation: `]f`/`[f`

2. **Mark files as viewed** (#51)
   - In diff viewer, `v` marks current file as viewed (stored in `WorkspaceState`)
   - On subsequent push (new commits detected), auto-clear viewed flags for changed files
   - Viewed files shown with checkmark; unreviewed with dot

3. **Customizable reviewer agents** (#61)
   - `reviewers: Option<Vec<ReviewerConfig>>` in `Config`
   - Each reviewer: `name`, `prompt`, `scope` (file globs)
   - `R` (capital) from sidebar triggers reviewer: writes prompt + diff to agent terminal
   - Per-repo config override via `.composer/reviewers.toml` in worktree root

4. **Checks tab** (#67) (`src/ui/checks_tab.rs`)
   - `Tab` key opens Checks tab aggregating: git status, CI pipeline, deployment states, user TODOs
   - Git status section: branch, ahead/behind, dirty files count
   - CI section: reuses check data from `GhStatusFetcher`
   - Deployments: configurable via `deployments: Vec<DeploymentCheck>` in config (runs shell command, shows stdout)
   - TODOs section: linked to todo list from #68

5. **Todo list with merge-blocking** (#68)
   - `t` from sidebar opens todo list panel
   - Add/remove/check items with `a`/`d`/`Space`
   - `merge_requires_todos_complete: bool` config option
   - If enabled and unchecked todos exist, PR merge action shows warning and requires confirmation

6. **Forward failing CI to agent** (#70)
   - When CI check fails (detected by `GhStatusFetcher`), show option to forward to agent
   - `f` on a failed check in Checks tab: writes check name + log excerpt to agent terminal
   - Formats as: `"CI check '{name}' failed:\n{log_excerpt}\nPlease analyze and fix."`

### Files
- Create: `src/ui/diff_viewer.rs`, `src/ui/checks_tab.rs`, `src/ui/todo_panel.rs`
- Modify: `src/config.rs`, `src/workspace.rs`, `src/state.rs`, `src/gh_status.rs`, `src/app.rs`, `src/main.rs`, `src/ui/mod.rs`

### Success Criteria
- [ ] Diff viewer with incremental context expansion
- [ ] Viewed file tracking with auto-reset on push
- [ ] Reviewer agent trigger with custom prompts
- [ ] Checks tab aggregates git/CI/deploy/todos
- [ ] Todo list with optional merge-blocking
- [ ] Failed CI forwarded to agent on demand
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-23.md`

---

## Phase 24: Agent Collaboration & Context

**Goal**: Multi-agent workflows and context awareness. Closes issues #39, #45, #46, #48, #58, #71, #74, #75.

### Tasks
1. **Hand off plans between agents** (#39)
   - `H` from sidebar: opens a structured plan editor
   - Plan content written to `.composer/plan.md` in worktree
   - Agent in target workspace notified with message: `"Incoming plan from workspace '{name}': see .composer/plan.md"`
   - Target workspace picker if multiple workspaces open

2. **Multiple chat tabs per workspace** (#45)
   - Extend Phase 16's tab model with a `title: Option<String>` field
   - Auto-generate title from first agent response (first 6 words)
   - `Ctrl+N` creates new chat tab (spawns fresh agent process)
   - Tab bar shows auto-generated titles, truncated

3. **Interactive planning mode** (#46)
   - Detect agent output containing structured plan markers (e.g. `## Plan` header or `- [ ]` blocks)
   - Surface plan in a side panel with approve/reject/feedback controls
   - Approve: write `\napprove\n` to terminal; Reject: prompt for feedback text first
   - `plan_detection: bool` config toggle (default true)

4. **Task/checklist tracking** (#48)
   - Parse `- [ ]` / `- [x]` lines from agent terminal output in real time
   - Display live checklist in a small overlay at corner of main panel
   - Auto-dismiss when all items checked; toggle visibility with `k`

5. **Context usage meter** (#58)
   - Parse Claude's context usage from terminal output (looks for token count patterns)
   - Display compact progress bar in status bar: `ctx [████░░] 45%`
   - Color: green <50%, yellow 50-80%, red >80%
   - `context_meter: bool` config toggle (default true)

6. **Context sharing via notes** (#71)
   - `N` from sidebar opens a shared notes panel
   - Notes stored in `~/.composer/notes/{workspace_id}.md`
   - Any workspace can view notes from any other workspace via a picker
   - Agent can be given notes context via `n` in notes panel: writes note content to agent terminal

7. **Unread count tracking** (#74)
   - Track new terminal output lines since last time workspace was active/focused
   - Display unread badge `(3)` next to workspace name in sidebar
   - Clear on focus; increment on background output
   - `unread_badges: bool` config toggle (default true)

8. **Cost tracking** (#75)
   - Parse cost information from Claude's terminal output (looks for `$X.XX` patterns near API response markers)
   - Accumulate per-response cost and cumulative workspace total
   - Display in status bar when workspace is focused: `$0.04 | Σ $1.23`
   - `cost_tracking: bool` config toggle (default true)

### Files
- Create: `src/ui/plan_panel.rs`, `src/ui/notes_panel.rs`, `src/ui/task_overlay.rs`, `src/cost_tracker.rs`
- Modify: `src/workspace.rs`, `src/tab.rs`, `src/state.rs`, `src/config.rs`, `src/app.rs`, `src/main.rs`, `src/ui/mod.rs`, `src/ui/sidebar.rs`, `src/ui/status_bar.rs`, `src/ui/tab_bar.rs`

### Success Criteria
- [ ] Plan handoff writes to target workspace and notifies agent
- [ ] Multiple chat tabs with auto-generated titles
- [ ] Planning mode surfaces plan for approval/feedback
- [ ] Task checklist parsed and displayed live
- [ ] Context usage meter in status bar
- [ ] Cross-workspace notes readable and shareable
- [ ] Unread badges update in sidebar
- [ ] Cost tracking accumulates and displays
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-24.md`

---

## Phase 25: MCP Integration

**Goal**: Full MCP server lifecycle management. Closes issues #56, #60, #62.

### Tasks
1. **Discover and run MCP servers** (#56) (`src/mcp.rs`)
   - Scan common MCP config locations: `~/.config/claude/mcp.json`, `.mcp.json` in worktree root (#60)
   - Parse server definitions: command, args, env vars
   - Start/stop/restart MCP server processes (tracked as child processes alongside PTY)
   - `McpManager`: owns server lifecycle, exposes `start(name)`, `stop(name)`, `restart(name)`, `status(name)`

2. **Recognize .mcp.json** (#60)
   - Parse `.mcp.json` from project root on workspace open
   - Schema: `{ "servers": { "<name>": { "command": "...", "args": [...], "env": {...} } } }`
   - Merge project-level servers with user-level config (project takes precedence on name collision)

3. **Show MCP server status** (#62) (`src/ui/mcp_panel.rs`)
   - `m` from sidebar opens MCP panel showing all configured servers
   - Per-server: name, status (running/stopped/error), last error if any
   - `s` to start, `x` to stop, `r` to restart selected server
   - Status indicator in sidebar (small `[M]` badge) when any MCP server is running

### Files
- Create: `src/mcp.rs`, `src/ui/mcp_panel.rs`
- Modify: `src/app.rs`, `src/workspace.rs`, `src/config.rs`, `src/main.rs`, `src/ui/mod.rs`, `src/ui/sidebar.rs`, `src/lib.rs`

### New Dependencies
```toml
serde_json = "1"   # for .mcp.json parsing (likely already indirect dep)
```

### Success Criteria
- [ ] `.mcp.json` parsed and servers discovered
- [ ] MCP server processes start/stop/restart cleanly
- [ ] MCP panel shows per-server status
- [ ] Sidebar badge when servers running
- [ ] Server state does not block app shutdown
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-25.md`

---

## Phase 26: Linear & External Services

**Goal**: Linear integration and advanced workspace seeding. Closes issues #55, #59, #63, #85 (env isolation finalized here if not done in phase 21).

### Tasks
1. **Connect Linear workspace** (#55) (`src/linear.rs`)
   - `linear_api_key: Option<String>` in `Config` (read from env `LINEAR_API_KEY` or config)
   - `LinearClient`: wraps Linear GraphQL API for issue list/search
   - `L` from sidebar opens Linear panel: browse assigned issues, search by title/ID
   - Cache results locally with 60s TTL to avoid repeated API calls

2. **Create workspaces from Linear issues** (#59)
   - In workspace creation flow (`n`), add "From Linear issue" option (requires Linear connected)
   - Pre-populates: workspace name from issue title, branch name from issue identifier, agent context with issue body
   - Writes issue details to `.composer/context.md` in worktree

3. **Attach Linear issues to workspace** (#63)
   - `l` from sidebar (within a workspace): opens Linear issue picker
   - Attaches issue ID to workspace; displays issue title in sidebar below workspace name
   - Writes issue body to agent terminal as context message
   - `issue_id: Option<String>` added to `WorkspaceState`

4. **Linear status sync**
   - When workspace PR is merged (detected by `GhStatusFetcher`), optionally update attached Linear issue to "Done"
   - `linear_auto_close: bool` config toggle (default false)
   - Prompt user before updating if `linear_auto_close` is false

### Files
- Create: `src/linear.rs`, `src/ui/linear_panel.rs`
- Modify: `src/config.rs`, `src/workspace.rs`, `src/state.rs`, `src/app.rs`, `src/main.rs`, `src/ui/mod.rs`, `src/ui/sidebar.rs`, `src/lib.rs`

### New Dependencies
```toml
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tokio = { version = "1", features = ["rt-multi-thread"] }  # for async HTTP
```

### Success Criteria
- [ ] Linear API key configured and workspace connected
- [ ] Issue browser with search in TUI
- [ ] Workspace created from Linear issue with context pre-populated
- [ ] Linear issue attached to existing workspace
- [ ] Optional auto-close of Linear issue on PR merge
- [ ] Graceful degradation when Linear not configured
- [ ] `cargo clippy` + `cargo fmt --check` pass

### Handoff
Write `.context/handoff-phase-26.md`

---

## New Dependencies

```toml
# Phase 25
serde_json = "1"                    # .mcp.json parsing

# Phase 26
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tokio = { version = "1", features = ["rt-multi-thread"] }

# All other phases build on existing deps:
# ratatui, crossterm, git2, portable-pty, vte, serde, toml, directories, walkdir
# gh CLI integration shells out to `gh` rather than adding HTTP deps
```

---

## New/Modified Files

### New Modules
```
src/
├── mcp.rs                        # MCP server lifecycle manager (phase 25)
├── linear.rs                     # Linear API client (phase 26)
├── cost_tracker.rs               # Per-response and cumulative cost parsing (phase 24)
└── ui/
    ├── changes_panel.rs          # Uncommitted/committed file changes (phase 19)
    ├── pr_panel.rs               # PR creation/merge flow (phase 20)
    ├── checks_panel.rs           # GitHub Actions check details (phase 20)
    ├── workspace_search.rs       # Branch/repo/PR search overlay (phase 21)
    ├── archive_overlay.rs        # Archived workspace browser (phase 21)
    ├── terminal_search.rs        # Ctrl+F terminal output search (phase 22)
    ├── file_tree.rs              # Monorepo-aware file explorer (phase 22)
    ├── diff_viewer.rs            # Inline diff with incremental context (phase 23)
    ├── checks_tab.rs             # Aggregated git/CI/deploy/todo view (phase 23)
    ├── todo_panel.rs             # Merge-blocking todo list (phase 23)
    ├── plan_panel.rs             # Agent plan approval/feedback UI (phase 24)
    ├── notes_panel.rs            # Cross-workspace shared notes (phase 24)
    ├── task_overlay.rs           # Live task checklist overlay (phase 24)
    ├── mcp_panel.rs              # MCP server management UI (phase 25)
    └── linear_panel.rs           # Linear issue browser (phase 26)
```

---

## Dependency Graph

```
Phase 19 (Git Workflow)
    │
    ▼
Phase 20 (PR Lifecycle) ── uses GhStatusFetcher from phase 17
    │
    ▼
Phase 21 (Workspace Organization) ── uses archive, PR detection from phase 20
    │
    ▼
Phase 22 (Navigation & Discovery) ── uses file tree, zen mode
    │
    ▼
Phase 23 (Review & Code Quality) ── uses diff, checks from phase 20, todos
    │
    ▼
Phase 24 (Agent Collaboration) ── uses tabs from phase 16, notes, cost, unread
    │
    ▼
Phase 25 (MCP Integration) ── foundational for tool-aware agents
    │
    ▼
Phase 26 (Linear & External) ── uses workspace creation from phase 21, PR detection
```

---

## Verification

After each phase:
1. `direnv exec . cargo build` - compiles
2. `direnv exec . cargo test` - tests pass
3. `direnv exec . cargo clippy` - no warnings
4. `direnv exec . cargo fmt --check` - formatted
5. Manual testing per phase success criteria

### End-to-End Test (after Phase 26)
1. Connect Linear, open issue, create workspace from it — context pre-populated
2. Agent makes changes; see live task checklist overlay
3. `d` opens diff viewer; mark files as viewed
4. Commit and push shortcut; CI check fails
5. Forward failing CI to agent from Checks tab
6. Agent fixes; CI passes; PR auto-created with template
7. PR merged; workspace auto-archived; Linear issue closed
8. `\` search finds archived workspace; restore with git state intact
9. `m` opens MCP panel; start a server; agent uses it
10. `Z` zen mode for focused terminal review
