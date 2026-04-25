//! Ratatui app. Every action the TUI exposes has a CLI equivalent.

pub mod app;
pub mod theme;
pub mod views;

use std::io::{self, stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{App, View};

pub fn run() -> anyhow::Result<()> {
    let mut app = App::new()?;

    enable_raw_mode()?;
    let mut out = stdout();
    out.execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = event_loop(&mut terminal, &mut app);

    let _ = disable_raw_mode();
    let _ = terminal.backend_mut().execute(LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                handle_key(app, key.code, key.modifiers);
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    match app.view {
        View::Profiles => views::profiles::render(f, app),
        View::Diff | View::Progress | View::Log => {
            // Views not yet implemented — show placeholder.
            views::placeholder::render(f, app);
        }
    }
}

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match (code, modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        (KeyCode::Char('j') | KeyCode::Down, _) => app.move_down(),
        (KeyCode::Char('k') | KeyCode::Up, _) => app.move_up(),
        (KeyCode::Char('r'), _) => app.refresh_scan(),
        // Placeholder: other keys acknowledged but not yet wired.
        _ => {}
    }
}
