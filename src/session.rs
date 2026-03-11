use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct ContextUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read: u64,
    pub cache_create: u64,
    pub total_context: u64, // approximate current context window usage
}

#[derive(Clone, Debug)]
pub struct Session {
    pub addr: String,
    pub pane_id: String,
    pub path: String,
    pub short_path: String,
    pub is_active: bool,
    pub state: SessionState,
    pub title: String,
    pub task: String,
    pub branch: String,
    pub project: String,    // root repo name
    pub worktree: String,   // worktree name (empty if main)
    pub context: Option<ContextUsage>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SessionState {
    Working,
    Idle,
    Waiting,
    Unknown,
}

impl SessionState {
    pub fn label(&self) -> &str {
        match self {
            Self::Working => "working",
            Self::Idle => "idle",
            Self::Waiting => "waiting",
            Self::Unknown => "unknown",
        }
    }

    pub fn symbol(&self) -> &str {
        match self {
            Self::Working => "●",
            Self::Idle => "○",
            Self::Waiting => "◐",
            Self::Unknown => "?",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        match self {
            Self::Working => ratatui::style::Color::Yellow,
            Self::Idle => ratatui::style::Color::Green,
            Self::Waiting => ratatui::style::Color::Magenta,
            Self::Unknown => ratatui::style::Color::DarkGray,
        }
    }
}

fn state_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cache/claude-pane-states")
}

fn read_file(path: &PathBuf) -> String {
    fs::read_to_string(path).unwrap_or_default().trim().to_string()
}

/// Get git branch and toplevel in a single call to reduce subprocess overhead.
fn git_info(dir: &str) -> (String, String) {
    // Single git call: outputs branch on line 1, toplevel on line 2
    let output = Command::new("git")
        .args(["-C", dir, "rev-parse", "--abbrev-ref", "HEAD", "--show-toplevel"])
        .output()
        .ok();

    match output {
        Some(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            let mut lines = text.lines();
            let branch = lines.next().unwrap_or("").trim().to_string();
            let toplevel = lines.next().unwrap_or("").trim().to_string();
            (branch, toplevel)
        }
        _ => (String::new(), String::new()),
    }
}

/// Determine project name and worktree name from the path.
/// Takes pre-fetched toplevel to avoid redundant git calls.
fn parse_project_worktree(path: &str, toplevel: &str, _home: &str) -> (String, String) {
    // Check if this is inside a .claude/worktrees/ path
    if path.contains("/.claude/worktrees/") {
        // Path like /home/user/dev/myproject/.claude/worktrees/feature-auth
        // Project is "myproject", worktree is "feature-auth"
        if let Some(idx) = path.find("/.claude/worktrees/") {
            let project_path = &path[..idx];
            let project = project_path
                .rsplit('/')
                .next()
                .unwrap_or(project_path)
                .to_string();
            let worktree = path[idx + "/.claude/worktrees/".len()..]
                .split('/')
                .next()
                .unwrap_or("")
                .to_string();
            return (project, worktree);
        }
    }

    // Regular repo — project name from the toplevel or path
    let base = if !toplevel.is_empty() {
        &toplevel
    } else {
        path
    };
    let project = base
        .rsplit('/')
        .next()
        .unwrap_or(base)
        .to_string();

    (project, String::new())
}

/// Read context/token usage from the most recent Claude session JSONL for this path.
fn read_context_usage(path: &str, home: &str) -> Option<ContextUsage> {
    // Convert path to Claude project directory name: /home/user/dev/foo → -home-user-dev-foo
    let project_dir_name = path.replace('/', "-");
    let claude_dir = PathBuf::from(home).join(".claude/projects").join(&project_dir_name);

    if !claude_dir.is_dir() {
        return None;
    }

    // Find the most recently modified .jsonl file
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(entries) = fs::read_dir(&claude_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                if let Ok(meta) = p.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if newest.as_ref().map_or(true, |(t, _)| modified > *t) {
                            newest = Some((modified, p));
                        }
                    }
                }
            }
        }
    }

    let jsonl_path = newest?.1;

    // Read the file from the end to find the last assistant message with usage.
    // For efficiency, read only the last 64KB.
    let file = fs::File::open(&jsonl_path).ok()?;
    let file_len = file.metadata().ok()?.len();
    let read_start = if file_len > 65536 { file_len - 65536 } else { 0 };

    use std::io::{Read, Seek, SeekFrom};
    let mut file = file;
    file.seek(SeekFrom::Start(read_start)).ok()?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;

    // Parse lines from end, find last assistant message with usage
    for line in buf.lines().rev() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if v.get("type").and_then(|t| t.as_str()) == Some("assistant") {
                if let Some(msg) = v.get("message") {
                    if let Some(usage) = msg.get("usage") {
                        let input = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        let output = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        let cache_read = usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        let cache_create = usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        let total_context = input + cache_read + cache_create;
                        return Some(ContextUsage {
                            input_tokens: input,
                            output_tokens: output,
                            cache_read,
                            cache_create,
                            total_context,
                        });
                    }
                }
            }
        }
    }

    None
}

impl Session {
    pub fn from_tmux_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 7 || parts[2] != "claude" {
            return None;
        }

        let home = std::env::var("HOME").unwrap_or_default();
        let path = parts[3].to_string();
        let short_path = path.replace(&home, "~");
        let is_active = parts[4] == "1" && parts[5] == "1" && parts[6] == "1";

        let pane_id = parts[1].to_string();
        let dir = state_dir();

        // Read state
        let state_str = read_file(&dir.join(format!("{}.state", pane_id)));
        let state_str = if state_str.is_empty() {
            read_file(&dir.join(&pane_id))
        } else {
            state_str
        };
        let state = match state_str.as_str() {
            "working" => SessionState::Working,
            "idle" => SessionState::Idle,
            "waiting" => SessionState::Waiting,
            _ => SessionState::Unknown,
        };

        let title = read_file(&dir.join(format!("{}.title", pane_id)));
        let task = read_file(&dir.join(format!("{}.task", pane_id)));
        let (branch, toplevel) = git_info(&path);
        let (project, worktree) = parse_project_worktree(&path, &toplevel, &home);
        let context = read_context_usage(&path, &home);

        Some(Session {
            addr: parts[0].to_string(),
            pane_id,
            path,
            short_path,
            is_active,
            state,
            title,
            task,
            branch,
            project,
            worktree,
            context,
        })
    }

    /// Display label: shows worktree name or "main"
    pub fn worktree_label(&self) -> &str {
        if self.worktree.is_empty() {
            if self.branch.is_empty() {
                ""
            } else {
                &self.branch
            }
        } else {
            &self.worktree
        }
    }
}

pub fn cleanup_state(pane_id: &str) {
    let dir = state_dir();
    for ext in &["", ".state", ".title", ".task"] {
        let path = dir.join(format!("{}{}", pane_id, ext));
        let _ = fs::remove_file(path);
    }
}
