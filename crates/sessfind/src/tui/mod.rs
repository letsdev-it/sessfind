mod app;
mod input;
mod ui;

use std::io::stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::indexer::engine::IndexEngine;

pub use sessfind_common::CommandSpec as ResumeCommand;

pub fn run(engine: &IndexEngine, initial_mode: Option<&str>) -> Result<Option<ResumeCommand>> {
    // Validate catalog and initial mode before taking control of the terminal.
    let mut app = app::App::new(engine, initial_mode)?;

    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        app.poll_pending_search();

        if let Ok(Some(ver)) = app.update_rx.try_recv()
            && ver != env!("CARGO_PKG_VERSION")
        {
            app.latest_version = Some(ver);
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key)
                    // Ignore key release events on some terminals
                    if key.kind == crossterm::event::KeyEventKind::Press =>
                {
                    input::handle_key(&mut app, key);
                }
                Event::Paste(text) if app.focus == app::Focus::Search => {
                    input::handle_paste(&mut app, &text);
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(DisableBracketedPaste)?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(app.resume_command())
}
