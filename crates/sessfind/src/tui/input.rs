use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{App, Focus, ResultsPane, ResumeOption};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Resume confirmation dialog intercepts all keys
    if app.confirm_resume.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.confirm_resume = None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(state) = &mut app.confirm_resume {
                    state.selected = state.selected.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(state) = &mut app.confirm_resume {
                    state.selected = (state.selected + 1).min(ResumeOption::ALL.len() - 1);
                }
            }
            KeyCode::Enter => {
                let option = app
                    .confirm_resume
                    .as_ref()
                    .map(|s| ResumeOption::ALL[s.selected])
                    .unwrap_or(ResumeOption::Cancel);
                app.confirm_resume_select(option);
            }
            _ => {}
        }
        return;
    }

    // Help popup intercepts all keys
    if app.show_help {
        match key.code {
            KeyCode::Esc | KeyCode::F(1) => {
                app.show_help = false;
                app.help_scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.help_scroll = app.help_scroll.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => app.help_scroll = app.help_scroll.saturating_sub(1),
            KeyCode::PageDown => app.help_scroll = app.help_scroll.saturating_add(10),
            KeyCode::PageUp => app.help_scroll = app.help_scroll.saturating_sub(10),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::BackTab if app.focus == Focus::Search => {
            app.toggle_mode();
        }
        KeyCode::Tab => {
            app.toggle_focus();
        }
        KeyCode::F(1) => {
            app.show_help = !app.show_help;
            if !app.show_help {
                app.help_scroll = 0;
            }
        }
        _ => match app.focus {
            Focus::Search => handle_search_key(app, key),
            Focus::Results => handle_results_key(app, key),
        },
    }
}

fn handle_search_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.clear();
            app.cursor_pos = 0;
            app.on_input_changed();
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_sort();
        }
        KeyCode::Char(c) => {
            app.input.insert(app.cursor_pos, c);
            app.cursor_pos += c.len_utf8();
            app.on_input_changed();
        }
        KeyCode::Backspace => {
            if app.cursor_pos > 0 {
                // Find previous char boundary
                let prev = app.input[..app.cursor_pos]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                app.input.remove(prev);
                app.cursor_pos = prev;
                app.on_input_changed();
            }
        }
        KeyCode::Left => {
            if app.cursor_pos > 0 {
                app.cursor_pos = app.input[..app.cursor_pos]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
            }
        }
        KeyCode::Right => {
            if app.cursor_pos < app.input.len() {
                app.cursor_pos = app.input[app.cursor_pos..]
                    .char_indices()
                    .nth(1)
                    .map(|(i, _)| app.cursor_pos + i)
                    .unwrap_or(app.input.len());
            }
        }
        KeyCode::Enter => {
            // Deferred modes: Enter triggers the search
            if app.search_mode().is_deferred() && !app.input.is_empty() {
                if app.search_mode().is_llm() {
                    app.request_llm_search();
                } else {
                    app.request_semantic_search();
                }
            } else if !app.results.is_empty() {
                app.results_pane = ResultsPane::List;
                app.focus = Focus::Results;
            }
        }
        _ => {}
    }
}

fn handle_results_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Left => {
            app.results_pane = ResultsPane::List;
        }
        KeyCode::Right => {
            app.results_pane = ResultsPane::Preview;
        }
        KeyCode::Up | KeyCode::Char('k') => match app.results_pane {
            ResultsPane::List => app.select_prev(),
            ResultsPane::Preview => app.scroll_detail_up(),
        },
        KeyCode::Down | KeyCode::Char('j') => match app.results_pane {
            ResultsPane::List => app.select_next(),
            ResultsPane::Preview => app.scroll_detail_down(),
        },
        KeyCode::Enter => app.resume_selected(),
        KeyCode::PageUp => app.scroll_detail_top(),
        _ => {}
    }
}
