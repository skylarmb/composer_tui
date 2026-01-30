# composer_tui

A terminal-based tool for orchestrating multiple parallel AI coding agents, using git worktrees to isolate each agent's work.

Built with Rust and [ratatui](https://ratatui.rs/).

## For Agents

Before starting work, read these docs:

1. **[docs/DESIGN.md](docs/DESIGN.md)** - Architecture, data model, and UI layout
2. **[docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md)** - Phased implementation with success criteria

If continuing from a previous phase, also read the relevant handoff doc in `.context/`.

Workflow guidance:
- Commit and push your changes after each coherent set of edits (e.g., finishing a phase task or feature slice). This keeps parallel agents in sync and simplifies handoffs.

## Development

Commands run through direnv to ensure correct environment:

```bash
direnv exec . cargo build      # Build
direnv exec . cargo run        # Run
direnv exec . cargo test       # Test
direnv exec . cargo clippy     # Lint
direnv exec . cargo fmt        # Format
```

## Project Status

See `docs/IMPLEMENTATION_PLAN.md` for current phase and progress.
