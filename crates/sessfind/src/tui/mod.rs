mod app;
mod input;
mod ui;

use std::io::stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::indexer::engine::IndexEngine;

pub use app::ResumeCommand;

pub fn run(engine: &IndexEngine) -> Result<Option<ResumeCommand>> {
    // Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = app::App::new(engine)?;

    // Main loop
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // If deferred search was requested, run it now (after UI redraw showed "Searching...")
        if app.semantic_searching {
            app.run_pending_semantic_search();
            continue;
        }
        if app.llm_searching {
            app.run_pending_llm_search();
            continue;
        }

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            // Ignore key release events on some terminals
            if key.kind == crossterm::event::KeyEventKind::Press {
                input::handle_key(&mut app, key);
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(app.resume_command())
}
