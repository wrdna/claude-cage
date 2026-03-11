use std::time::Instant;

use crate::session::Session;
use crate::skills::{self, Skill};
use crate::state;
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
}

impl App {
    pub fn new() -> Self {
        let sessions = tmux::list_claude_sessions();
        let skill_list = skills::load_skills();
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
}
