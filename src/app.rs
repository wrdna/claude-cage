use std::collections::HashSet;
use std::time::Instant;

use crate::board::{self, BoardEntry, EntryTag};
use crate::session::Session;
use crate::skills::{self, Skill};
use crate::state;
use crate::task::{self, Task};
use crate::tmux;

#[derive(PartialEq)]
pub enum Mode {
    Normal,
    Chat,
    Worktree,
    ConfirmKill,
    Skill,
    AddSkillName,
    AddSkillCommand,
    Nudge,      // composing a nudge message for a task
    TaskChat,   // forwarding keystrokes to a task's linked pane
    BoardReply, // composing a reply to a board entry
}

#[derive(PartialEq)]
pub enum ViewMode {
    Sessions,
    Tasks,
    Board,
}

pub struct App {
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub preview_scroll: usize,
    pub mode: Mode,
    pub input: String,
    pub input_cursor: usize,
    pub flash_msg: String,
    pub flash_time: Option<Instant>,
    pub skills: Vec<Skill>,
    pub skill_selected: usize,
    pub skill_name_buf: String, // temp buffer for add-skill flow
    pub view_mode: ViewMode,
    pub tasks: Vec<Task>,
    pub task_selected: usize,
    pub task_expanded: HashSet<String>,
    pub nudge_target_id: String,
    pub board_entries: Vec<BoardEntry>,
    pub board_selected: usize,
    pub board_filter: Option<EntryTag>,
    pub board_reply_target: String, // entry id being replied to
}

impl App {
    pub fn new() -> Self {
        let sessions = tmux::list_claude_sessions();
        let skill_list = skills::load_skills();
        let tasks = task::load_tasks();
        let view_mode = load_view_mode();

        // Auto-expand root tasks so the tree isn't all collapsed
        let mut task_expanded = HashSet::new();
        for t in &tasks {
            if !t.subtasks.is_empty() {
                task_expanded.insert(t.id.clone());
            }
        }

        Self {
            sessions,
            selected: 0,
            scroll_offset: 0,
            preview_scroll: 0,
            mode: Mode::Normal,
            input: String::new(),
            input_cursor: 0,
            flash_msg: String::new(),
            flash_time: None,
            skills: skill_list,
            skill_selected: 0,
            skill_name_buf: String::new(),
            view_mode,
            tasks,
            task_selected: 0,
            task_expanded,
            nudge_target_id: String::new(),
            board_entries: board::load_entries(),
            board_selected: 0,
            board_filter: None,
            board_reply_target: String::new(),
        }
    }

    pub fn refresh(&mut self) {
        self.sessions = tmux::list_claude_sessions();
        if self.selected >= self.sessions.len() && !self.sessions.is_empty() {
            self.selected = self.sessions.len() - 1;
        }
        // Clean up state files for panes that no longer exist
        let active_ids: Vec<String> = self.sessions.iter().map(|s| s.pane_id.clone()).collect();
        state::cleanup_stale(&active_ids);

        // Always reload tasks so the badge/count stays current
        self.tasks = task::load_tasks();
        if self.view_mode == ViewMode::Tasks {
            let flat_len = task::flatten_tree(&self.tasks, &self.task_expanded, 0).len();
            if self.task_selected >= flat_len && flat_len > 0 {
                self.task_selected = flat_len - 1;
            }
        }

        // Always reload board
        self.board_entries = board::load_entries();
        if self.view_mode == ViewMode::Board {
            let visible = self.board_visible_count();
            if self.board_selected >= visible && visible > 0 {
                self.board_selected = visible - 1;
            }
        }
    }

    pub fn next(&mut self) {
        if self.selected < self.sessions.len().saturating_sub(1) {
            self.selected += 1;
            self.preview_scroll = 0;
        }
    }

    pub fn prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.preview_scroll = 0;
        }
    }

    pub fn preview_up(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_add(1);
    }

    pub fn preview_down(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_sub(1);
    }

    pub fn preview_scroll_by(&mut self, delta: isize) {
        if delta > 0 {
            self.preview_scroll = self.preview_scroll.saturating_add(delta as usize);
        } else {
            self.preview_scroll = self.preview_scroll.saturating_sub((-delta) as usize);
        }
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.sessions.get(self.selected)
    }

    pub fn flash(&mut self, msg: &str) {
        self.flash_msg = msg.to_string();
        self.flash_time = Some(Instant::now());
    }

    pub fn flash_active(&self) -> bool {
        self.flash_time
            .map(|t| t.elapsed().as_secs_f32() < 2.5)
            .unwrap_or(false)
    }

    pub fn toggle_view(&mut self) {
        match self.view_mode {
            ViewMode::Sessions => {
                self.tasks = task::load_tasks();
                // Auto-expand root tasks on switch
                for t in &self.tasks {
                    if !t.subtasks.is_empty() {
                        self.task_expanded.insert(t.id.clone());
                    }
                }
                self.view_mode = ViewMode::Tasks;
                self.preview_scroll = 0;
            }
            ViewMode::Tasks => {
                self.board_entries = board::load_entries();
                self.view_mode = ViewMode::Board;
                self.preview_scroll = 0;
            }
            ViewMode::Board => {
                self.view_mode = ViewMode::Sessions;
                self.preview_scroll = 0;
            }
        }
        save_view_mode(&self.view_mode);
    }

    pub fn task_next(&mut self) {
        let flat = task::flatten_tree(&self.tasks, &self.task_expanded, 0);
        if self.task_selected < flat.len().saturating_sub(1) {
            self.task_selected += 1;
            self.preview_scroll = 0;
        }
    }

    pub fn task_prev(&mut self) {
        if self.task_selected > 0 {
            self.task_selected -= 1;
            self.preview_scroll = 0;
        }
    }

    pub fn toggle_task_expand(&mut self) {
        let flat = task::flatten_tree(&self.tasks, &self.task_expanded, 0);
        if let Some(ft) = flat.get(self.task_selected) {
            if ft.has_children {
                let id = ft.task.id.clone();
                if self.task_expanded.contains(&id) {
                    self.task_expanded.remove(&id);
                } else {
                    self.task_expanded.insert(id);
                }
            }
        }
    }

    pub fn selected_task(&self) -> Option<&Task> {
        let flat = task::flatten_tree(&self.tasks, &self.task_expanded, 0);
        flat.get(self.task_selected).map(|ft| ft.task)
    }

    /// Get the pane_id of the selected task's linked session.
    pub fn selected_task_pane(&self) -> Option<String> {
        self.selected_task().and_then(|t| t.pane_id.clone())
    }

    /// Find a session by pane_id.
    pub fn session_by_pane(&self, pane_id: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.pane_id == pane_id)
    }

    /// Start nudge mode: compose a message for the selected task.
    pub fn start_nudge(&mut self) {
        if let Some(t) = self.selected_task() {
            self.nudge_target_id = t.id.clone();
            self.mode = Mode::Nudge;
            self.input.clear();
            self.input_cursor = 0;
        }
    }

    /// Start task chat: forward keystrokes to the selected task's linked pane.
    pub fn start_task_chat(&mut self) {
        if let Some(pane_id) = self.selected_task_pane() {
            if self.session_by_pane(&pane_id).is_some() {
                self.mode = Mode::TaskChat;
                self.preview_scroll = 0;
            }
        }
    }

    // ─── Board ───

    /// Get display-ordered indices of visible board entries (filtered + sorted).
    pub fn board_visible_indices(&self) -> Vec<usize> {
        let order = board::display_order(&self.board_entries);
        if let Some(ref tag) = self.board_filter {
            order
                .into_iter()
                .filter(|&i| self.board_entries[i].tag == *tag)
                .collect()
        } else {
            order
        }
    }

    pub fn board_visible_count(&self) -> usize {
        self.board_visible_indices().len()
    }

    pub fn board_next(&mut self) {
        let count = self.board_visible_count();
        if self.board_selected < count.saturating_sub(1) {
            self.board_selected += 1;
            self.preview_scroll = 0;
        }
    }

    pub fn board_prev(&mut self) {
        if self.board_selected > 0 {
            self.board_selected -= 1;
            self.preview_scroll = 0;
        }
    }

    pub fn board_cycle_filter(&mut self) {
        let tags = EntryTag::all();
        self.board_filter = match &self.board_filter {
            None => Some(tags[0].clone()),
            Some(current) => {
                let pos = tags.iter().position(|t| t == current).unwrap_or(0);
                if pos + 1 >= tags.len() {
                    None // wrap around to "all"
                } else {
                    Some(tags[pos + 1].clone())
                }
            }
        };
        self.board_selected = 0;
        self.preview_scroll = 0;
    }

    pub fn selected_board_entry(&self) -> Option<&BoardEntry> {
        let indices = self.board_visible_indices();
        indices
            .get(self.board_selected)
            .and_then(|&i| self.board_entries.get(i))
    }

    pub fn board_toggle_pin(&mut self) {
        if let Some(entry) = self.selected_board_entry() {
            let id = entry.id.clone();
            board::toggle_pin(&id);
            self.board_entries = board::load_entries();
        }
    }

    pub fn start_board_reply(&mut self) {
        if let Some(entry) = self.selected_board_entry() {
            self.board_reply_target = entry.id.clone();
            self.mode = Mode::BoardReply;
            self.input.clear();
            self.input_cursor = 0;
        }
    }
}

fn view_mode_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(home).join(".cache/claude-cage/view_mode")
}

fn save_view_mode(mode: &ViewMode) {
    let path = view_mode_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, match mode {
        ViewMode::Sessions => "sessions",
        ViewMode::Tasks => "tasks",
        ViewMode::Board => "board",
    });
}

fn load_view_mode() -> ViewMode {
    let path = view_mode_path();
    match std::fs::read_to_string(&path) {
        Ok(s) if s.trim() == "tasks" => ViewMode::Tasks,
        Ok(s) if s.trim() == "board" => ViewMode::Board,
        _ => ViewMode::Sessions,
    }
}
