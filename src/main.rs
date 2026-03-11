mod app;
mod session;
mod skills;
mod state;
mod task;
mod tmux;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::{App, Mode, ViewMode};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Subcommand: claude-cage state <working|idle|waiting>
    if args.len() >= 2 && args[1] == "state" {
        let st = args.get(2).map(|s| s.as_str()).unwrap_or("unknown");
        std::process::exit(state::handle_state(st));
    }

    // Subcommand: claude-cage task <init|add|status|output|nudge|clear|list>
    if args.len() >= 2 && args[1] == "task" {
        let task_args: Vec<String> = args[2..].to_vec();
        std::process::exit(task::handle_task_cmd(&task_args));
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();
    let mut last_refresh = Instant::now();

    loop {
        // Auto-refresh every 2 seconds
        if last_refresh.elapsed() > Duration::from_secs(2) {
            app.refresh();
            last_refresh = Instant::now();
        }

        terminal.draw(|f| ui::draw(f, &app))?;

        let poll_ms = if app.mode == Mode::Chat || app.mode == Mode::TaskChat { 50 } else { 500 };
        if event::poll(Duration::from_millis(poll_ms))? {
            match event::read()? {
                Event::Key(key) => {
                    match app.mode {
                        Mode::Normal => {
                            let quit = match app.view_mode {
                                ViewMode::Sessions => handle_normal(&mut app, key),
                                ViewMode::Tasks => handle_task_normal(&mut app, key),
                            };
                            if quit {
                                return Ok(());
                            }
                        }
                        Mode::Chat => handle_chat(&mut app, key),
                        Mode::Worktree => handle_worktree(&mut app, key),
                        Mode::ConfirmKill => handle_confirm_kill(&mut app, key),
                        Mode::Skill => handle_skill(&mut app, key),
                        Mode::AddSkillName => handle_add_skill_name(&mut app, key),
                        Mode::AddSkillCommand => handle_add_skill_command(&mut app, key),
                        Mode::Nudge => handle_nudge(&mut app, key),
                        Mode::TaskChat => handle_task_chat(&mut app, key),
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => app.preview_up(),
                        MouseEventKind::ScrollDown => app.preview_down(),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) -> bool {
    // Ctrl+j/k for preview scroll (1 line), Ctrl+u/d for half-page
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('j') => { app.preview_down(); return false; }
            KeyCode::Char('k') => { app.preview_up(); return false; }
            KeyCode::Char('d') => { app.preview_scroll_by(-(15_isize)); return false; }
            KeyCode::Char('u') => { app.preview_scroll_by(15); return false; }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => app.next(),
        KeyCode::Char('k') | KeyCode::Up => app.prev(),
        KeyCode::Enter => {
            if let Some(s) = app.selected_session() {
                tmux::switch_to(&s.addr);
                return true;
            }
        }
        KeyCode::Char('c') => {
            if !app.sessions.is_empty() {
                app.mode = Mode::Chat;
                app.input.clear();
                app.input_cursor = 0;
                app.preview_scroll = 0;
            }
        }
        KeyCode::Char('n') => {
            let claude = tmux::claude_bin();
            tmux::new_window(&claude);
            return true;
        }
        KeyCode::Char('w') => {
            // New worktree session — prompt for name
            app.mode = Mode::Worktree;
            app.input.clear();
            app.input_cursor = 0;
        }
        KeyCode::Char('t') => {
            app.toggle_view();
        }
        KeyCode::Char('S') => {
            if !app.sessions.is_empty() {
                app.mode = Mode::Skill;
                app.input.clear();
                app.input_cursor = 0;
                app.skill_selected = 0;
            }
        }
        KeyCode::Char('K') => {
            if !app.sessions.is_empty() {
                app.mode = Mode::ConfirmKill;
            }
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            app.refresh();
            app.flash("Refreshed");
        }
        KeyCode::Char('q') | KeyCode::Esc => return true,
        _ => {}
    }
    false
}

fn handle_task_normal(app: &mut App, key: KeyEvent) -> bool {
    // Ctrl+u/d for preview scroll
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('d') => { app.preview_scroll_by(-(15_isize)); return false; }
            KeyCode::Char('u') => { app.preview_scroll_by(15); return false; }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => app.task_next(),
        KeyCode::Char('k') | KeyCode::Up => app.task_prev(),
        KeyCode::Enter | KeyCode::Char(' ') => {
            // If task has children, toggle expand. If leaf with pane, switch to it.
            let flat = task::flatten_tree(&app.tasks, &app.task_expanded, 0);
            if let Some(ft) = flat.get(app.task_selected) {
                if ft.has_children {
                    app.toggle_task_expand();
                } else if let Some(ref pid) = ft.task.pane_id {
                    if let Some(s) = app.session_by_pane(pid) {
                        tmux::switch_to(&s.addr);
                        return true;
                    }
                }
            }
        }
        KeyCode::Char('c') => {
            // Chat: if task has a live linked pane, enter TaskChat. Otherwise, nudge.
            if app.selected_task_pane().is_some()
                && app.selected_task_pane().as_ref().and_then(|p| app.session_by_pane(p)).is_some()
            {
                app.start_task_chat();
            } else {
                app.start_nudge();
            }
        }
        KeyCode::Char('m') => {
            // Always nudge (even if task has a pane)
            app.start_nudge();
        }
        KeyCode::Char('t') => {
            app.toggle_view();
        }
        KeyCode::Char('r') | KeyCode::Char('R') => {
            app.refresh();
            app.flash("Refreshed");
        }
        KeyCode::Char('q') | KeyCode::Esc => return true,
        _ => {}
    }
    false
}

fn handle_nudge(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            let msg = app.input.trim().to_string();
            if !msg.is_empty() {
                task::write_nudge(&app.nudge_target_id, &msg);
                app.flash(&format!("Nudge sent to {}", app.nudge_target_id));
            }
            app.mode = Mode::Normal;
        }
        KeyCode::Backspace => {
            if app.input_cursor > 0 {
                app.input_cursor -= 1;
                app.input.remove(app.input_cursor);
            }
        }
        KeyCode::Char(c) => {
            app.input.insert(app.input_cursor, c);
            app.input_cursor += 1;
        }
        _ => {}
    }
}

fn handle_task_chat(app: &mut App, key: KeyEvent) {
    let pane_id = match app.selected_task_pane() {
        Some(p) => p,
        None => {
            app.mode = Mode::Normal;
            return;
        }
    };

    // Ctrl+] to exit
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(']') {
        app.mode = Mode::Normal;
        return;
    }

    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            tmux::send_raw_key(&pane_id, "Enter");
            app.preview_scroll = 0;
        }
        KeyCode::Backspace => {
            tmux::send_raw_key(&pane_id, "BSpace");
        }
        KeyCode::Tab => {
            tmux::send_raw_key(&pane_id, "Tab");
        }
        KeyCode::Up => {
            tmux::send_raw_key(&pane_id, "Up");
        }
        KeyCode::Down => {
            tmux::send_raw_key(&pane_id, "Down");
        }
        KeyCode::Left => {
            tmux::send_raw_key(&pane_id, "Left");
        }
        KeyCode::Right => {
            tmux::send_raw_key(&pane_id, "Right");
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let ctrl_key = format!("C-{}", c);
                tmux::send_raw_key(&pane_id, &ctrl_key);
            } else {
                tmux::send_literal(&pane_id, c);
            }
        }
        _ => {}
    }
}

fn handle_worktree(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            let name = app.input.trim().to_string();
            if !name.is_empty() {
                // Launch claude with --worktree in a new tmux window
                let claude = tmux::claude_bin();
                let cmd = format!("{} --dangerously-skip-permissions --worktree {}", claude, name);
                tmux::new_window(&cmd);
                app.flash(&format!("Worktree '{}' created", name));
            }
            app.mode = Mode::Normal;
        }
        KeyCode::Backspace => {
            if app.input_cursor > 0 {
                app.input_cursor -= 1;
                app.input.remove(app.input_cursor);
            }
        }
        KeyCode::Char(c) => {
            app.input.insert(app.input_cursor, c);
            app.input_cursor += 1;
        }
        _ => {}
    }
}

fn handle_chat(app: &mut App, key: KeyEvent) {
    let Some(s) = app.selected_session() else {
        app.mode = Mode::Normal;
        return;
    };
    let pane_id = s.pane_id.clone();

    // Ctrl+] to exit chat mode (like SSH escape)
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(']') {
        app.mode = Mode::Normal;
        return;
    }

    // Forward everything directly to the target pane
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            tmux::send_raw_key(&pane_id, "Enter");
            app.preview_scroll = 0;
        }
        KeyCode::Backspace => {
            tmux::send_raw_key(&pane_id, "BSpace");
        }
        KeyCode::Tab => {
            tmux::send_raw_key(&pane_id, "Tab");
        }
        KeyCode::Up => {
            tmux::send_raw_key(&pane_id, "Up");
        }
        KeyCode::Down => {
            tmux::send_raw_key(&pane_id, "Down");
        }
        KeyCode::Left => {
            tmux::send_raw_key(&pane_id, "Left");
        }
        KeyCode::Right => {
            tmux::send_raw_key(&pane_id, "Right");
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                // Forward Ctrl+key combos (e.g. Ctrl+C, Ctrl+D)
                let ctrl_key = format!("C-{}", c);
                tmux::send_raw_key(&pane_id, &ctrl_key);
            } else {
                tmux::send_literal(&pane_id, c);
            }
        }
        _ => {}
    }
}

fn handle_confirm_kill(app: &mut App, key: KeyEvent) {
    if key.code == KeyCode::Char('y') {
        if let Some(s) = app.selected_session() {
            let pane_id = s.pane_id.clone();
            tmux::kill_pane(&pane_id);
            session::cleanup_state(&pane_id);
            app.refresh();
            app.flash("Killed");
        }
    }
    app.mode = Mode::Normal;
}

fn handle_skill(app: &mut App, key: KeyEvent) {
    let filtered = skills::filter_and_sort(&app.skills, &app.input);
    let count = filtered.len();

    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            if let Some(&(_, skill)) = filtered.get(app.skill_selected) {
                if let Some(s) = app.selected_session() {
                    let addr = s.addr.clone();
                    let cmd = skill.command.clone();
                    tmux::send_keys(&s.pane_id, &cmd);
                    app.flash(&format!("Ran '{}' on {}", skill.name, addr));
                }
            }
            app.mode = Mode::Normal;
        }
        KeyCode::Up | KeyCode::BackTab => {
            if app.skill_selected > 0 {
                app.skill_selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            if app.skill_selected < count.saturating_sub(1) {
                app.skill_selected += 1;
            }
        }
        KeyCode::Backspace => {
            if key.modifiers.contains(KeyModifiers::ALT) {
                while app.input_cursor > 0
                    && app.input.chars().nth(app.input_cursor - 1) == Some(' ')
                {
                    app.input_cursor -= 1;
                    app.input.remove(app.input_cursor);
                }
                while app.input_cursor > 0
                    && app.input.chars().nth(app.input_cursor - 1) != Some(' ')
                {
                    app.input_cursor -= 1;
                    app.input.remove(app.input_cursor);
                }
            } else if app.input_cursor > 0 {
                app.input_cursor -= 1;
                app.input.remove(app.input_cursor);
            }
            app.skill_selected = 0; // reset selection on input change
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl+a to add a new skill
            app.mode = Mode::AddSkillName;
            app.input.clear();
            app.input_cursor = 0;
        }
        KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl+x to delete selected skill
            if let Some(&(orig_idx, _)) = filtered.get(app.skill_selected) {
                let name = app.skills[orig_idx].name.clone();
                app.skills.remove(orig_idx);
                skills::save_skills(&app.skills);
                app.flash(&format!("Deleted '{}'", name));
                if app.skill_selected >= app.skills.len() && app.skill_selected > 0 {
                    app.skill_selected -= 1;
                }
            }
        }
        KeyCode::Char(c) => {
            app.input.insert(app.input_cursor, c);
            app.input_cursor += 1;
            app.skill_selected = 0;
        }
        _ => {}
    }
}

fn handle_add_skill_name(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            let name = app.input.trim().to_string();
            if !name.is_empty() {
                app.skill_name_buf = name;
                app.mode = Mode::AddSkillCommand;
                app.input.clear();
                app.input_cursor = 0;
            }
        }
        KeyCode::Backspace => {
            if app.input_cursor > 0 {
                app.input_cursor -= 1;
                app.input.remove(app.input_cursor);
            }
        }
        KeyCode::Char(c) => {
            app.input.insert(app.input_cursor, c);
            app.input_cursor += 1;
        }
        _ => {}
    }
}

fn handle_add_skill_command(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            let command = app.input.trim().to_string();
            if !command.is_empty() {
                let name = app.skill_name_buf.clone();
                app.skills.push(skills::Skill {
                    name: name.clone(),
                    command,
                });
                skills::save_skills(&app.skills);
                app.flash(&format!("Added skill '{}'", name));
            }
            app.mode = Mode::Normal;
        }
        KeyCode::Backspace => {
            if app.input_cursor > 0 {
                app.input_cursor -= 1;
                app.input.remove(app.input_cursor);
            }
        }
        KeyCode::Char(c) => {
            app.input.insert(app.input_cursor, c);
            app.input_cursor += 1;
        }
        _ => {}
    }
}
