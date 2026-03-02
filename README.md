```
 _                              _
| |_  ___  _ _  _ __   ___  ___| |_
|  _|/ -_)| '_|| '  \ / -_)(_-<| ' \
 \__|\___||_|  |_|_|_|\___||__/|_||_|
```

# Termesh

AI agent control tower + GPU-accelerated terminal multiplexer.

Run multiple AI coding agents (Claude Code, Codex, Gemini) side by side in a single workspace with real-time status tracking and code diff visualization.

## Features

- **Multi-agent orchestration** — Run Claude, Codex, Gemini, or plain shell sessions simultaneously
- **Focus / Split mode** — Single session view or multi-pane split layout, toggle with `Ctrl+Enter`
- **Agent state detection** — Automatically detects Idle, Thinking, Tool Use, Error states from PTY output
- **Session swap** — Swap sessions between panes with a visual picker (`Ctrl+S`)
- **Live diff panel** — Side panel shows git-based real-time file changes per session
- **Session management** — Create, switch, rename, and close sessions with keyboard shortcuts
- **GPU-accelerated rendering** — wgpu-based renderer with CJK and emoji fallback fonts
- **Cross-platform** — Windows, macOS (Intel & Apple Silicon), Linux

## Installation

### Download

Pre-built binaries are available from [GitHub Releases](https://github.com/ckr3453/termesh/releases).

| Platform | Download |
|----------|----------|
| Windows x86_64 | `termesh-windows-x86_64.tar.gz` |
| macOS Intel | `termesh-macos-x86_64.tar.gz` |
| macOS Apple Silicon | `termesh-macos-aarch64.tar.gz` |
| Linux x86_64 | `termesh-linux-x86_64.tar.gz` |

### Build from source

```bash
# Requires Rust 1.70+
cargo build --release
# Binary at target/release/termesh
```

## Usage

```bash
# Launch with agent picker
termesh

# Start with a specific agent
termesh --agent claude
termesh --agent codex
termesh --agent gemini
termesh --agent shell
```

## Keyboard Shortcuts

> macOS uses `Cmd` instead of `Ctrl` for all shortcuts below.

### Session

| Shortcut | Action |
|----------|--------|
| `Ctrl+N` | New session |
| `Ctrl+W` | Close session |
| `Ctrl+1`~`9` | Switch to session N |
| `Ctrl+[` | Previous session |
| `Ctrl+]` | Next session |
| `Ctrl+R` | Rename session |
| `Ctrl+B` | Toggle session list |
| `Ctrl+S` | Swap session in pane (Split mode) |

### Layout

| Shortcut | Action |
|----------|--------|
| `Ctrl+Enter` | Toggle Focus / Split mode |
| `Ctrl+T` | Split horizontal |
| `Ctrl+Shift+T` | Split vertical |
| `Ctrl+1`~`4` | Focus pane N (Split mode) |

### Side Panel (Focus mode)

| Shortcut | Action |
|----------|--------|
| `Ctrl+E` | Toggle diff panel |
| `Ctrl+D` | Toggle diff mode (Unified / Side-by-side) |
| `Ctrl+Shift+Up` | Scroll up |
| `Ctrl+Shift+Down` | Scroll down |
| `Ctrl+Shift+Enter` | Select file / Enter diff |
| `Ctrl+Shift+[` | Back to file list |

### Clipboard

| Shortcut | Action |
|----------|--------|
| `Ctrl+Shift+C` | Copy selection |
| `Ctrl+Shift+V` | Paste |

## Architecture

```
termesh/
├── termesh-core        # Shared types, config, platform utilities
├── termesh-pty         # PTY spawn/management, session lifecycle
├── termesh-terminal    # Terminal emulation (alacritty_terminal)
├── termesh-renderer    # GPU rendering (wgpu)
├── termesh-layout      # Pane split, focus/split layout
├── termesh-input       # Keybinding, input handling
├── termesh-agent       # Agent adapters, state detection
├── termesh-diff        # Git change tracking, diff generation
├── termesh-platform    # Platform-specific native layer (winit)
└── termesh-app         # App entry point, event loop
```

## Requirements

- GPU with Vulkan, Metal, or DX12 support
- AI agents installed separately (e.g., `npm install -g @anthropic-ai/claude-code`)

## License

MIT
