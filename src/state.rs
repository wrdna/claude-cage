use std::fs;
use std::io::Read;
use std::path::PathBuf;

fn state_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cache/claude-pane-states")
}

/// Handle the `claude-cage state <working|idle|waiting>` subcommand.
/// Reads JSON from stdin, writes state/title/task files.
/// Replaces the bash `claude-state` script entirely.
pub fn handle_state(state: &str) -> i32 {
    let pane_id = match std::env::var("TMUX_PANE") {
        Ok(id) if !id.is_empty() => id,
        _ => return 0, // Not in tmux, nothing to do
    };

    let dir = state_dir();
    if fs::create_dir_all(&dir).is_err() {
        return 1;
    }

    // Write state file
    let state_file = dir.join(format!("{}.state", pane_id));
    if fs::write(&state_file, state).is_err() {
        return 1;
    }

    // Read JSON from stdin (hook input)
    if state == "working" {
        let mut input = String::new();
        if std::io::stdin().read_to_string(&mut input).is_ok() && !input.is_empty() {
            if let Some(prompt) = extract_prompt(&input) {
                if !prompt.is_empty() {
                    let title_file = dir.join(format!("{}.title", pane_id));
                    // Only write title if it doesn't exist yet (first prompt = session title)
                    if !title_file.exists() {
                        let _ = fs::write(&title_file, &prompt);
                    }
                    // Always write task (latest prompt)
                    let task_file = dir.join(format!("{}.task", pane_id));
                    let _ = fs::write(&task_file, &prompt);
                }
            }
        }
    }

    0
}

/// Extract the prompt string from hook JSON input.
/// Tries common field names: prompt, message, input.
fn extract_prompt(input: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(input).ok()?;
    let obj = v.as_object()?;

    for key in &["prompt", "message", "input"] {
        if let Some(val) = obj.get(*key) {
            if let Some(s) = val.as_str() {
                let truncated: String = s.chars().take(120).collect();
                return Some(truncated);
            }
        }
    }

    None
}

/// Clean up stale state files for panes that no longer exist in tmux.
pub fn cleanup_stale(active_pane_ids: &[String]) {
    let dir = state_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // State files are like %42.state, %42.title, %42.task, or just %42
        let pane_id = name.split('.').next().unwrap_or(&name);
        if pane_id.starts_with('%') && !active_pane_ids.contains(&pane_id.to_string()) {
            let _ = fs::remove_file(entry.path());
        }
    }
}
