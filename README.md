# composer_tui

A terminal-based tool for orchestrating multiple parallel AI coding agents, using git worktrees to isolate each agent's work.

Built with Rust and [ratatui](https://ratatui.rs/).

## For Agents

Before starting work, read these docs:

1. **[docs/DESIGN.md](docs/DESIGN.md)** - Architecture, data model, and UI layout
2. **[docs/IMPLEMENTATION_PLAN_V3.md](docs/IMPLEMENTATION_PLAN_V3.md)** - Phased implementation with success criteria

If continuing from a previous phase, also read the relevant handoff doc in `.context/` (e.g. `.context/handoff-phase-15.md`).

### Phase workflow

1. Read the implementation plan and the previous phase's handoff doc.
2. Implement the phase, writing tests for all new functionality.
3. Verify your work passes all checks before considering the phase complete:
   ```bash
   direnv exec . cargo fmt        # Format code
   direnv exec . cargo clippy     # Lint (must be warning-free)
   direnv exec . cargo test       # All tests must pass
   ```
4. Write a handoff doc at `.context/handoff-phase-<N>.md` summarizing:
   - What was implemented
   - Files changed
   - Test summary (count, any notable coverage)
   - Any decisions or gotchas for the next phase
5. Commit and push your changes.

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

See `docs/IMPLEMENTATION_PLAN_V3.md` for current phase and progress.
