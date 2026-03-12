# Future Features

Desired features for the TUI app, roughly ordered by priority.

---

## Command Palette & Search

- **Command palette** (Cmd+K / Ctrl+K) with fuzzy search over all available actions
- **Workspace quick-switch** overlay for fast jumping between workspaces
- **Chat/terminal search** (Ctrl+F) with forward/backward navigation and highlighting
- **File picker** with fuzzy search (Ctrl+P) to open files in an editor

## Workspace Management

- **Create workspaces from PRs, branches, or Linear issues** — not just from scratch
- **Workspace status grouping** — organize by backlog / in progress / in review / done
- **Group by repository** and pin important workspaces
- **Archive/unarchive** workspaces with auto-save of git state
- **Auto-archive on PR merge**
- **Fork workspace** with chat summary carried over
- **Workspace search** by branch name, repo, or PR number
- **Workspace reordering** via drag or keyboard
- **Broadcast input mode** — send keystrokes to all terminals simultaneously

## Agent & Model Management

- **Multi-model support** — switch between Claude, GPT, and other providers
- **Quick model switching** via number keys or picker
- **Switch agents mid-chat** with automatic context summaries
- **Hand off plans between agents** in the same workspace
- **Custom provider support** for any Anthropic-compatible endpoint
- **AWS Bedrock and Google Vertex AI** provider backends

## Chat & Conversation UI

- **Structured chat view** — render agent messages, tool calls, and user input as a conversation rather than raw terminal output
- **Multiple chat tabs** per workspace with auto-generated titles
- **Interactive planning mode** with approval/feedback prompts surfaced in the UI
- **Task/checklist tracking** within a workspace
- **Checkpointing** — revert to a previous conversation turn; export to new chat
- **Copy chat as markdown**
- **Chat summaries on hover** and table of contents for long conversations
- **Context usage meter** with token breakdown

## Code Editing & Review

- **Built-in file editor** with syntax highlighting and search
- **Integrated diff viewer** with incremental expansion
- **Mark files as viewed** with auto-re-flagging on change
- **Comment on diffs** with markdown formatting and multiline support
- **GitHub comment syncing** with author avatars
- **Customizable reviewer agents** and per-repo review prompts
- **File tree explorer** with monorepo-aware navigation

## Git & GitHub Integration

- **Changes panel** showing uncommitted vs. committed changes grouped by file
- **Target branch selection** changeable mid-workspace
- **Commit and push** shortcut
- **Merge conflict resolution** prompts surfaced in the UI
- **Create/merge PRs** with customizable templates and editable descriptions
- **PR status checks monitoring** with GitHub Actions logs and re-run buttons
- **Graphite stack support** with sidebar visualization
- **Rebase detection** and auto-detect merge vs. rebase preference
- **Custom git branch prefix** configuration

## Linear Integration

- Connect a Linear workspace and browse/search issues
- Create workspaces directly from Linear issues
- Attach Linear issues to a workspace chat

## MCP (Model Context Protocol)

- Discover and run MCP servers
- Recognize `.mcp.json` configuration files
- Show MCP status before sending messages
- Visualize agent tool usage

## Notes & Scratch

- **Workspace scratchpad** with markdown preview
- **Todo list** with merge-blocking (must complete items before merging)
- **Context sharing** via notes between agents

## Checks & Monitoring

- **Checks tab** aggregating git status, CI, deployments, and user TODOs
- **Forward failing CI checks** to the agent for analysis
- **Service outage detection** (Anthropic, GitHub)
- **Cost tracking** per response and cumulative per workspace

## Appearance & Theming

- **Light/dark mode toggle** with accent color customization
- **Custom monospace font** selection
- **Zen mode** — hide all chrome except the active terminal
- **Mermaid diagram rendering** with pan/zoom
- **LaTeX rendering** support
- **Zoom controls**
- **Configurable chat width**

## Notifications

- **Desktop toast notifications** linking back to the relevant workspace
- **Completion and background sound effects** (configurable)
- **Unread count tracking** and sidebar notification badges for workspaces awaiting input

## IDE Integration

- **"Open in" support** for VS Code, Cursor, Zed, IntelliJ, Xcode, Fork, etc.
- **Deep linking** from external tools (Linear, Slack, VS Code) into specific workspaces

## Privacy & Enterprise

- **Enterprise data privacy mode** toggle (also configurable via shared config)
- **Workspace-specific environment isolation**
