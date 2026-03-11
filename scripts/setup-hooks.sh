#!/usr/bin/env bash

# Add claude-cage hooks to Claude Code settings if not already configured.
# This enables live status tracking (working/idle/waiting) for each pane.

SETTINGS_FILE="$HOME/.claude/settings.json"

# Skip if already configured
if [ -f "$SETTINGS_FILE" ] && grep -q "claude-cage state" "$SETTINGS_FILE" 2>/dev/null; then
    exit 0
fi

mkdir -p "$HOME/.claude"

# If settings file doesn't exist, create it with hooks
if [ ! -f "$SETTINGS_FILE" ]; then
    cat > "$SETTINGS_FILE" << 'SETTINGS'
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          { "type": "command", "command": "claude-cage state working" }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          { "type": "command", "command": "claude-cage state idle" }
        ]
      }
    ],
    "Notification": [
      {
        "matcher": "permission_prompt",
        "hooks": [
          { "type": "command", "command": "claude-cage state waiting" }
        ]
      }
    ]
  }
}
SETTINGS
    echo "claude-cage: created hooks in $SETTINGS_FILE"
    exit 0
fi

# Settings exist but no claude-cage hooks — notify user
if ! grep -q "claude-cage" "$SETTINGS_FILE" 2>/dev/null; then
    echo "claude-cage: hooks not found in $SETTINGS_FILE"
    echo "claude-cage: add the following hooks manually for live status tracking:"
    echo '  "UserPromptSubmit": [{"matcher":"","hooks":[{"type":"command","command":"claude-cage state working"}]}]'
    echo '  "Stop": [{"matcher":"","hooks":[{"type":"command","command":"claude-cage state idle"}]}]'
    echo '  "Notification": [{"matcher":"permission_prompt","hooks":[{"type":"command","command":"claude-cage state waiting"}]}]'
fi
