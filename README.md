# claude-cage

TUI for managing multiple [Claude Code](https://claude.com/claude-code) sessions and orchestrated agent tasks in tmux.

![Rust](https://img.shields.io/badge/rust-stable-orange) ![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Session overview** — see all running Claude instances across tmux sessions at a glance
- **Task view** — monitor orchestrated agent teams with live status, output, and progress
- **Live status** — working / idle / waiting indicators via Claude Code hooks
- **Truecolor preview** — full RGB ANSI rendering of pane output
- **Send prompts remotely** — fire off tasks to any Claude session without switching
- **Task chat & nudges** — communicate with agents or redirect them mid-task
- **Worktree launcher** — spin up a new `claude --worktree` session from the manager
- **Skills** — saved commands you can fire at any session (e.g., `/orchestrate`, `/implement`)
- **Persistent view** — remembers your last view (Sessions/Tasks) across restarts

## Install

```bash
cargo install --path .
```

Or build from source:

```bash
git clone https://github.com/wrdna/claude-cage.git
cd claude-cage
cargo build --release
ln -sf $(pwd)/target/release/claude-cage ~/.local/bin/claude-cage
```

## Setup

### tmux plugin (TPM)

Add to `~/.tmux.conf`:

```tmux
set -g @plugin 'wrdna/claude-cage'
```

Or load manually:

```tmux
run '~/.tmux/plugins/claude-cage/claude-cage.tmux'
```

Default keybindings: `prefix + m` opens the manager, `prefix + c` opens a quick Claude popup.

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

Open with `prefix + m` (or however you bound it). Press `t` to toggle between Sessions and Tasks views.

### Sessions view

| Key | Action |
|-----|--------|
| `j`/`k` | Navigate sessions |
| `Enter` | Switch to session |
| `c` | Chat — forward keystrokes to selected session |
| `n` | New Claude session |
| `w` | Create new worktree session |
| `S` | Open skill picker |
| `K` | Kill session (with confirmation) |
| `t` | Switch to Tasks view |
| `r` | Refresh |
| `q` / `Esc` | Quit |

### Tasks view

| Key | Action |
|-----|--------|
| `j`/`k` | Navigate task tree |
| `Enter` / `Space` | Expand/collapse subtasks, or switch to linked session |
| `c` | Chat with task's linked session (or nudge if no session) |
| `m` | Nudge — send a direction change to the agent |
| `Ctrl+u`/`Ctrl+d` | Scroll output/preview |
| `t` | Switch to Sessions view |
| `r` | Refresh |
| `q` / `Esc` | Quit |

## Task tracking CLI

Agents use the `claude-cage task` CLI to register progress visible in the TUI:

```bash
# Initialize a top-level orchestration task
claude-cage task init <id> <name> [--role <role>] [--pane-id <pane>]

# Add a subtask under a parent
claude-cage task add <parent-id> <id> <name> [--role <role>] [--pane-id <pane>]

# Update task status
claude-cage task status <id> <pending|in_progress|completed|failed>

# Set task output (replaces)
claude-cage task output <id> <text...>

# Append a line to task output (for streaming updates like training metrics)
claude-cage task append <id> <text...>

# Check for user nudges (reads and consumes)
claude-cage task nudge <id>

# List all tasks as JSON
claude-cage task list

# Clear all tasks
claude-cage task clear
```

### Example: training task with streaming output

```bash
TASK_ID="train-$(date +%s)"
claude-cage task init "$TASK_ID" "Train dodge curriculum v8" --role implement
claude-cage task status "$TASK_ID" in_progress

# Agent appends progress as training runs
claude-cage task append "$TASK_ID" "Step 1000 | Loss: 0.234 | Acc: 45.2% | 1.2M SPS"
claude-cage task append "$TASK_ID" "Step 2000 | Loss: 0.198 | Acc: 52.1% | 1.3M SPS"
claude-cage task append "$TASK_ID" "Step 3000 | Loss: 0.156 | Acc: 61.8% | 1.3M SPS"

# Output persists after completion
claude-cage task status "$TASK_ID" completed
claude-cage task append "$TASK_ID" "Final: 75.6% accuracy at 500M steps"
```

## Status indicators

| Symbol | State | Color |
|--------|-------|-------|
| `●` | Working | Yellow |
| `○` | Idle | Green |
| `◐` | Waiting for input | Magenta |
| `?` | Unknown | Gray |

### Task status

| Symbol | Status | Color |
|--------|--------|-------|
| `☐` | Pending | Gray |
| `◑` | In Progress | Yellow |
| `☑` | Completed | Green |
| `☒` | Failed | Red |

### Agent roles

| Role | Color |
|------|-------|
| `orchestrate` | Cyan |
| `implement` | Green |
| `research` | Yellow |
| `review` | Blue |
| `security` | Red |
| `test-gen` | Magenta |
| `architect` | Cyan |

## How it works

claude-cage reads tmux pane metadata to find all panes running `claude`, reads state files written by Claude Code hooks, and renders everything in a ratatui TUI. The preview panel captures pane output with full ANSI escape sequence parsing for truecolor rendering.

The task system uses a JSON file (`~/.cache/claude-cage/tasks.json`) as shared state between the TUI and agent processes. Nudges use file-based IPC via `~/.cache/claude-cage/nudges/<task-id>.txt` — the TUI writes, the agent reads and consumes.

## License

MIT
