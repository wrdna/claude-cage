use std::process::Command;

use crate::session::Session;

fn run(args: &[&str]) -> String {
    Command::new("tmux")
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

pub fn list_claude_sessions() -> Vec<Session> {
    let raw = run(&[
        "list-panes",
        "-a",
        "-F",
        "#{session_name}:#{window_index}.#{pane_index} #{pane_id} #{pane_current_command} #{pane_current_path} #{session_attached} #{window_active} #{pane_active}",
    ]);

    raw.lines().filter_map(Session::from_tmux_line).collect()
}

pub fn capture_pane(pane_id: &str, lines: usize) -> Vec<String> {
    let raw = run(&[
        "capture-pane",
        "-t",
        pane_id,
        "-p",
        "-e",
        "-S",
        &format!("-{}", lines),
    ]);
    raw.lines().map(String::from).collect()
}

pub fn switch_to(addr: &str) {
    run(&["switch-client", "-t", addr]);
}

pub fn send_keys(pane_id: &str, text: &str) {
    run(&["send-keys", "-t", pane_id, text, "Enter"]);
}

/// Send a raw tmux key name (e.g. "Enter", "BSpace", "C-c", "Up")
pub fn send_raw_key(pane_id: &str, key: &str) {
    run(&["send-keys", "-t", pane_id, key]);
}

/// Send a literal character (handles special chars that tmux interprets)
pub fn send_literal(pane_id: &str, c: char) {
    let s = c.to_string();
    run(&["send-keys", "-t", pane_id, "-l", &s]);
}

pub fn kill_pane(pane_id: &str) {
    run(&["kill-pane", "-t", pane_id]);
}

pub fn new_window(cmd: &str) {
    // -- separates tmux flags from the shell command
    // bash -lc ensures login shell so nvm/PATH are loaded
    run(&["new-window", "-n", "claude", "--", "bash", "-lc", cmd]);
}

/// Find the full path to the claude binary.
pub fn claude_bin() -> String {
    // Check common locations
    for path in &[
        // Direct which lookup (works if current shell has it)
        "",
        // nvm default
        "/home/wrdna/.nvm/versions/node/v18.20.8/bin/claude",
        // Global npm
        "/usr/local/bin/claude",
        "/usr/bin/claude",
    ] {
        if path.is_empty() {
            // Try which
            if let Ok(output) = std::process::Command::new("which")
                .arg("claude")
                .output()
            {
                if output.status.success() {
                    let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !p.is_empty() {
                        return p;
                    }
                }
            }
        } else if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }
    // Fallback — hope it's on PATH in the new shell
    "claude".to_string()
}
