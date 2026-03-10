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

pub fn kill_pane(pane_id: &str) {
    run(&["kill-pane", "-t", pane_id]);
}

pub fn new_window(cmd: &str) {
    run(&["new-window", cmd]);
}
