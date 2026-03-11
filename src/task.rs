use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl TaskStatus {
    pub fn symbol(&self) -> &str {
        match self {
            Self::Pending => "☐",
            Self::InProgress => "◑",
            Self::Completed => "☑",
            Self::Failed => "☒",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Pending => Color::DarkGray,
            Self::InProgress => Color::Yellow,
            Self::Completed => Color::Green,
            Self::Failed => Color::Red,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub status: TaskStatus,
    pub role: String,
    pub pane_id: Option<String>,
    pub subtasks: Vec<Task>,
    pub output: String,
    #[serde(default)]
    pub created_at: u64,
}

/// A flattened view of a task tree node for rendering.
pub struct FlatTask<'a> {
    pub depth: usize,
    pub task: &'a Task,
    pub has_children: bool,
    pub is_expanded: bool,
    pub is_last: bool,
}

fn tasks_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cache/claude-cage/tasks.json")
}

fn nudges_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cache/claude-cage/nudges")
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn load_tasks() -> Vec<Task> {
    let path = tasks_path();
    if let Ok(data) = fs::read_to_string(&path) {
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        Vec::new()
    }
}

pub fn save_tasks(tasks: &[Task]) {
    let path = tasks_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string_pretty(tasks) {
        // Atomic write: tmp then rename
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, &data).is_ok() {
            if fs::rename(&tmp, &path).is_err() {
                // Fallback: direct write
                let _ = fs::write(&path, &data);
            }
        }
    }
}

/// Recursively find a task by ID (immutable).
pub fn find_task<'a>(tasks: &'a [Task], id: &str) -> Option<&'a Task> {
    for t in tasks {
        if t.id == id {
            return Some(t);
        }
        if let Some(found) = find_task(&t.subtasks, id) {
            return Some(found);
        }
    }
    None
}

/// Recursively find a task by ID (mutable).
pub fn find_task_mut<'a>(tasks: &'a mut Vec<Task>, id: &str) -> Option<&'a mut Task> {
    for t in tasks.iter_mut() {
        if t.id == id {
            return Some(t);
        }
        if let Some(found) = find_task_mut(&mut t.subtasks, id) {
            return Some(found);
        }
    }
    None
}

/// Add a subtask under a parent. Returns true if parent was found.
pub fn add_subtask(tasks: &mut Vec<Task>, parent_id: &str, subtask: Task) -> bool {
    if let Some(parent) = find_task_mut(tasks, parent_id) {
        parent.subtasks.push(subtask);
        true
    } else {
        false
    }
}

/// Update a task's status. Returns true if found.
pub fn update_status(tasks: &mut Vec<Task>, id: &str, status: TaskStatus) -> bool {
    if let Some(t) = find_task_mut(tasks, id) {
        t.status = status;
        true
    } else {
        false
    }
}

/// Set a task's output text (replaces). Returns true if found.
pub fn set_output(tasks: &mut Vec<Task>, id: &str, output: String) -> bool {
    if let Some(t) = find_task_mut(tasks, id) {
        t.output = output;
        true
    } else {
        false
    }
}

/// Append a line to a task's output. Returns true if found.
pub fn append_output(tasks: &mut Vec<Task>, id: &str, line: &str) -> bool {
    if let Some(t) = find_task_mut(tasks, id) {
        if !t.output.is_empty() {
            t.output.push('\n');
        }
        t.output.push_str(line);
        true
    } else {
        false
    }
}

/// Write a nudge message for a task.
pub fn write_nudge(task_id: &str, message: &str) {
    let dir = nudges_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join(format!("{}.txt", task_id));
    let _ = fs::write(&path, message);
}

/// Read and consume a nudge message for a task. Returns None if no nudge pending.
pub fn read_nudge(task_id: &str) -> Option<String> {
    let dir = nudges_dir();
    let path = dir.join(format!("{}.txt", task_id));
    let msg = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    let trimmed = msg.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

/// Check if a nudge exists without consuming it.
pub fn has_nudge(task_id: &str) -> bool {
    let dir = nudges_dir();
    dir.join(format!("{}.txt", task_id)).exists()
}

/// Flatten the task tree into a list for rendering, respecting expanded state.
pub fn flatten_tree<'a>(
    tasks: &'a [Task],
    expanded: &HashSet<String>,
    depth: usize,
) -> Vec<FlatTask<'a>> {
    let mut result = Vec::new();
    let len = tasks.len();
    for (i, task) in tasks.iter().enumerate() {
        let has_children = !task.subtasks.is_empty();
        let is_expanded = expanded.contains(&task.id);
        let is_last = i == len - 1;
        result.push(FlatTask {
            depth,
            task,
            has_children,
            is_expanded,
            is_last,
        });
        if has_children && is_expanded {
            let children = flatten_tree(&task.subtasks, expanded, depth + 1);
            result.extend(children);
        }
    }
    result
}

pub fn role_color(role: &str) -> Color {
    match role {
        "architect" => Color::Cyan,
        "implement" => Color::Green,
        "review" => Color::Blue,
        "security" => Color::Red,
        "test-gen" => Color::Magenta,
        "research" => Color::Yellow,
        "orchestrate" => Color::LightCyan,
        _ => Color::White,
    }
}

// ─── CLI subcommand handler ──────────────────────────────────────────────

/// Handle `claude-cage task <subcommand> [args...]`
/// Returns exit code.
pub fn handle_task_cmd(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("Usage: claude-cage task <init|add|status|output|append|nudge|clear|list>");
        return 1;
    }

    match args[0].as_str() {
        "init" => cmd_init(&args[1..]),
        "add" => cmd_add(&args[1..]),
        "status" => cmd_status(&args[1..]),
        "output" => cmd_output(&args[1..]),
        "append" => cmd_append(&args[1..]),
        "nudge" => cmd_nudge(&args[1..]),
        "clear" => cmd_clear(),
        "list" => cmd_list(),
        other => {
            eprintln!("Unknown task subcommand: {}", other);
            1
        }
    }
}

/// claude-cage task init <id> <name> [--role <role>] [--pane-id <pane>]
fn cmd_init(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("Usage: claude-cage task init <id> <name> [--role <role>] [--pane-id <pane>]");
        return 1;
    }
    let id = &args[0];
    let name = &args[1];
    let role = parse_flag(args, "--role").unwrap_or_else(|| "orchestrate".to_string());
    let pane_id = parse_flag(args, "--pane-id");

    let mut tasks = load_tasks();
    tasks.push(Task {
        id: id.clone(),
        name: name.clone(),
        status: TaskStatus::InProgress,
        role,
        pane_id,
        subtasks: Vec::new(),
        output: String::new(),
        created_at: now_unix(),
    });
    save_tasks(&tasks);
    println!("{}", id);
    0
}

/// claude-cage task add <parent-id> <id> <name> [--role <role>] [--pane-id <pane>]
fn cmd_add(args: &[String]) -> i32 {
    if args.len() < 3 {
        eprintln!("Usage: claude-cage task add <parent-id> <id> <name> [--role <role>] [--pane-id <pane>]");
        return 1;
    }
    let parent_id = &args[0];
    let id = &args[1];
    let name = &args[2];
    let role = parse_flag(args, "--role").unwrap_or_default();
    let pane_id = parse_flag(args, "--pane-id");

    let mut tasks = load_tasks();
    let subtask = Task {
        id: id.clone(),
        name: name.clone(),
        status: TaskStatus::Pending,
        role,
        pane_id,
        subtasks: Vec::new(),
        output: String::new(),
        created_at: now_unix(),
    };

    if add_subtask(&mut tasks, parent_id, subtask) {
        save_tasks(&tasks);
        0
    } else {
        eprintln!("Parent task '{}' not found", parent_id);
        1
    }
}

/// claude-cage task status <id> <pending|in_progress|completed|failed>
fn cmd_status(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("Usage: claude-cage task status <id> <pending|in_progress|completed|failed>");
        return 1;
    }
    let id = &args[0];
    let status = match TaskStatus::from_str(&args[1]) {
        Some(s) => s,
        None => {
            eprintln!("Invalid status: {}. Use: pending, in_progress, completed, failed", args[1]);
            return 1;
        }
    };

    let mut tasks = load_tasks();
    if update_status(&mut tasks, id, status) {
        save_tasks(&tasks);
        0
    } else {
        eprintln!("Task '{}' not found", id);
        1
    }
}

/// claude-cage task output <id> [text]
/// If text is omitted, reads from stdin.
fn cmd_output(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("Usage: claude-cage task output <id> [text...]");
        return 1;
    }
    let id = &args[0];
    let text = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        let mut buf = String::new();
        let _ = std::io::stdin().read_to_string(&mut buf);
        buf.trim().to_string()
    };

    let mut tasks = load_tasks();
    if set_output(&mut tasks, id, text) {
        save_tasks(&tasks);
        0
    } else {
        eprintln!("Task '{}' not found", id);
        1
    }
}

/// claude-cage task append <id> <text...>
/// Appends a line to the task's output (doesn't replace).
fn cmd_append(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("Usage: claude-cage task append <id> <text...>");
        return 1;
    }
    let id = &args[0];
    let text = args[1..].join(" ");

    let mut tasks = load_tasks();
    if append_output(&mut tasks, id, &text) {
        save_tasks(&tasks);
        0
    } else {
        eprintln!("Task '{}' not found", id);
        1
    }
}

/// claude-cage task nudge <id>
/// Reads and consumes a pending nudge. Prints to stdout if found, exits 1 if none.
fn cmd_nudge(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("Usage: claude-cage task nudge <id>");
        return 1;
    }
    match read_nudge(&args[0]) {
        Some(msg) => {
            println!("{}", msg);
            0
        }
        None => 1,
    }
}

/// claude-cage task clear
fn cmd_clear() -> i32 {
    save_tasks(&[]);
    println!("Tasks cleared");
    0
}

/// claude-cage task list — dump tasks as JSON to stdout
fn cmd_list() -> i32 {
    let tasks = load_tasks();
    if let Ok(json) = serde_json::to_string_pretty(&tasks) {
        println!("{}", json);
    }
    0
}

/// Parse a --flag value from args.
fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
