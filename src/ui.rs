use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Mode};
use crate::session::SessionState;
use crate::tmux;

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();

    // Main layout: header, body, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(5),   // body
            Constraint::Length(2), // footer
        ])
        .split(size);

    draw_header(f, chunks[0], app);
    draw_body(f, chunks[1], app);
    draw_footer(f, chunks[2], app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let count = app.sessions.len();
    let title = format!(
        " Claude Session Manager  {} session{}",
        count,
        if count == 1 { "" } else { "s" }
    );
    let line = Line::from(vec![
        Span::styled(title, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_body(f: &mut Frame, area: Rect, app: &App) {
    if app.sessions.is_empty() {
        let text = vec![
            Line::from(""),
            Line::from("  No Claude sessions running."),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Press "),
                Span::styled("n", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" to start one."),
            ]),
        ];
        f.render_widget(Paragraph::new(text), area);
        return;
    }

    // Split: left list | right details
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_session_list(f, body[0], app);
    draw_details(f, body[1], app);
}

fn draw_session_list(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Sessions ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Group sessions: project → worktree → sessions
    // Use IndexMap-like ordered grouping
    struct WorktreeGroup<'a> {
        name: String, // branch/worktree label
        sessions: Vec<(usize, &'a crate::session::Session)>,
    }
    struct ProjectGroup<'a> {
        name: String,
        worktrees: Vec<WorktreeGroup<'a>>,
    }

    let mut projects: Vec<ProjectGroup> = Vec::new();
    for (i, s) in app.sessions.iter().enumerate() {
        // Find or create project group
        let proj_idx = projects
            .iter()
            .position(|p| p.name == s.project)
            .unwrap_or_else(|| {
                projects.push(ProjectGroup {
                    name: s.project.clone(),
                    worktrees: Vec::new(),
                });
                projects.len() - 1
            });

        let wt_label = s.worktree_label().to_string();
        let proj = &mut projects[proj_idx];

        // Find or create worktree group
        let wt_idx = proj
            .worktrees
            .iter()
            .position(|w| w.name == wt_label)
            .unwrap_or_else(|| {
                proj.worktrees.push(WorktreeGroup {
                    name: wt_label.clone(),
                    sessions: Vec::new(),
                });
                proj.worktrees.len() - 1
            });

        proj.worktrees[wt_idx].sessions.push((i, s));
    }

    let mut items: Vec<ListItem> = Vec::new();

    for proj in &projects {
        // Project header
        items.push(ListItem::new(Line::from(vec![
            Span::styled(
                format!(" {} ", proj.name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ])));

        for wt in &proj.worktrees {
            // Worktree/branch subheader (if there are multiple or it's a named worktree)
            if proj.worktrees.len() > 1 || !wt.name.is_empty() {
                let branch_label = if wt.name.is_empty() {
                    "main".to_string()
                } else {
                    wt.name.clone()
                };
                items.push(ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("⎇ ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        branch_label,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])));
            }

            for (i, s) in &wt.sessions {
                let is_sel = *i == app.selected;
                let sym_color = s.state.color();
                let indent = if proj.worktrees.len() > 1 || !wt.name.is_empty() {
                    "    "
                } else {
                    "  "
                };

                let mut spans = vec![
                    Span::styled(
                        format!("{}{} ", indent, s.state.symbol()),
                        Style::default().fg(sym_color).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        s.addr.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ];
                if s.is_active {
                    spans.push(Span::styled(" *", Style::default().fg(Color::Green)));
                }
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    s.state.label(),
                    Style::default().fg(sym_color),
                ));

                // Title hint
                if !s.title.is_empty() {
                    let used = indent.len() + 2 + s.addr.len() + 2 + s.state.label().len() + 4;
                    let max_title = (inner.width as usize).saturating_sub(used);
                    if max_title > 5 {
                        let t = if s.title.len() > max_title {
                            format!("{}…", &s.title[..max_title - 1])
                        } else {
                            s.title.clone()
                        };
                        spans.push(Span::styled(
                            format!("  {}", t),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }

                let style = if is_sel {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };

                items.push(ListItem::new(Line::from(spans)).style(style));
            }
        }

        // Spacer between projects
        items.push(ListItem::new(Line::from("")));
    }

    let list = List::new(items);
    f.render_widget(list, inner);
}

fn draw_details(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(s) = app.selected_session() else {
        return;
    };

    // Split details area: info top, preview bottom
    let info_height = 14 + if !s.worktree.is_empty() { 1 } else { 0 };
    let detail_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(info_height), Constraint::Min(3)])
        .split(inner);

    // Info section
    let mut info_lines: Vec<Line> = vec![];

    // Pane
    let mut pane_spans = vec![
        Span::styled("  Pane:     ", Style::default().fg(Color::DarkGray)),
        Span::styled(&s.addr, Style::default().add_modifier(Modifier::BOLD)),
    ];
    if s.is_active {
        pane_spans.push(Span::styled("  active", Style::default().fg(Color::Green)));
    }
    info_lines.push(Line::from(pane_spans));

    // Project
    info_lines.push(Line::from(vec![
        Span::styled("  Project:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(&s.project, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]));

    // Branch
    if !s.branch.is_empty() {
        info_lines.push(Line::from(vec![
            Span::styled("  Branch:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("⎇ {}", s.branch),
                Style::default().fg(Color::Yellow),
            ),
        ]));
    }

    // Worktree
    if !s.worktree.is_empty() {
        info_lines.push(Line::from(vec![
            Span::styled("  Worktree: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&s.worktree, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]));
    }

    // Dir
    info_lines.push(Line::from(vec![
        Span::styled("  Dir:      ", Style::default().fg(Color::DarkGray)),
        Span::raw(&s.short_path),
    ]));

    // State
    info_lines.push(Line::from(vec![
        Span::styled("  State:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} {}", s.state.symbol(), s.state.label()),
            Style::default()
                .fg(s.state.color())
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    info_lines.push(Line::from(""));

    // Title
    info_lines.push(Line::from(vec![
        Span::styled(
            "  Title",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    if s.title.is_empty() {
        info_lines.push(Line::from(Span::styled(
            "  —",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        info_lines.push(Line::from(format!("  {}", s.title)));
    }

    info_lines.push(Line::from(""));

    // Latest task
    info_lines.push(Line::from(vec![
        Span::styled(
            "  Latest Task",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    if !s.task.is_empty() && s.task != s.title {
        info_lines.push(Line::from(format!("  {}", s.task)));
    } else if s.task == s.title && !s.task.is_empty() {
        info_lines.push(Line::from(Span::styled(
            "  (same as title)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        info_lines.push(Line::from(Span::styled(
            "  —",
            Style::default().fg(Color::DarkGray),
        )));
    }

    f.render_widget(Paragraph::new(info_lines).wrap(Wrap { trim: false }), detail_chunks[0]);

    // Preview section
    draw_preview(f, detail_chunks[1], s);
}

fn draw_preview(f: &mut Frame, area: Rect, s: &crate::session::Session) {
    let block = Block::default()
        .title(" Preview ")
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let height = inner.height as usize;
    if height == 0 {
        return;
    }

    let raw_lines = tmux::capture_pane(&s.pane_id, height);

    // Parse ANSI escape sequences and render with proper colors
    let lines: Vec<Line> = raw_lines
        .iter()
        .rev()
        .take(height)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|line| parse_ansi_line(line))
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}

/// Parse a line containing ANSI escape codes into a ratatui Line with styled spans.
fn parse_ansi_line(input: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut style = Style::default();
    let mut buf = String::new();
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Flush current buffer
            if !buf.is_empty() {
                spans.push(Span::styled(buf.clone(), style));
                buf.clear();
            }

            // Parse CSI sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                let mut params = String::new();
                while let Some(&pc) = chars.peek() {
                    if pc.is_ascii_digit() || pc == ';' {
                        params.push(pc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                // Consume the final character
                let final_char = chars.next();
                if final_char == Some('m') {
                    style = apply_sgr(&params, style);
                }
            }
        } else {
            buf.push(c);
        }
    }

    if !buf.is_empty() {
        spans.push(Span::styled(buf, style));
    }

    if spans.is_empty() {
        Line::from("")
    } else {
        Line::from(spans)
    }
}

/// Apply SGR (Select Graphic Rendition) parameters to a style.
fn apply_sgr(params: &str, _current: Style) -> Style {
    let codes: Vec<u16> = params
        .split(';')
        .filter_map(|s| s.parse().ok())
        .collect();

    let mut style = _current;
    let mut i = 0;

    while i < codes.len() {
        match codes[i] {
            0 => style = Style::default(),
            1 => style = style.add_modifier(Modifier::BOLD),
            2 => style = style.add_modifier(Modifier::DIM),
            3 => style = style.add_modifier(Modifier::ITALIC),
            4 => style = style.add_modifier(Modifier::UNDERLINED),
            7 => style = style.add_modifier(Modifier::REVERSED),
            22 => style = style.remove_modifier(Modifier::BOLD | Modifier::DIM),
            23 => style = style.remove_modifier(Modifier::ITALIC),
            24 => style = style.remove_modifier(Modifier::UNDERLINED),
            27 => style = style.remove_modifier(Modifier::REVERSED),
            // Foreground colors
            30..=37 => style = style.fg(ansi_to_color(codes[i] - 30)),
            38 => {
                if i + 1 < codes.len() && codes[i + 1] == 2 && i + 4 < codes.len() {
                    // RGB: 38;2;r;g;b
                    let r = codes[i + 2] as u8;
                    let g = codes[i + 3] as u8;
                    let b = codes[i + 4] as u8;
                    style = style.fg(Color::Rgb(r, g, b));
                    i += 4;
                } else if i + 1 < codes.len() && codes[i + 1] == 5 && i + 2 < codes.len() {
                    // 256-color: 38;5;n
                    style = style.fg(Color::Indexed(codes[i + 2] as u8));
                    i += 2;
                }
            }
            39 => style = style.fg(Color::Reset),
            // Background colors
            40..=47 => style = style.bg(ansi_to_color(codes[i] - 40)),
            48 => {
                if i + 1 < codes.len() && codes[i + 1] == 2 && i + 4 < codes.len() {
                    let r = codes[i + 2] as u8;
                    let g = codes[i + 3] as u8;
                    let b = codes[i + 4] as u8;
                    style = style.bg(Color::Rgb(r, g, b));
                    i += 4;
                } else if i + 1 < codes.len() && codes[i + 1] == 5 && i + 2 < codes.len() {
                    style = style.bg(Color::Indexed(codes[i + 2] as u8));
                    i += 2;
                }
            }
            49 => style = style.bg(Color::Reset),
            90..=97 => style = style.fg(ansi_to_color(codes[i] - 90 + 8)),
            100..=107 => style = style.bg(ansi_to_color(codes[i] - 100 + 8)),
            _ => {}
        }
        i += 1;
    }
    style
}

fn ansi_to_color(n: u16) -> Color {
    match n {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::White,
        8 => Color::DarkGray,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        15 => Color::White,
        _ => Color::Reset,
    }
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let line = match &app.mode {
        Mode::Send => {
            if let Some(s) = app.selected_session() {
                Line::from(vec![
                    Span::styled(
                        format!(" → {}> ", s.addr),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(&app.input),
                ])
            } else {
                Line::from("")
            }
        }
        Mode::Worktree => {
            Line::from(vec![
                Span::styled(
                    " ⎇ Worktree name> ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&app.input),
            ])
        }
        Mode::ConfirmKill => {
            if let Some(s) = app.selected_session() {
                Line::from(vec![
                    Span::styled(
                        format!(" Kill {}? ", s.addr),
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("y/n", Style::default().fg(Color::DarkGray)),
                ])
            } else {
                Line::from("")
            }
        }
        Mode::Normal => {
            if app.flash_active() {
                Line::from(Span::styled(
                    format!(" {}", app.flash_msg),
                    Style::default().fg(Color::Green),
                ))
            } else {
                let keys = vec![
                    ("j/k", "nav"),
                    ("Enter", "switch"),
                    ("s", "send"),
                    ("n", "new"),
                    ("w", "worktree"),
                    ("K", "kill"),
                    ("r", "refresh"),
                    ("q", "quit"),
                ];
                let mut spans: Vec<Span> = vec![Span::raw(" ")];
                for (k, d) in keys {
                    spans.push(Span::styled(
                        k,
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ));
                    spans.push(Span::styled(
                        format!(" {}  ", d),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                Line::from(spans)
            }
        }
    };

    f.render_widget(Paragraph::new(line), area);
}
