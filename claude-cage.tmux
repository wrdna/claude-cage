#!/usr/bin/env bash

CURRENT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
BINARY="$CURRENT_DIR/target/release/claude-cage"

# Build if binary doesn't exist
if [ ! -f "$BINARY" ]; then
    "$CURRENT_DIR/scripts/install.sh"
fi

# Read user options with defaults
key=$(tmux show-option -gqv @claude-cage-key)
key=${key:-m}

width=$(tmux show-option -gqv @claude-cage-width)
width=${width:-90%}

height=$(tmux show-option -gqv @claude-cage-height)
height=${height:-80%}

popup_key=$(tmux show-option -gqv @claude-popup-key)
popup_key=${popup_key:-c}

# Bind the session manager popup
tmux bind-key "$key" display-popup -E -w "$width" -h "$height" "$BINARY"

# Bind the quick claude popup (bundled script)
tmux bind-key "$popup_key" display-popup -w 80% -h 80% -d "#{pane_current_path}" -E "$CURRENT_DIR/scripts/claude-popup"

# Set up pane border labels for claude panes
tmux set-option -g pane-border-status top
tmux set-option -g pane-border-format \
    " #[bold]#{?#{==:#{pane_current_command},claude},#[fg=colour34]Claude#[default],#{pane_current_command}}#[default] #{pane_current_path} "

# Install hooks into Claude Code settings if not already present
"$CURRENT_DIR/scripts/setup-hooks.sh"
