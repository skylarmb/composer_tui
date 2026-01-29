# composer_tui - MVP Design Document

## 1. Overview & Goals

### Problem Statement

When working with AI coding agents, it's often useful to run multiple agents in parallel on different tasks. However, managing multiple agents becomes chaotic:
- Agents can conflict if they modify the same files
- It's hard to monitor multiple agents simultaneously
- Context switching between agent sessions is cumbersome

### Solution

`composer_tui` is a terminal-based orchestration tool that:
- Provides isolated workspaces for each agent using git worktrees
- Offers a unified TUI to monitor and interact with all agents
- Uses vim-like keybindings for efficient navigation

### MVP Goal

Get a minimal TUI running with the project infrastructure in place. The MVP is a foundation to build on, not a usable product yet.

---

## 2. MVP Scope

### In Scope

| Feature | Description |
|---------|-------------|
| Project setup | Nix flake, direnv, cargo project structure |
| Basic TUI | Boots and renders the two-panel layout |
| Sidebar | Displays hardcoded placeholder workspace items |
| Main panel | Empty placeholder (no shell yet) |
| Navigation | j/k to move sidebar selection, q to quit |
| Tests | Unit tests for core components |

### Out of Scope (Future Work)

- Embedded terminal/PTY in main panel
- Git worktree creation/management
- Agent process spawning/orchestration
- Configuration file loading
- Persistent state
- Mouse support
- Resize handling (beyond ratatui defaults)

---

## 3. Architecture

### Component Overview

```
┌─────────────────────────────────────────────────────────┐
│                        main.rs                          │
│  - Parse args, initialize terminal, run event loop     │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                        App                              │
│  - Holds application state                              │
│  - Handles input events                                 │
│  - Delegates rendering to UI module                     │
└─────────────────────────────────────────────────────────┘
                            │
              ┌─────────────┴─────────────┐
              ▼                           ▼
┌─────────────────────┐     ┌─────────────────────────────┐
│     Workspace       │     │           UI                │
│  - id, name, status │     │  - render_sidebar()         │
│  (data struct)      │     │  - render_main_panel()      │
└─────────────────────┘     │  - render_header()          │
                            └─────────────────────────────┘
```

### Module Structure

```
src/
├── main.rs          # Entry point, terminal setup, event loop
├── app.rs           # App struct and state management
├── ui/
│   ├── mod.rs       # UI module, main render function
│   ├── sidebar.rs   # Sidebar widget
│   ├── main_panel.rs# Main content panel widget
│   └── header.rs    # Header bar widget
├── workspace.rs     # Workspace data structures
├── event.rs         # Input event handling
└── lib.rs           # Library root (for testing)
```

### Data Flow

1. `main.rs` sets up terminal and creates `App`
2. Event loop: poll for input → `App::handle_event()` → `App::render()`
3. `handle_event()` updates state based on keypress
4. `render()` calls UI functions with current state
5. Loop until quit signal

---

## 4. TUI Layout & Navigation

### Layout Structure

```
┌────────────────────────────────────────────────────────────────────────────────┐
│                                 composer_tui                                   │  <- Header (1 row)
├──────────┬─────────────────────────────────────────────────────────────────────┤
│          │                                                                     │
│  [W1]    │                                                                     │
│          │                                                                     │
│   W2     │                                                                     │
│          │                                                                     │
│   W3     │                     Main Content Area                               │  <- Main Panel
│          │                   (placeholder for MVP)                             │
│          │                                                                     │
│          │                                                                     │
│          │                                                                     │
│          │                                                                     │
│    ▲     │                                                                     │
│ Sidebar  │                                                                     │
└──────────┴─────────────────────────────────────────────────────────────────────┘
   (fixed     (flexible width)
    width)
```

### Widget Hierarchy

```
Frame
└── Layout (Vertical)
    ├── Header (height: 1)
    └── Layout (Horizontal)
        ├── Sidebar (width: 12, fixed)
        └── MainPanel (width: remaining)
```

### Keybindings (MVP)

| Key | Action |
|-----|--------|
| `j` / `↓` | Move selection down in sidebar |
| `k` / `↑` | Move selection up in sidebar |
| `q` | Quit application |
| `Esc` | Quit application |

---

## 5. Data Model

### Core Types

```rust
/// Application state container
pub struct App {
    /// List of workspaces (hardcoded for MVP)
    workspaces: Vec<Workspace>,
    /// Currently selected workspace index
    selected_index: usize,
    /// Whether the app should quit
    should_quit: bool,
}

/// Represents a single agent workspace
pub struct Workspace {
    /// Unique identifier
    id: String,
    /// Display name shown in sidebar
    name: String,
}
```

### MVP Initialization

For MVP, workspaces are hardcoded:

```rust
fn default_workspaces() -> Vec<Workspace> {
    vec![
        Workspace::new("1", "W1"),
        Workspace::new("2", "W2"),
        Workspace::new("3", "W3"),
    ]
}
```

---

## 6. Project Setup

### Directory Structure

```
composer_tui/
├── .envrc              # direnv config (use flake)
├── .gitignore
├── Cargo.toml
├── Cargo.lock
├── flake.nix           # Nix flake for dev environment
├── flake.lock
├── README.md
├── docs/
│   └── DESIGN.md       # This document
└── src/
    └── ...             # Source files (see Module Structure)
```

### Cargo.toml Dependencies

```toml
[package]
name = "composer_tui"
version = "0.1.0"
edition = "2021"

[dependencies]
ratatui = "0.29"        # TUI framework
crossterm = "0.28"      # Terminal backend

[dev-dependencies]
# Testing utilities (if needed)
```

### Nix Flake

The flake provides:
- Rust toolchain (stable)
- `cargo`, `rustc`, `rustfmt`, `clippy`
- Any system dependencies (none expected for MVP)

```nix
{
  description = "composer_tui development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
          ];
        };
      }
    );
}
```

### .envrc

```bash
use flake
```

### Development Workflow

```bash
# Enter dev environment (automatic with direnv)
cd composer_tui

# Build
cargo build

# Run
cargo run

# Test
cargo test

# Lint
cargo clippy

# Format
cargo fmt
```

---

## 7. Testing Strategy

### What to Test

| Component | Test Type | What to Verify |
|-----------|-----------|----------------|
| `App` | Unit | State transitions (selection up/down, quit flag) |
| `Workspace` | Unit | Construction, default values |
| `event` | Unit | Keypress → action mapping |
| UI rendering | Manual | Visual inspection during development |

### Example Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_wraps_down() {
        let mut app = App::new();
        app.selected_index = app.workspaces.len() - 1;
        app.select_next();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_selection_wraps_up() {
        let mut app = App::new();
        app.selected_index = 0;
        app.select_previous();
        assert_eq!(app.selected_index, app.workspaces.len() - 1);
    }

    #[test]
    fn test_quit_sets_flag() {
        let mut app = App::new();
        assert!(!app.should_quit);
        app.quit();
        assert!(app.should_quit);
    }
}
```

### Manual Testing Checklist

- [ ] App starts without error
- [ ] Layout matches mockup
- [ ] j/k navigation works and wraps
- [ ] Selection highlight is visible
- [ ] q/Esc quits cleanly (terminal restored)

---

## 8. Future Considerations

These are explicitly out of scope for MVP but worth noting for future design:

### UI Enhancements
- Status bar at bottom for keybinding hints, messages
- Workspace status indicators (active/idle/error with colors)
- Header content beyond app name

### Terminal Embedding
- Will need PTY handling (likely `portable-pty` crate)
- Each workspace will own a PTY instance
- Main panel renders PTY output

### Git Worktree Management
- Create worktrees in a designated directory
- Track worktree paths in Workspace struct
- Cleanup on workspace deletion

### Agent Orchestration
- Spawn agent processes in worktree directories
- Capture stdout/stderr
- Send input from TUI

### Configuration
- Config file for preferences
- Workspace persistence across sessions

### Additional Keybindings
- `Enter` to focus main panel / enter terminal mode
- `1-9` to jump to workspace by number
- `/` to search workspaces
- `n` to create new workspace
- `d` to delete workspace

---

## Appendix: Quick Reference

### Build Commands

```bash
cargo build          # Debug build
cargo build --release # Release build
cargo run            # Run debug
cargo test           # Run tests
cargo clippy         # Lint
cargo fmt            # Format
```

### Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point |
| `src/app.rs` | Application state |
| `src/ui/mod.rs` | Rendering logic |
| `flake.nix` | Dev environment |
