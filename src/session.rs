use std::fs;
use std::path::PathBuf;
use std::process::Command;

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

/// Get the git branch for a directory
fn git_branch(dir: &str) -> String {
    Command::new("git")
        .args(["-C", dir, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Get the top-level git repo root for a directory
fn git_toplevel(dir: &str) -> String {
    Command::new("git")
        .args(["-C", dir, "rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Determine project name and worktree name from the path
fn parse_project_worktree(path: &str, home: &str) -> (String, String) {
    let toplevel = git_toplevel(path);

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
        let branch = git_branch(&path);
        let (project, worktree) = parse_project_worktree(&path, &home);

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
