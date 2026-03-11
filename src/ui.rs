use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Mode, ViewMode};
use crate::board;
use crate::session::SessionState;
use crate::skills;
use crate::task;
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

    // Skill picker overlay
    if app.mode == Mode::Skill || app.mode == Mode::AddSkillName || app.mode == Mode::AddSkillCommand {
        draw_skill_popup(f, size, app);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![];

    // Title
    spans.push(Span::styled(
        " claude-cage ",
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::styled("  ", Style::default()));

    // Tab bar: Sessions | Tasks | Board
    let session_count = app.sessions.len();
    let task_total = count_all_tasks(&app.tasks);
    let task_active = count_active_tasks(&app.tasks);
    let board_count = app.board_entries.len();

    // Sessions tab
    let sess_label = format!(" 1 Sessions({}) ", session_count);
    if app.view_mode == ViewMode::Sessions {
        spans.push(Span::styled(sess_label, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    } else {
        spans.push(Span::styled(sess_label, Style::default().fg(Color::DarkGray)));
    }

    spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));

    // Tasks tab
    let tasks_label = if task_active > 0 {
        format!(" 2 Tasks({}/{}) ", task_active, task_total)
    } else {
        format!(" 2 Tasks({}) ", task_total)
    };
    if app.view_mode == ViewMode::Tasks {
        spans.push(Span::styled(tasks_label, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
    } else if task_active > 0 {
        spans.push(Span::styled(tasks_label, Style::default().fg(Color::Yellow)));
    } else {
        spans.push(Span::styled(tasks_label, Style::default().fg(Color::DarkGray)));
    }

    spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));

    // Board tab
    let board_label = format!(" 3 Board({}) ", board_count);
    if app.view_mode == ViewMode::Board {
        spans.push(Span::styled(board_label, Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)));
    } else if board_count > 0 {
        spans.push(Span::styled(board_label, Style::default().fg(Color::Gray)));
    } else {
        spans.push(Span::styled(board_label, Style::default().fg(Color::DarkGray)));
    }

    // Context info after tabs
    if app.view_mode == ViewMode::Board {
        if let Some(ref tag) = app.board_filter {
            spans.push(Span::styled(
                format!("  filter: {}", tag.label()),
                Style::default().fg(tag.color()),
            ));
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn count_all_tasks(tasks: &[task::Task]) -> usize {
    let mut count = tasks.len();
    for t in tasks {
        count += count_all_tasks(&t.subtasks);
    }
    count
}

fn count_active_tasks(tasks: &[task::Task]) -> usize {
    let mut count = 0;
    for t in tasks {
        if t.status == task::TaskStatus::InProgress {
            count += 1;
        }
        count += count_active_tasks(&t.subtasks);
    }
    count
}

fn draw_body(f: &mut Frame, area: Rect, app: &App) {
    match app.view_mode {
        ViewMode::Sessions => draw_body_sessions(f, area, app),
        ViewMode::Tasks => draw_body_tasks(f, area, app),
        ViewMode::Board => draw_body_board(f, area, app),
    }
}

fn draw_body_sessions(f: &mut Frame, area: Rect, app: &App) {
    if app.sessions.is_empty() {
        let text = vec![
            Line::from(""),
            Line::from("  No Claude sessions running."),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Press "),
                Span::styled("n", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" to start one, or "),
                Span::styled("2", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" for task view, "),
                Span::styled("3", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" for board."),
            ]),
        ];
        f.render_widget(Paragraph::new(text), area);
        return;
    }

    // Split: left list | right details/chat
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_session_list(f, body[0], app);

    if app.mode == Mode::Chat {
        draw_chat_view(f, body[1], app);
    } else {
        draw_details(f, body[1], app);
    }
}

fn draw_body_tasks(f: &mut Frame, area: Rect, app: &App) {
    if app.tasks.is_empty() {
        let text = vec![
            Line::from(""),
            Line::from("  No tasks yet."),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Use "),
                Span::styled("/orchestrate", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" or "),
                Span::styled("/architect", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" to create tasks."),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Press "),
                Span::styled("1", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" for sessions, "),
                Span::styled("3", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" for board."),
            ]),
        ];
        f.render_widget(Paragraph::new(text), area);
        return;
    }

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_task_list(f, body[0], app);

    if app.mode == Mode::TaskChat {
        draw_task_chat_view(f, body[1], app);
    } else {
        draw_task_details(f, body[1], app);
    }
}

fn draw_body_board(f: &mut Frame, area: Rect, app: &App) {
    if app.board_entries.is_empty() {
        let text = vec![
            Line::from(""),
            Line::from("  No board entries yet."),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Agents post findings via "),
                Span::styled(
                    "claude-cage board post",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Press "),
                Span::styled("1", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" for sessions, "),
                Span::styled("2", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" for tasks."),
            ]),
        ];
        f.render_widget(Paragraph::new(text), area);
        return;
    }

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    draw_board_list(f, body[0], app);
    draw_board_detail(f, body[1], app);
}

fn draw_board_list(f: &mut Frame, area: Rect, app: &App) {
    let filter_label = match &app.board_filter {
        Some(tag) => format!(" Board [{}] ", tag.label()),
        None => " Board ".to_string(),
    };
    let block = Block::default()
        .title(filter_label)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let indices = app.board_visible_indices();
    if indices.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No entries match filter",
                Style::default().fg(Color::DarkGray),
            ))),
            inner,
        );
        return;
    }

    let height = inner.height as usize;
    // Scroll so selected item is visible
    let scroll_start = if app.board_selected >= height {
        app.board_selected - height + 1
    } else {
        0
    };

    let items: Vec<ListItem> = indices
        .iter()
        .enumerate()
        .skip(scroll_start)
        .take(height)
        .map(|(i, &entry_idx)| {
            let entry = &app.board_entries[entry_idx];
            let selected = i == app.board_selected;

            let mut spans = vec![];

            // Pin indicator
            if entry.pinned {
                spans.push(Span::styled(
                    " * ",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::raw("   "));
            }

            // Tag badge
            spans.push(Span::styled(
                format!("{} ", entry.tag.symbol()),
                Style::default().fg(entry.tag.color()).add_modifier(Modifier::BOLD),
            ));

            // Time
            spans.push(Span::styled(
                format!("{:>4} ", board::relative_time(entry.timestamp)),
                Style::default().fg(Color::DarkGray),
            ));

            // Role badge
            if !entry.role.is_empty() {
                spans.push(Span::styled(
                    format!("[{}] ", entry.role),
                    Style::default().fg(task::role_color(&entry.role)),
                ));
            }

            // Content preview (truncated)
            let max_content = (inner.width as usize).saturating_sub(
                spans.iter().map(|s| s.content.len()).sum::<usize>(),
            );
            let content_preview: String = entry.content.lines().next().unwrap_or("").chars().take(max_content).collect();
            spans.push(Span::styled(
                content_preview,
                if selected {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ));

            let style = if selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();

    f.render_widget(List::new(items), inner);
}

fn draw_board_detail(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Entry Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(entry) = app.selected_board_entry() else {
        return;
    };

    let mut lines: Vec<Line> = vec![];

    // Tag + time
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{} {}", entry.tag.symbol(), entry.tag.label()),
            Style::default().fg(entry.tag.color()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {} ago", board::relative_time(entry.timestamp)),
            Style::default().fg(Color::DarkGray),
        ),
        if entry.pinned {
            Span::styled("  *pinned*", Style::default().fg(Color::Yellow))
        } else {
            Span::raw("")
        },
    ]));

    // Source
    let source_label = if entry.task_id.is_empty() {
        "user".to_string()
    } else {
        entry.task_id.clone()
    };
    lines.push(Line::from(vec![
        Span::styled("  from: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            &source_label,
            Style::default().fg(task::role_color(&entry.role)),
        ),
        if !entry.role.is_empty() {
            Span::styled(
                format!(" ({})", entry.role),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            Span::raw("")
        },
    ]));

    // Directed to
    if let Some(ref to) = entry.directed_to {
        lines.push(Line::from(vec![
            Span::styled("  to:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(to, Style::default().fg(Color::Cyan)),
        ]));
    }

    // Reply to
    if let Some(ref reply_id) = entry.reply_to {
        lines.push(Line::from(vec![
            Span::styled("  re:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(reply_id, Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::from(""));

    // Content (full, multi-line, scrollable)
    let content_lines: Vec<Line> = entry
        .content
        .lines()
        .map(|l| Line::from(format!("  {}", l)))
        .collect();

    let total_lines = lines.len() + content_lines.len();
    let visible_height = inner.height as usize;

    // Add content lines
    lines.extend(content_lines);

    // Add reply thread
    let replies: Vec<&board::BoardEntry> = app
        .board_entries
        .iter()
        .filter(|e| e.reply_to.as_deref() == Some(&entry.id))
        .collect();

    if !replies.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  --- {} replies ---", replies.len()),
            Style::default().fg(Color::DarkGray),
        )));
        for reply in &replies {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    if reply.task_id.is_empty() {
                        "user"
                    } else {
                        &reply.task_id
                    },
                    Style::default().fg(task::role_color(&reply.role)).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {} ago", board::relative_time(reply.timestamp)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            for line in reply.content.lines() {
                lines.push(Line::from(format!("    {}", line)));
            }
        }
    }

    // Scroll
    let skip = if lines.len() > visible_height {
        let max_scroll = lines.len() - visible_height;
        max_scroll.saturating_sub(app.preview_scroll)
    } else {
        0
    };
    let visible: Vec<Line> = lines.into_iter().skip(skip).collect();

    f.render_widget(Paragraph::new(visible), inner);
}

fn draw_chat_view(f: &mut Frame, area: Rect, app: &App) {
    let Some(s) = app.selected_session() else { return };

    let title = format!(" {} — {}  (Esc to exit) ", s.addr, s.project);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let height = inner.height as usize;
    if height > 0 {
        let capture_lines = height + app.preview_scroll;
        let raw_lines = tmux::capture_pane(&s.pane_id, capture_lines);
        let total = raw_lines.len();
        let end = total.saturating_sub(app.preview_scroll);
        let start = end.saturating_sub(height);
        let lines: Vec<Line> = raw_lines[start..end]
            .iter()
            .map(|line| parse_ansi_line(line))
            .collect();
        f.render_widget(Paragraph::new(lines), inner);
    }
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
    let ctx_lines = if s.context.is_some() { 5 } else { 2 };
    let info_height = 14 + ctx_lines + if !s.worktree.is_empty() { 1 } else { 0 };
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

    // Context usage
    info_lines.push(Line::from(""));
    info_lines.push(Line::from(vec![
        Span::styled(
            "  Context",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    if let Some(ctx) = &s.context {
        // Show context bar (200k window)
        let max_context: u64 = 200_000;
        let pct = ((ctx.total_context as f64 / max_context as f64) * 100.0).min(100.0);
        let bar_width = (inner.width as usize).saturating_sub(6);
        let filled = ((pct / 100.0) * bar_width as f64) as usize;
        let empty = bar_width.saturating_sub(filled);

        let bar_color = if pct > 90.0 {
            Color::Red
        } else if pct > 70.0 {
            Color::Yellow
        } else {
            Color::Green
        };

        info_lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "█".repeat(filled),
                Style::default().fg(bar_color),
            ),
            Span::styled(
                "░".repeat(empty),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" {:.0}%", pct),
                Style::default().fg(bar_color).add_modifier(Modifier::BOLD),
            ),
        ]));

        info_lines.push(Line::from(vec![
            Span::styled("  Tokens: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format_tokens(ctx.total_context)),
            Span::styled(" / 200k", Style::default().fg(Color::DarkGray)),
            Span::raw("    "),
            Span::styled("out: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format_tokens(ctx.output_tokens)),
        ]));

        if ctx.cache_read > 0 || ctx.cache_create > 0 {
            info_lines.push(Line::from(vec![
                Span::styled("  Cache:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}  read", format_tokens(ctx.cache_read)),
                    Style::default().fg(Color::Green),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{}  new", format_tokens(ctx.cache_create)),
                    Style::default().fg(Color::Yellow),
                ),
            ]));
        }
    } else {
        info_lines.push(Line::from(Span::styled(
            "  —",
            Style::default().fg(Color::DarkGray),
        )));
    }

    f.render_widget(Paragraph::new(info_lines).wrap(Wrap { trim: false }), detail_chunks[0]);

    // Preview section
    draw_preview(f, detail_chunks[1], s, app.preview_scroll);
}

fn draw_preview(f: &mut Frame, area: Rect, s: &crate::session::Session, scroll: usize) {
    let title = if scroll > 0 {
        format!(" Preview [+{}] ", scroll)
    } else {
        " Preview ".to_string()
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let height = inner.height as usize;
    if height == 0 {
        return;
    }

    // Capture more lines to allow scrolling back
    let capture_lines = height + scroll;
    let raw_lines = tmux::capture_pane(&s.pane_id, capture_lines);

    // Parse ANSI escape sequences and render with proper colors
    // Take from the end, skip `scroll` lines, then take `height` visible lines
    let total = raw_lines.len();
    let end = total.saturating_sub(scroll);
    let start = end.saturating_sub(height);

    let lines: Vec<Line> = raw_lines[start..end]
        .iter()
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

fn draw_skill_popup(f: &mut Frame, area: Rect, app: &App) {
    // Center popup: 50% width, up to 60% height
    let popup_w = (area.width as f32 * 0.5).max(30.0).min(area.width as f32) as u16;
    let popup_h = (area.height as f32 * 0.6).max(8.0).min(area.height as f32) as u16;
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(x, y, popup_w, popup_h);

    // Clear background
    f.render_widget(Clear, popup_area);

    let title = match app.mode {
        Mode::AddSkillName => " New Skill — Name ",
        Mode::AddSkillCommand => &format!(" New Skill '{}' — Command ", app.skill_name_buf),
        _ => " Skills ",
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    if app.mode == Mode::AddSkillName || app.mode == Mode::AddSkillCommand {
        // Simple input prompt
        let prompt_label = if app.mode == Mode::AddSkillName {
            "Name: "
        } else {
            "Command: "
        };
        let line = Line::from(vec![
            Span::styled(prompt_label, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(&app.input),
            Span::styled("_", Style::default().fg(Color::DarkGray)),
        ]);
        f.render_widget(Paragraph::new(line), inner);
        return;
    }

    // Skill mode: search bar + filtered list
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    // Search input
    let search_line = Line::from(vec![
        Span::styled(" > ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(&app.input),
        Span::styled("_", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(search_line), layout[0]);

    // Hint line
    let hint = Line::from(vec![
        Span::styled("  Tab", Style::default().fg(Color::Green)),
        Span::styled("/", Style::default().fg(Color::DarkGray)),
        Span::styled("Shift-Tab", Style::default().fg(Color::Green)),
        Span::styled(" nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Green)),
        Span::styled(" run  ", Style::default().fg(Color::DarkGray)),
        Span::styled("C-a", Style::default().fg(Color::Green)),
        Span::styled(" add  ", Style::default().fg(Color::DarkGray)),
        Span::styled("C-x", Style::default().fg(Color::Green)),
        Span::styled(" del", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(hint), layout[1]);

    // Filtered skill list
    let filtered = skills::filter_and_sort(&app.skills, &app.input);
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, (_, skill))| {
            let is_sel = i == app.skill_selected;
            let marker = if is_sel { ">" } else { " " };
            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", marker),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    &skill.name,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", skill.command),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            let style = if is_sel {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    if items.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled("  No matching skills", Style::default().fg(Color::DarkGray))),
            layout[2],
        );
    } else {
        f.render_widget(List::new(items), layout[2]);
    }
}

fn draw_task_chat_view(f: &mut Frame, area: Rect, app: &App) {
    let pane_id = match app.selected_task_pane() {
        Some(p) => p,
        None => return,
    };
    let session = app.session_by_pane(&pane_id);
    let title_name = app.selected_task().map(|t| t.name.as_str()).unwrap_or("?");
    let title = format!(" {} — chat  (Esc to exit) ", title_name);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let pid = match session {
        Some(s) => &s.pane_id,
        None => return,
    };

    let height = inner.height as usize;
    if height > 0 {
        let capture_lines = height + app.preview_scroll;
        let raw_lines = tmux::capture_pane(pid, capture_lines);
        let total = raw_lines.len();
        let end = total.saturating_sub(app.preview_scroll);
        let start = end.saturating_sub(height);
        let lines: Vec<Line> = raw_lines[start..end]
            .iter()
            .map(|line| parse_ansi_line(line))
            .collect();
        f.render_widget(Paragraph::new(lines), inner);
    }
}

fn draw_task_list(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Tasks ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let flat = task::flatten_tree(&app.tasks, &app.task_expanded, 0);

    let mut items: Vec<ListItem> = Vec::new();
    for (i, ft) in flat.iter().enumerate() {
        let is_sel = i == app.task_selected;
        let indent = "  ".repeat(ft.depth);

        // Tree connector
        let connector = if ft.has_children {
            if ft.is_expanded { "▼ " } else { "▶ " }
        } else {
            "· "
        };

        // Status icon
        let status_sym = ft.task.status.symbol();
        let status_color = ft.task.status.color();

        // Role badge
        let role_col = task::role_color(&ft.task.role);

        // Truncate name to fit
        let prefix_len = ft.depth * 2 + 2 + 2; // indent + connector + status+space
        let suffix_len = if ft.task.role.is_empty() { 0 } else { ft.task.role.len() + 3 }; // " role"
        let max_name = (inner.width as usize)
            .saturating_sub(prefix_len + suffix_len + 2);
        let name = if ft.task.name.len() > max_name && max_name > 2 {
            format!("{}…", &ft.task.name[..max_name - 1])
        } else {
            ft.task.name.clone()
        };

        let mut spans = vec![
            Span::raw(indent),
            Span::styled(
                connector,
                Style::default().fg(if ft.has_children { Color::White } else { Color::DarkGray }),
            ),
            Span::styled(
                format!("{} ", status_sym),
                Style::default().fg(status_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(name),
        ];

        // Role badge
        if !ft.task.role.is_empty() {
            spans.push(Span::styled(
                format!("  {}", ft.task.role),
                Style::default().fg(role_col),
            ));
        }

        // Session link indicator
        if let Some(ref pid) = ft.task.pane_id {
            let session_state = app.session_by_pane(pid);
            let (sym, col) = match session_state {
                Some(s) => (s.state.symbol(), s.state.color()),
                None => ("✕", Color::DarkGray), // session gone
            };
            spans.push(Span::styled(
                format!(" {}", sym),
                Style::default().fg(col),
            ));
        }

        let style = if is_sel {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else {
            Style::default()
        };

        items.push(ListItem::new(Line::from(spans)).style(style));
    }

    let list = List::new(items);
    f.render_widget(list, inner);
}

/// Calculate the compact info header height for a task.
fn info_height(t: &task::Task) -> u16 {
    let mut h: u16 = 4; // name + status/role + session + separator
    if task::has_nudge(&t.id) {
        h += 1;
    }
    h
}

fn draw_task_details(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Task Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let flat = task::flatten_tree(&app.tasks, &app.task_expanded, 0);
    let Some(ft) = flat.get(app.task_selected) else {
        return;
    };
    let t = ft.task;

    // Check if this task has a linked session with a live pane
    let linked_session = t.pane_id.as_ref().and_then(|pid| app.session_by_pane(pid));
    let has_preview = linked_session.is_some();
    let has_output = !t.output.is_empty();

    // Layout: compact header (fixed) + output (flex) + preview (if linked)
    // Header is ~4–6 lines. Output gets the bulk. Preview gets 40% if present.
    let detail_chunks = if has_preview {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(info_height(t)),
                Constraint::Min(3),        // output
                Constraint::Percentage(40), // live preview
            ])
            .split(inner)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(info_height(t)),
                Constraint::Min(3), // output gets all remaining space
            ])
            .split(inner)
    };

    // ── Compact info header ──
    let mut info_lines: Vec<Line> = vec![];

    // Line 1: Name (bold)
    info_lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            &t.name,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
    ]));

    // Line 2: Status + Role on same line
    let mut status_spans = vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{} {}", t.status.symbol(), t.status.label()),
            Style::default().fg(t.status.color()).add_modifier(Modifier::BOLD),
        ),
    ];
    if !t.role.is_empty() {
        status_spans.push(Span::styled("  ", Style::default()));
        status_spans.push(Span::styled(
            format!("[{}]", &t.role),
            Style::default().fg(task::role_color(&t.role)),
        ));
    }
    // Subtask progress inline
    if !t.subtasks.is_empty() {
        let total = t.subtasks.len();
        let done = t.subtasks.iter().filter(|s| s.status == task::TaskStatus::Completed).count();
        let in_prog = t.subtasks.iter().filter(|s| s.status == task::TaskStatus::InProgress).count();
        status_spans.push(Span::styled("  ", Style::default()));
        status_spans.push(Span::styled(format!("{}", done), Style::default().fg(Color::Green)));
        status_spans.push(Span::styled(format!("/{}", total), Style::default().fg(Color::DarkGray)));
        status_spans.push(Span::styled(" subtasks", Style::default().fg(Color::DarkGray)));
        if in_prog > 0 {
            status_spans.push(Span::styled(
                format!(" ({} active)", in_prog),
                Style::default().fg(Color::Yellow),
            ));
        }
    }
    info_lines.push(Line::from(status_spans));

    // Line 3: Session link
    let session_span = match &linked_session {
        Some(s) => Span::styled(
            format!("{} {} {}", s.addr, s.state.symbol(), s.state.label()),
            Style::default().fg(s.state.color()),
        ),
        None => match &t.pane_id {
            Some(pid) => Span::styled(
                format!("{} (gone)", pid),
                Style::default().fg(Color::DarkGray),
            ),
            None => Span::styled("no session", Style::default().fg(Color::DarkGray)),
        },
    };
    info_lines.push(Line::from(vec![
        Span::styled("  session: ", Style::default().fg(Color::DarkGray)),
        session_span,
    ]));

    // Nudge pending
    if task::has_nudge(&t.id) {
        info_lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                "NUDGE PENDING",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    // Separator
    info_lines.push(Line::from(""));

    f.render_widget(
        Paragraph::new(info_lines),
        detail_chunks[0],
    );

    // ── Output section (scrollable) ──
    let output_area = detail_chunks[1];
    let output_block = Block::default()
        .title(if has_output {
            format!(" Output ({} lines) ", t.output.lines().count())
        } else {
            " Output ".to_string()
        })
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));

    let output_inner = output_block.inner(output_area);
    f.render_widget(output_block, output_area);

    if has_output {
        let output_lines: Vec<Line> = t.output.lines()
            .map(|l| Line::from(format!("  {}", l)))
            .collect();

        let total_lines = output_lines.len();
        let visible_height = output_inner.height as usize;

        // Scroll: preview_scroll=0 means bottom (latest output visible)
        let skip = if total_lines > visible_height {
            let max_scroll = total_lines - visible_height;
            // preview_scroll acts as "lines from bottom"
            max_scroll.saturating_sub(app.preview_scroll)
        } else {
            0
        };

        let visible: Vec<Line> = output_lines.into_iter().skip(skip).collect();
        f.render_widget(
            Paragraph::new(visible),
            output_inner,
        );
    } else {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  Waiting for output...",
                Style::default().fg(Color::DarkGray),
            ))),
            output_inner,
        );
    }

    // ── Live preview (if linked session) ──
    if has_preview {
        if let Some(s) = linked_session {
            draw_preview(f, detail_chunks[2], s, app.preview_scroll);
        }
    }
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let line = match &app.mode {
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
        Mode::Chat | Mode::TaskChat => {
            Line::from("") // chat view handles its own footer
        }
        Mode::Nudge => {
            Line::from(vec![
                Span::styled(
                    " Nudge> ",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&app.input),
                Span::styled("_", Style::default().fg(Color::DarkGray)),
            ])
        }
        Mode::BoardReply => {
            Line::from(vec![
                Span::styled(
                    " Reply> ",
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&app.input),
                Span::styled("_", Style::default().fg(Color::DarkGray)),
            ])
        }
        Mode::Skill | Mode::AddSkillName | Mode::AddSkillCommand => {
            Line::from("")  // popup handles its own display
        }
        Mode::Normal => {
            if app.flash_active() {
                Line::from(Span::styled(
                    format!(" {}", app.flash_msg),
                    Style::default().fg(Color::Green),
                ))
            } else {
                let keys = match app.view_mode {
                    ViewMode::Sessions => vec![
                        ("j/k", "nav"),
                        ("C-u/d", "scroll"),
                        ("Enter", "switch"),
                        ("c", "chat"),
                        ("S", "skills"),
                        ("n", "new"),
                        ("w", "worktree"),
                        ("K", "kill"),
                        ("1/2/3", "views"),
                        ("r", "refresh"),
                        ("q", "quit"),
                    ],
                    ViewMode::Tasks => vec![
                        ("j/k", "nav"),
                        ("Enter", "expand"),
                        ("c", "chat"),
                        ("m", "nudge"),
                        ("C-u/d", "scroll"),
                        ("1/2/3", "views"),
                        ("r", "refresh"),
                        ("q", "quit"),
                    ],
                    ViewMode::Board => vec![
                        ("j/k", "nav"),
                        ("c", "reply"),
                        ("f", "filter"),
                        ("p", "pin"),
                        ("C-u/d", "scroll"),
                        ("1/2/3", "views"),
                        ("r", "refresh"),
                        ("q", "quit"),
                    ],
                };
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
