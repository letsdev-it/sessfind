use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

use chrono::Local;

use super::app::{App, Focus, ResultsPane, ResumeOption};

/// Brand accent color #818CF8
const ACCENT: Color = Color::Rgb(129, 140, 248);
/// Mid-tint for "find" — visible on both dark and light terminals
const ACCENT2: Color = Color::Rgb(150, 160, 248);
/// Dot on `i` in the logo (`▀▀` on row 1, above the stem)
const ACCENT_ORANGE: Color = Color::Rgb(251, 146, 60);

/// Compact `sessfind` (half-blocks); `sess` = accent, `find` = accent2. No shadow.
const BANNER_LINES: [&str; 5] = [
    "                          ▄▄              ▄▄ ",
    "                         ██  ▀▀           ██ ",
    "▄█▀▀▀ ▄█▀█▄ ▄█▀▀▀ ▄█▀▀▀ ▀██▀ ██  ████▄ ▄████ ",
    "▀███▄ ██▄█▀ ▀███▄ ▀███▄  ██  ██  ██ ██ ██ ██ ",
    "▄▄▄█▀ ▀█▄▄▄ ▄▄▄█▀ ▄▄▄█▀  ██  ██▄ ██ ██ ▀████ ",
];

/// First column (0-based) tinted with `ACCENT2` (`find`).
const BANNER_FIND_START_COL: usize = 24;
/// Banner row index (0-based) and columns for the orange `i` dot (`▀▀`).
const BANNER_I_DOT_ROW: usize = 1;
const BANNER_I_DOT_COLS: std::ops::RangeInclusive<usize> = 29..=30;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // banner: 5 logo + blank + subtitle (tight; no extra pad)
            Constraint::Min(5),    // main area
            Constraint::Length(3), // input bar
            Constraint::Length(1), // status bar
        ])
        .split(f.area());

    draw_banner(f, app, chunks[0]);
    draw_main_area(f, app, chunks[1]);
    draw_input_bar(f, app, chunks[2]);
    draw_status_bar(f, app, chunks[3]);

    if app.show_help {
        draw_help_popup(f, f.area(), app.help_scroll);
    }

    if app.confirm_resume.is_some() {
        draw_resume_confirm_popup(f, f.area(), app);
    }
}

fn draw_banner(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for (row, banner_line) in BANNER_LINES.iter().enumerate() {
        let fg_line: Vec<char> = banner_line.chars().collect();

        if fg_line.is_empty() {
            lines.push(Line::from(""));
            continue;
        }

        let mut spans: Vec<Span> = Vec::new();
        let mut current_text = String::new();
        let mut current_color: Option<Color> = None;

        for (i, &ch) in fg_line.iter().enumerate() {
            let ink = ch != ' ';

            let fg_color = if row == BANNER_I_DOT_ROW && BANNER_I_DOT_COLS.contains(&i) && ch == '▀'
            {
                ACCENT_ORANGE
            } else if i < BANNER_FIND_START_COL {
                ACCENT
            } else {
                ACCENT2
            };
            let (out_ch, color) = if ink {
                (ch, fg_color)
            } else {
                (' ', Color::Reset)
            };

            if current_color == Some(color) {
                current_text.push(out_ch);
            } else {
                if !current_text.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut current_text),
                        Style::default().fg(current_color.unwrap_or(Color::Reset)),
                    ));
                }
                current_text.push(out_ch);
                current_color = Some(color);
            }
        }

        if !current_text.is_empty() {
            spans.push(Span::styled(
                current_text,
                Style::default().fg(current_color.unwrap_or(Color::Reset)),
            ));
        }

        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled(
            " Session Finder",
            Style::default()
                .fg(Color::Reset)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            "  https://letsdev.it | github.com/letsdev-it/sessfind",
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, area);

    // Show update available in top-right corner
    if let Some(ref ver) = app.latest_version {
        let text = format!(" v{ver} available ");
        let width = text.len() as u16;
        if area.width > width {
            let update_area = Rect::new(area.right() - width, area.y, width, 1);
            let update_widget = Paragraph::new(Span::styled(
                text,
                Style::default()
                    .fg(ACCENT_ORANGE)
                    .add_modifier(Modifier::BOLD),
            ));
            f.render_widget(update_widget, update_area);
        }
    }
}

fn draw_main_area(f: &mut Frame, app: &App, area: Rect) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_results_list(f, app, panes[0]);
    draw_detail_pane(f, app, panes[1]);
}

fn draw_results_list(f: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.focus == Focus::Results && app.results_pane == ResultsPane::List {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Split into list area + fixed info line at bottom
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let list_area = layout[0];
    let info_area = layout[1];

    let block = Block::default()
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .title(" Sessions ")
        .border_style(border_style);

    if app.semantic_searching || app.llm_searching {
        let label = app.search_mode().label();
        let color = if app.llm_searching {
            Color::Yellow
        } else {
            Color::Cyan
        };
        let p = Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" Searching with {label}"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("...", Style::default().fg(Color::DarkGray)),
        ]))
        .block(block);
        f.render_widget(p, list_area);

        let info_block = Block::default()
            .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
            .border_style(border_style);
        f.render_widget(info_block, info_area);
        return;
    }

    if app.results.is_empty() {
        let msg = if app.input.is_empty() {
            "No indexed sessions.\nRun: sessfind index"
        } else {
            "No results found."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        f.render_widget(p, list_area);

        // Empty info line with bottom border
        let info_block = Block::default()
            .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
            .border_style(border_style);
        f.render_widget(info_block, info_area);
        return;
    }

    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let source_color = match r.source {
                crate::models::Source::ClaudeCode => Color::Magenta,
                crate::models::Source::OpenCode => Color::Cyan,
                crate::models::Source::Copilot => Color::Yellow,
                crate::models::Source::Cursor => Color::Green,
                crate::models::Source::Codex => Color::LightRed,
            };

            let date = r.timestamp.with_timezone(&Local).format("%Y-%m-%d %H:%M");
            let project = truncate_end(&short_project(&r.project), 12);

            let style = if i == app.selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let date_color = if i == app.selected {
                Color::Gray
            } else {
                Color::DarkGray
            };

            let line = Line::from(vec![
                Span::styled(
                    format!(" {:<9}", r.source.as_str()),
                    Style::default().fg(source_color),
                ),
                Span::styled(
                    format!("{:<12} ", project),
                    Style::default().fg(Color::Reset),
                ),
                Span::styled(format!("{}", date), Style::default().fg(date_color)),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    // Compute offset to keep selected visible
    let visible_height = list_area.height.saturating_sub(2) as usize; // minus top border + padding
    let offset = if app.selected >= visible_height {
        app.selected - visible_height + 1
    } else {
        0
    };

    let list = List::new(items[offset..].to_vec()).block(block);
    f.render_widget(list, list_area);

    // Fixed info line at bottom with bottom border
    let count = app.results.len();
    let limit_info = if app.input.is_empty() {
        format!(" {} sessions | date: last activity", count)
    } else {
        format!(" {} results (max 50) | date: last activity", count)
    };

    let info_line = Paragraph::new(Line::from(Span::styled(
        limit_info,
        Style::default().fg(Color::DarkGray),
    )))
    .block(
        Block::default()
            .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
            .border_style(border_style),
    );
    f.render_widget(info_line, info_area);
}

fn draw_detail_pane(f: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.focus == Focus::Results && app.results_pane == ResultsPane::Preview {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    if app.results.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Details ")
            .border_style(border_style);
        let p = Paragraph::new("").block(block);
        f.render_widget(p, area);
        return;
    }

    // Split: scrollable content on top, fixed resume hint at bottom
    let detail_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // scrollable content
            Constraint::Length(2), // fixed resume hint + bottom border
        ])
        .split(area);

    let content_area = detail_layout[0];
    let hint_area = detail_layout[1];

    let block = Block::default()
        .borders(Borders::TOP | Borders::RIGHT | Borders::LEFT)
        .title(" Details ")
        .border_style(border_style);

    let selected = &app.results[app.selected];

    let mut lines: Vec<Line> = Vec::new();

    // Metadata
    let source_color = match selected.source {
        crate::models::Source::ClaudeCode => Color::Magenta,
        crate::models::Source::OpenCode => Color::Cyan,
        crate::models::Source::Copilot => Color::Yellow,
        crate::models::Source::Cursor => Color::Green,
        crate::models::Source::Codex => Color::LightRed,
    };

    lines.push(Line::from(vec![
        Span::styled(" Source:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(selected.source.as_str(), Style::default().fg(source_color)),
    ]));

    lines.push(Line::from(vec![
        Span::styled(" Session: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&selected.session_id, Style::default().fg(Color::Reset)),
    ]));

    lines.push(Line::from(vec![
        Span::styled(" Project: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&selected.project, Style::default().fg(Color::Reset)),
    ]));

    lines.push(Line::from(vec![
        Span::styled(" Date:    ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            selected
                .timestamp
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string(),
            Style::default().fg(Color::Reset),
        ),
    ]));

    if let Some(title) = &selected.title {
        lines.push(Line::from(vec![
            Span::styled(" Title:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(title, Style::default().fg(Color::Reset)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled(" Method:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(app.search_mode().label(), Style::default().fg(Color::Green)),
        Span::styled(
            format!("  Score: {:.2}", selected.score),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " ─── Content ───",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));

    // Content from detail_chunks
    for chunk in &app.detail_chunks {
        for text_line in chunk.snippet.lines() {
            let line = if text_line.starts_with("USER:") {
                Line::from(Span::styled(
                    format!(" {text_line}"),
                    Style::default().fg(Color::Green),
                ))
            } else if text_line.starts_with("ASSISTANT:") {
                Line::from(Span::styled(
                    format!(" {text_line}"),
                    Style::default().fg(Color::Blue),
                ))
            } else if text_line.starts_with("[tools:") {
                Line::from(Span::styled(
                    format!(" {text_line}"),
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                Line::from(format!(" {text_line}"))
            };
            lines.push(line);
        }
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll as u16, 0));

    f.render_widget(paragraph, content_area);

    // Fixed resume hint at bottom with bottom border
    let hint = Paragraph::new(Line::from(Span::styled(
        " Enter → resume this session",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::ITALIC),
    )))
    .block(
        Block::default()
            .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
            .border_style(border_style),
    );

    f.render_widget(hint, hint_area);
}

fn draw_input_bar(f: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.focus == Focus::Search {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mode_label = format!("[{}]", app.search_mode().label());

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    let input_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(mode_label.len() as u16 + 1),
        ])
        .split(inner);

    // Search input
    let input_text = Line::from(vec![
        Span::styled("search> ", Style::default().fg(Color::DarkGray)),
        Span::styled(&app.input, Style::default().fg(Color::Reset)),
    ]);
    let input_widget = Paragraph::new(input_text);
    f.render_widget(input_widget, input_layout[0]);

    // Mode badge color per type
    let mode_color = match app.search_mode() {
        super::app::SearchMode::Llm(_) => Color::Yellow,
        super::app::SearchMode::Semantic => Color::Cyan,
        _ => Color::Green,
    };
    let mode_widget = Paragraph::new(Span::styled(
        &mode_label,
        Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
    ));
    f.render_widget(mode_widget, input_layout[1]);

    // Show cursor
    if app.focus == Focus::Search && !app.show_help {
        let cursor_x = input_layout[0].x + 8 + app.input[..app.cursor_pos].chars().count() as u16;
        let cursor_y = input_layout[0].y;
        f.set_cursor_position((cursor_x, cursor_y));
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let count = app.results.len();

    let mut spans: Vec<Span> = Vec::new();

    match app.focus {
        Focus::Search => {
            spans.push(Span::styled(" Tab→results", Style::default().fg(ACCENT)));
            spans.push(Span::styled(
                "  Ctrl+U ",
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled("clear", Style::default().fg(Color::White)));
            spans.push(Span::styled(
                "  Enter ",
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled("search/go", Style::default().fg(Color::White)));
            spans.push(Span::styled(
                "  Shift+Tab ",
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled("mode", Style::default().fg(Color::White)));
            spans.push(Span::styled("  F1 ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled("help", Style::default().fg(Color::White)));
            spans.push(Span::styled("  Esc ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled("quit", Style::default().fg(Color::White)));
        }
        Focus::Results => {
            spans.push(Span::styled(" Tab→search", Style::default().fg(ACCENT)));
            spans.push(Span::styled(
                "  \u{2190}\u{2192} ",
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled(
                "switch pane",
                Style::default().fg(Color::White),
            ));

            match app.results_pane {
                ResultsPane::List => {
                    spans.push(Span::styled(
                        "  \u{2191}\u{2193} ",
                        Style::default().fg(Color::DarkGray),
                    ));
                    spans.push(Span::styled("navigate", Style::default().fg(Color::White)));
                }
                ResultsPane::Preview => {
                    spans.push(Span::styled(
                        "  \u{2191}\u{2193} ",
                        Style::default().fg(Color::DarkGray),
                    ));
                    spans.push(Span::styled("scroll", Style::default().fg(Color::White)));
                    spans.push(Span::styled(
                        "  PgUp/PgDn ",
                        Style::default().fg(Color::DarkGray),
                    ));
                    spans.push(Span::styled(
                        "top/bottom",
                        Style::default().fg(Color::White),
                    ));
                }
            }

            spans.push(Span::styled(
                "  Enter ",
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::styled("resume", Style::default().fg(Color::White)));
            spans.push(Span::styled("  F1 ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled("help", Style::default().fg(Color::White)));
            spans.push(Span::styled("  Esc ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled("quit", Style::default().fg(Color::White)));
        }
    }

    spans.push(Span::styled(
        format!("  {count} sessions"),
        Style::default().fg(Color::DarkGray),
    ));

    let status = Line::from(spans);
    let bar = Paragraph::new(status).style(Style::default().bg(Color::Black));
    f.render_widget(bar, area);
}

fn draw_help_popup(f: &mut Frame, area: Rect, scroll: usize) {
    let help_text = vec![
        Line::from(Span::styled(
            " Session Finder - Help",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Search Modes (Shift+Tab to switch):",
            Style::default()
                .fg(Color::Reset)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "   FTS (Full-Text Search)",
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                " — tantivy BM25 ranking",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(
            "     Keyword-based search with relevance scoring.",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "     Best for finding specific terms or phrases.",
            Style::default().fg(Color::Reset),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "     Query syntax:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            "       shopping                single keyword",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "       shopping assistant      any of these words (OR)",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "       +shopping +assistant    all words required (AND)",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "       \"shopping assistant\"    exact phrase",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "       shopp*                  prefix wildcard",
            Style::default().fg(Color::Reset),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("   Fuzzy", Style::default().fg(Color::Green)),
            Span::styled(
                " — case-insensitive substring match",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(
            "     Searches in session content, project name, and title.",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "     Useful when FTS doesn't find what you need.",
            Style::default().fg(Color::Reset),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("   Semantic", Style::default().fg(Color::Cyan)),
            Span::styled(
                " — ML embedding similarity (multilingual)",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(
            "     Finds conceptually similar sessions, not just keywords.",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "     Press Enter to search (not instant, uses ML model).",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "     Requires: cargo install sessfind-semantic",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("   LLM", Style::default().fg(Color::Yellow)),
            Span::styled(
                " — re-rank via installed AI CLI tools",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(
            "     FTS narrows candidates, then LLM re-ranks by relevance.",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "     Press Enter to search (not instant, calls LLM).",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "     Detected tools: claude, opencode, copilot (if installed)",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            "     Model: set SESSFIND_LLM_MODEL env var (default: haiku)",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Keybindings:",
            Style::default()
                .fg(Color::Reset)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "   Tab           switch focus between search and results",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "   Shift+Tab     cycle search mode (FTS / Fuzzy / LLM...)",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "   Left/Right    switch between list and preview pane (in results)",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "   Up/Down, j/k  navigate list or scroll preview (context-sensitive)",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "   PgUp/PgDn     scroll preview pane",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "   Enter         resume selected session",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "   Ctrl+U        clear search input",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "   F1            toggle this help",
            Style::default().fg(Color::Reset),
        )),
        Line::from(Span::styled(
            "   Esc           quit (or close this help)",
            Style::default().fg(Color::Reset),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Press Esc or F1 to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    // Center popup: ~76 wide, capped by terminal size
    let popup_w = 76u16.min(area.width.saturating_sub(4));
    let popup_h = (help_text.len() as u16 + 2).min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(x, y, popup_w, popup_h);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .border_style(Style::default().fg(ACCENT));

    let help = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll as u16, 0));

    f.render_widget(help, popup_area);
}

fn draw_resume_confirm_popup(f: &mut Frame, area: Rect, app: &App) {
    let state = match &app.confirm_resume {
        Some(s) => s,
        None => return,
    };

    let source_color = match state.source {
        crate::models::Source::ClaudeCode => Color::Magenta,
        crate::models::Source::OpenCode => Color::Cyan,
        crate::models::Source::Copilot => Color::Yellow,
        crate::models::Source::Cursor => Color::Green,
        crate::models::Source::Codex => Color::LightRed,
    };

    let local_date = state
        .timestamp
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string();

    let title_text = state.title.as_deref().unwrap_or("(untitled)").to_string();

    let session_dir_label = if state.session_dir_exists {
        state.project.clone()
    } else {
        format!("{} (will be created)", state.project)
    };

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".into());

    let options: Vec<(String, bool)> = vec![
        (session_dir_label, state.selected == 0),
        (cwd, state.selected == 1),
        ("Cancel".into(), state.selected == 2),
    ];

    let mut lines: Vec<Line> = Vec::new();

    // Session summary
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {} ", state.source.as_str()),
            Style::default()
                .fg(source_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("· ", Style::default().fg(Color::DarkGray)),
        Span::styled(&local_date, Style::default().fg(Color::Reset)),
    ]));
    lines.push(Line::from(Span::styled(
        format!(" {}", title_text),
        Style::default().fg(Color::Reset),
    )));

    lines.push(Line::from(""));

    // Question
    lines.push(Line::from(Span::styled(
        " Where do you want to resume the session?",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Options
    for (i, (label, selected)) in options.iter().enumerate() {
        let option = ResumeOption::ALL[i];
        let (prefix, style) = if *selected {
            (
                " ▸ ",
                Style::default()
                    .fg(if option == ResumeOption::Cancel {
                        Color::Red
                    } else {
                        ACCENT
                    })
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                "   ",
                Style::default().fg(if option == ResumeOption::Cancel {
                    Color::DarkGray
                } else {
                    Color::Reset
                }),
            )
        };
        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(label, style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " ↑↓ select  Enter confirm  Esc cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let popup_w = 72u16.min(area.width.saturating_sub(4));
    let popup_h = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(popup_w)) / 2;
    let y = (area.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(x, y, popup_w, popup_h);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Resume Session ")
        .border_style(Style::default().fg(ACCENT));

    let popup = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(popup, popup_area);
}

fn short_project(project: &str) -> String {
    // Extract last path component
    project
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(project)
        .to_string()
}

fn truncate_end(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        let truncated: String = chars[..max - 1].iter().collect();
        format!("{truncated}\u{2026}")
    }
}
