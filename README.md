# composer_tui

# Goals

- Orchestrate multiple parallel AI coding agents.
- Use disposable git worktrees as a workspace for each agent to isolate their work.

# Requirements

- Written in Rust
- Uses the `ratatui` TUI library
- Has a gorgeous TUI interface
- vim-like key bindings for navigating the TUI

# Appearance

- sidebar with items for each active workspace (W1, W2, W3 are placeholders representing some workspaces)
- the rest of the UI is the main content area, which is just a shell for now.

```
┌────────────────────────────────────────────────────────────────────────────────┐
│                                 composer_tui                                   │
├──────────┬─────────────────────────────────────────────────────────────────────┤
│┌────────┐│                                                                     │
││   W1   ││                                                                     │
│└────────┘│                                                                     │
│┌────────┐│                                                                     │
││   W2   ││                                                                     │
│└────────┘│                                                                     │
│┌────────┐│                     Main Content Area                               │
││   W3   ││                   (interactive terminal / shell )                   │
│└────────┘│                                                                     │
│          │                                                                     │
│          │                                                                     │
│          │                                                                     │
│          │                                                                     │
│          │                                                                     │
│          │                                                                     │
│          │                                                                     │
│          │                                                                     │
└──────────┴─────────────────────────────────────────────────────────────────────┘
```

# Development

- Use TDD and focus on high test coverage
- Use a nix flake / nix devshell and direnv to isolate project dev environment from the system
- Approach feature development iteratively, checking your work along the way. Do not "1-shot" features.
