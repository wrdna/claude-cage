use std::time::Instant;

use crate::session::Session;
use crate::tmux;

#[derive(PartialEq)]
pub enum Mode {
    Normal,
    Send,
    Worktree,
    ConfirmKill,
}

pub struct App {
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub mode: Mode,
    pub input: String,
    pub input_cursor: usize,
    pub flash_msg: String,
    pub flash_time: Option<Instant>,
}

impl App {
    pub fn new() -> Self {
        let sessions = tmux::list_claude_sessions();
        Self {
            sessions,
            selected: 0,
            scroll_offset: 0,
            mode: Mode::Normal,
            input: String::new(),
            input_cursor: 0,
            flash_msg: String::new(),
            flash_time: None,
        }
    }

    pub fn refresh(&mut self) {
        self.sessions = tmux::list_claude_sessions();
        if self.selected >= self.sessions.len() && !self.sessions.is_empty() {
            self.selected = self.sessions.len() - 1;
        }
    }

    pub fn next(&mut self) {
        if self.selected < self.sessions.len().saturating_sub(1) {
            self.selected += 1;
        }
    }

    pub fn prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
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
}
