mod app;
mod session;
mod tmux;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::{App, Mode};

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
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

        if event::poll(Duration::from_millis(500))? {
            if let Event::Key(key) = event::read()? {
                match app.mode {
                    Mode::Normal => {
                        if handle_normal(&mut app, key) {
                            return Ok(());
                        }
                    }
                    Mode::Send => handle_send(&mut app, key),
                    Mode::Worktree => handle_worktree(&mut app, key),
                    Mode::ConfirmKill => handle_confirm_kill(&mut app, key),
                }
            }
        }
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => app.next(),
        KeyCode::Char('k') | KeyCode::Up => app.prev(),
        KeyCode::Enter => {
            if let Some(s) = app.selected_session() {
                tmux::switch_to(&s.addr);
                return true;
            }
        }
        KeyCode::Char('s') => {
            if !app.sessions.is_empty() {
                app.mode = Mode::Send;
                app.input.clear();
                app.input_cursor = 0;
            }
        }
        KeyCode::Char('n') => {
            tmux::new_window("claude-tmux");
            return true;
        }
        KeyCode::Char('w') => {
            // New worktree session — prompt for name
            app.mode = Mode::Worktree;
            app.input.clear();
            app.input_cursor = 0;
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

fn handle_send(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            let input = app.input.trim().to_string();
            if !input.is_empty() {
                if let Some(s) = app.selected_session() {
                    let addr = s.addr.clone();
                    tmux::send_keys(&s.pane_id, &input);
                    app.flash(&format!("Sent to {}", addr));
                }
            }
            app.mode = Mode::Normal;
        }
        KeyCode::Backspace => {
            if key.modifiers.contains(KeyModifiers::ALT) {
                // Alt+Backspace: delete word
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
        }
        KeyCode::Left => {
            if app.input_cursor > 0 {
                app.input_cursor -= 1;
            }
        }
        KeyCode::Right => {
            if app.input_cursor < app.input.len() {
                app.input_cursor += 1;
            }
        }
        KeyCode::Char(c) => {
            app.input.insert(app.input_cursor, c);
            app.input_cursor += 1;
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
                let cmd = format!("claude --dangerously-skip-permissions --worktree {}", name);
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
