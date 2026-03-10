# claude-cage

TUI for managing multiple [Claude Code](https://claude.com/claude-code) sessions in tmux.

![Rust](https://img.shields.io/badge/rust-stable-orange) ![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Session overview** — see all running Claude instances across tmux sessions at a glance
- **Grouped by project & worktree** — sessions organized by repo and git worktree/branch
- **Live status** — working / idle / waiting indicators via Claude Code hooks
- **Truecolor preview** — full RGB ANSI rendering of pane output
- **Send prompts remotely** — fire off tasks to any Claude session without switching
- **Worktree launcher** — spin up a new `claude --worktree` session from the manager
- **Title & task tracking** — see what each session is working on and its latest task

## Install

```bash
cargo install --path .
```

Or build from source:

```bash
git clone https://github.com/wrdna/claude-cage.git
cd claude-cage
cargo build --release
cp target/release/claude-cage ~/.local/bin/
```

## Setup

### tmux keybinding

Add to `~/.tmux.conf`:

```tmux
bind m display-popup -E -w 80% -h 60% "claude-cage"
```

Reload with `prefix + r` or `tmux source-file ~/.tmux.conf`.

### Status hooks

Add to `~/.claude/settings.json` to enable live status tracking:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          { "type": "command", "command": "claude-state working" }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          { "type": "command", "command": "claude-state idle" }
        ]
      }
    ],
    "Notification": [
      {
        "matcher": "permission_prompt",
        "hooks": [
          { "type": "command", "command": "claude-state waiting" }
        ]
      }
    ]
  }
}
```

Install the `claude-state` helper:

```bash
#!/bin/bash
# Save as ~/.local/bin/claude-state and chmod +x
STATE_DIR="$HOME/.cache/claude-pane-states"
mkdir -p "$STATE_DIR"
[ -z "$TMUX_PANE" ] && exit 0
INPUT=$(cat)
echo "$1" > "$STATE_DIR/${TMUX_PANE}.state"
if [ "$1" = "working" ]; then
    PROMPT=$(echo "$INPUT" | python3 -c "
import sys, json
try:
    d = json.loads(sys.stdin.read())
    p = d.get('prompt', '') or d.get('message', '') or d.get('input', '') or ''
    print(p[:120])
except: pass
" 2>/dev/null)
    if [ -n "$PROMPT" ]; then
        [ ! -f "$STATE_DIR/${TMUX_PANE}.title" ] && echo "$PROMPT" > "$STATE_DIR/${TMUX_PANE}.title"
        echo "$PROMPT" > "$STATE_DIR/${TMUX_PANE}.task"
    fi
fi
```

## Usage

Open with `prefix + m` (or however you bound it).

| Key | Action |
|-----|--------|
| `j`/`k` | Navigate sessions |
| `Enter` | Switch to session |
| `s` | Send a prompt to selected session |
| `w` | Create new worktree session |
| `n` | New Claude session |
| `K` | Kill session (with confirmation) |
| `r` | Refresh |
| `q` / `Esc` | Quit |

## Status indicators

| Symbol | State | Color |
|--------|-------|-------|
| `●` | Working | Yellow |
| `○` | Idle | Green |
| `◐` | Waiting for input | Magenta |
| `?` | Unknown | Gray |

## How it works

claude-cage reads tmux pane metadata to find all panes running `claude`, reads state files written by Claude Code hooks, detects git branch/worktree info, and renders everything in a ratatui TUI. The preview panel captures pane output with full ANSI escape sequence parsing for truecolor rendering.

## License

MIT
