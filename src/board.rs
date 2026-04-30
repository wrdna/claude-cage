use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::task;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryTag {
    Finding,
    Blocker,
    Artifact,
    Recommendation,
    Question,
    Progress,
    Metric,
    Reply,
}

impl EntryTag {
    pub fn label(&self) -> &str {
        match self {
            Self::Finding => "finding",
            Self::Blocker => "blocker",
            Self::Artifact => "artifact",
            Self::Recommendation => "recommendation",
            Self::Question => "question",
            Self::Progress => "progress",
            Self::Metric => "metric",
            Self::Reply => "reply",
        }
    }

    pub fn symbol(&self) -> &str {
        match self {
            Self::Finding => "~",
            Self::Blocker => "!",
            Self::Artifact => "@",
            Self::Recommendation => ">",
            Self::Question => "?",
            Self::Progress => "-",
            Self::Metric => "#",
            Self::Reply => "<",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Finding => Color::Cyan,
            Self::Blocker => Color::Red,
            Self::Artifact => Color::Green,
            Self::Recommendation => Color::Yellow,
            Self::Question => Color::Magenta,
            Self::Progress => Color::Blue,
            Self::Metric => Color::LightCyan,
            Self::Reply => Color::White,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "finding" => Some(Self::Finding),
            "blocker" => Some(Self::Blocker),
            "artifact" => Some(Self::Artifact),
            "recommendation" | "rec" => Some(Self::Recommendation),
            "question" => Some(Self::Question),
            "progress" => Some(Self::Progress),
            "metric" => Some(Self::Metric),
            "reply" => Some(Self::Reply),
            _ => None,
        }
    }

    /// All tags in cycle order for filter toggling.
    pub fn all() -> &'static [EntryTag] {
        &[
            Self::Finding,
            Self::Blocker,
            Self::Artifact,
            Self::Recommendation,
            Self::Question,
            Self::Progress,
            Self::Metric,
            Self::Reply,
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoardEntry {
    pub id: String,
    pub timestamp: u64,
    pub task_id: String,
    pub role: String,
    pub tag: EntryTag,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directed_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub pinned: bool,
}

fn board_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".cache/claude-cage/board.jsonl")
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn gen_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let ts = now_unix();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    // Mix timestamp + counter + pid for uniqueness across processes
    let pid = std::process::id();
    format!("b-{}-{:04x}{:04x}", ts, pid & 0xFFFF, seq & 0xFFFF)
}

/// Load all board entries from JSONL file.
pub fn load_entries() -> Vec<BoardEntry> {
    let path = board_path();
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);
    reader
        .lines()
        .filter_map(|line| {
            line.ok()
                .and_then(|l| serde_json::from_str::<BoardEntry>(&l).ok())
        })
        .collect()
}

/// Append a single entry to the board (concurrent-safe for small writes).
pub fn append_entry(entry: &BoardEntry) {
    let path = board_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        if let Ok(json) = serde_json::to_string(entry) {
            let _ = writeln!(file, "{}", json);
        }
    }
}

/// Rewrite the entire board (used for pin/unpin which modifies existing entries).
fn save_entries(entries: &[BoardEntry]) {
    let path = board_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("jsonl.tmp");
    if let Ok(mut file) = fs::File::create(&tmp) {
        for entry in entries {
            if let Ok(json) = serde_json::to_string(entry) {
                let _ = writeln!(file, "{}", json);
            }
        }
    }
    if fs::rename(&tmp, &path).is_err() {
        // fallback
        if let Ok(mut file) = fs::File::create(&path) {
            for entry in entries {
                if let Ok(json) = serde_json::to_string(entry) {
                    let _ = writeln!(file, "{}", json);
                }
            }
        }
    }
}

/// Toggle pin status on an entry.
pub fn toggle_pin(entry_id: &str) -> bool {
    let mut entries = load_entries();
    if let Some(e) = entries.iter_mut().find(|e| e.id == entry_id) {
        e.pinned = !e.pinned;
        save_entries(&entries);
        true
    } else {
        false
    }
}

/// Filter entries by tag and/or source task.
pub fn filter_entries<'a>(
    entries: &'a [BoardEntry],
    tag: Option<&EntryTag>,
    from_task: Option<&str>,
) -> Vec<&'a BoardEntry> {
    entries
        .iter()
        .filter(|e| {
            tag.map_or(true, |t| &e.tag == t) && from_task.map_or(true, |id| e.task_id == id)
        })
        .collect()
}

/// Get entries sorted for display: pinned first, then reverse chronological.
pub fn display_order(entries: &[BoardEntry]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..entries.len()).collect();
    indices.sort_by(|&a, &b| {
        let ea = &entries[a];
        let eb = &entries[b];
        // Pinned first, then newest first
        eb.pinned
            .cmp(&ea.pinned)
            .then(eb.timestamp.cmp(&ea.timestamp))
    });
    indices
}

/// Format a unix timestamp as relative time (e.g. "2m ago", "1h ago").
pub fn relative_time(ts: u64) -> String {
    let now = now_unix();
    let diff = now.saturating_sub(ts);
    if diff < 60 {
        format!("{}s", diff)
    } else if diff < 3600 {
        format!("{}m", diff / 60)
    } else if diff < 86400 {
        format!("{}h", diff / 3600)
    } else {
        format!("{}d", diff / 86400)
    }
}

/// Look up task name by ID from the task tree.
pub fn task_name_for(task_id: &str) -> Option<String> {
    let tasks = task::load_tasks();
    task::find_task(&tasks, task_id).map(|t| t.name.clone())
}

// ─── CLI subcommand handler ──────────────────────────────────────────────

/// Handle `claude-cage board <subcommand> [args...]`
pub fn handle_board_cmd(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("Usage: claude-cage board <post|read|pin|reply|clear|list|watch>");
        return 1;
    }

    match args[0].as_str() {
        "post" => cmd_post(&args[1..]),
        "read" => cmd_read(&args[1..]),
        "pin" => cmd_pin(&args[1..]),
        "reply" => cmd_reply(&args[1..]),
        "clear" => cmd_clear(),
        "list" => cmd_list(&args[1..]),
        "watch" => cmd_watch(&args[1..]),
        other => {
            eprintln!("Unknown board subcommand: {}", other);
            1
        }
    }
}

/// claude-cage board post <task-id> <text> --tag <tag> [--to <task-id>] [--role <role>]
fn cmd_post(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!(
            "Usage: claude-cage board post <task-id> <text> --tag <tag> [--to <task-id>] [--role <role>]"
        );
        return 1;
    }
    let task_id = &args[0];

    // Collect text (everything that isn't a flag)
    let mut text_parts = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i].starts_with("--") {
            i += 2; // skip flag + value
        } else {
            text_parts.push(args[i].as_str());
            i += 1;
        }
    }
    let text = text_parts.join(" ");
    if text.is_empty() {
        eprintln!("No text provided");
        return 1;
    }

    let tag_str = parse_flag(args, "--tag").unwrap_or_else(|| "finding".to_string());
    let tag = match EntryTag::from_str(&tag_str) {
        Some(t) => t,
        None => {
            eprintln!(
                "Invalid tag: {}. Use: finding, blocker, artifact, recommendation, question, progress, metric",
                tag_str
            );
            return 1;
        }
    };
    let directed_to = parse_flag(args, "--to");
    let role = parse_flag(args, "--role").unwrap_or_default();

    let entry = BoardEntry {
        id: gen_id(),
        timestamp: now_unix(),
        task_id: task_id.clone(),
        role,
        tag,
        content: text,
        directed_to,
        reply_to: None,
        pinned: false,
    };

    append_entry(&entry);
    println!("{}", entry.id);
    0
}

/// claude-cage board read [--tag <tag>] [--from <task-id>] [--last <n>]
fn cmd_read(args: &[String]) -> i32 {
    let entries = load_entries();

    let tag_filter = parse_flag(args, "--tag").and_then(|s| EntryTag::from_str(&s));
    let from_filter = parse_flag(args, "--from");
    let last_n: usize = parse_flag(args, "--last")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);

    let filtered = filter_entries(
        &entries,
        tag_filter.as_ref(),
        from_filter.as_deref(),
    );

    let start = filtered.len().saturating_sub(last_n);
    for entry in &filtered[start..] {
        println!(
            "[{}] {} [{}] {}: {}{}",
            relative_time(entry.timestamp),
            entry.tag.symbol(),
            entry.tag.label(),
            if entry.task_id.is_empty() {
                "user"
            } else {
                &entry.task_id
            },
            entry.content,
            if entry.pinned { " (pinned)" } else { "" },
        );
    }
    0
}

/// claude-cage board pin <entry-id>
fn cmd_pin(args: &[String]) -> i32 {
    if args.is_empty() {
        eprintln!("Usage: claude-cage board pin <entry-id>");
        return 1;
    }
    if toggle_pin(&args[0]) {
        0
    } else {
        eprintln!("Entry '{}' not found", args[0]);
        1
    }
}

/// claude-cage board reply <entry-id> <text>
fn cmd_reply(args: &[String]) -> i32 {
    if args.len() < 2 {
        eprintln!("Usage: claude-cage board reply <entry-id> <text>");
        return 1;
    }
    let reply_to_id = &args[0];
    let text = args[1..].join(" ");

    // Find the original entry to get task context
    let entries = load_entries();
    let original = entries.iter().find(|e| e.id == *reply_to_id);
    let task_id = original.map(|e| e.task_id.clone()).unwrap_or_default();

    let entry = BoardEntry {
        id: gen_id(),
        timestamp: now_unix(),
        task_id: String::new(), // replies from user have no task
        role: "user".to_string(),
        tag: EntryTag::Reply,
        content: text,
        directed_to: if task_id.is_empty() {
            None
        } else {
            Some(task_id)
        },
        reply_to: Some(reply_to_id.clone()),
        pinned: false,
    };

    append_entry(&entry);
    println!("{}", entry.id);
    0
}

/// claude-cage board clear
fn cmd_clear() -> i32 {
    let path = board_path();
    let _ = fs::remove_file(&path);
    println!("Board cleared");
    0
}

/// claude-cage board list [--json]
fn cmd_list(args: &[String]) -> i32 {
    let entries = load_entries();
    if args.first().map(|s| s.as_str()) == Some("--json") {
        if let Ok(json) = serde_json::to_string_pretty(&entries) {
            println!("{}", json);
        }
    } else {
        for entry in &entries {
            println!(
                "{} [{}] {} ({}) — {}{}",
                entry.id,
                entry.tag.label(),
                entry.content,
                if entry.task_id.is_empty() {
                    "user".to_string()
                } else {
                    entry.task_id.clone()
                },
                relative_time(entry.timestamp),
                if entry.pinned { " *pinned*" } else { "" },
            );
        }
    }
    0
}

/// claude-cage board watch [--tag <tag>] [--from <task-id>] [--to <task-id>]
///                          [--since-id <entry-id>] [--last <n>] [--poll-ms <ms>] [--json]
/// Tail-follow new entries appended to the board. Starts at "now" by default;
/// --last N backfills the last N entries first; --since-id resumes after a
/// specific entry. Filters: --tag, --from (poster), --to (directed-to).
/// Output: one line per matching entry (same shape as `read`), flushed.
/// Use --json for one JSON object per line (machine-readable).
fn cmd_watch(args: &[String]) -> i32 {
    let tag_filter = parse_flag(args, "--tag").and_then(|s| EntryTag::from_str(&s));
    let from_filter = parse_flag(args, "--from");
    let to_filter = parse_flag(args, "--to");
    let since_id = parse_flag(args, "--since-id");
    let last_n: Option<usize> = parse_flag(args, "--last").and_then(|s| s.parse().ok());
    let poll_ms: u64 = parse_flag(args, "--poll-ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);
    let json_mode = args.iter().any(|a| a == "--json");

    let matches = |e: &BoardEntry| -> bool {
        if let Some(t) = &tag_filter {
            if &e.tag != t {
                return false;
            }
        }
        if let Some(f) = &from_filter {
            if &e.task_id != f {
                return false;
            }
        }
        if let Some(to) = &to_filter {
            if e.directed_to.as_deref() != Some(to.as_str()) {
                return false;
            }
        }
        true
    };

    let emit = |entry: &BoardEntry| {
        let stdout = std::io::stdout();
        let mut h = stdout.lock();
        if json_mode {
            if let Ok(json) = serde_json::to_string(entry) {
                let _ = writeln!(h, "{}", json);
            }
        } else {
            let _ = writeln!(
                h,
                "[{}] {} [{}] {}: {}{}",
                relative_time(entry.timestamp),
                entry.tag.symbol(),
                entry.tag.label(),
                if entry.task_id.is_empty() {
                    "user"
                } else {
                    &entry.task_id
                },
                entry.content,
                if entry.pinned { " (pinned)" } else { "" },
            );
        }
        let _ = h.flush();
    };

    // Establish starting point. Priority: --since-id > --last > "now."
    let entries = load_entries();
    let mut last_count = if let Some(sid) = &since_id {
        match entries.iter().position(|e| &e.id == sid) {
            Some(i) => i + 1, // start AFTER since-id
            None => entries.len(), // not found → start from now
        }
    } else if let Some(n) = last_n {
        let start = entries.len().saturating_sub(n);
        // Backfill: emit the last N matching entries before entering loop.
        for entry in &entries[start..] {
            if matches(entry) {
                emit(entry);
            }
        }
        entries.len()
    } else {
        entries.len()
    };

    loop {
        let entries = load_entries();
        if entries.len() > last_count {
            for entry in &entries[last_count..] {
                if matches(entry) {
                    emit(entry);
                }
            }
            last_count = entries.len();
        } else if entries.len() < last_count {
            // Board was cleared / truncated. Reset to current end so we don't
            // re-emit forever; flag this on stderr for the watcher to see.
            eprintln!("[watch] board shrank ({} → {}); resyncing", last_count, entries.len());
            last_count = entries.len();
        }
        std::thread::sleep(Duration::from_millis(poll_ms));
    }
}

/// Parse a --flag value from args.
fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
