use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{App, Focus};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Help popup intercepts all keys
    if app.show_help {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') => app.show_help = false,
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::BackTab => {
            app.toggle_mode();
        }
        KeyCode::Tab => {
            app.toggle_focus();
        }
        KeyCode::Char('?') if app.focus == Focus::Results => {
            app.show_help = true;
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
                app.focus = Focus::Results;
            }
        }
        KeyCode::Up => {
            app.select_prev();
        }
        KeyCode::Down => {
            app.select_next();
        }
        _ => {}
    }
}

fn handle_results_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up => app.select_prev(),
        KeyCode::Down => app.select_next(),
        KeyCode::Enter => app.resume_selected(),
        KeyCode::PageDown => app.scroll_detail_down(),
        KeyCode::PageUp => app.scroll_detail_up(),
        KeyCode::Char('j') => app.select_next(),
        KeyCode::Char('k') => app.select_prev(),
        _ => {}
    }
}
