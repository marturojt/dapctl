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

use app::{App, DiffState, View};

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

        // Run any pending computation immediately after showing its loading frame.
        if matches!(app.diff_state, DiffState::Loading) {
            compute_diff(app);
            if app.should_quit {
                break;
            }
            continue; // re-draw with result without waiting for an event
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                handle_key(app, key.code, key.modifiers);
            }
        }

        app.tick_flash();

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    match app.view {
        View::Profiles => views::profiles::render(f, app),
        View::Diff => views::diff::render(f, app),
        View::Progress | View::Log => views::placeholder::render(f, app),
    }
}

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match (code, modifiers) {
        // Always: ctrl-c quits
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }

        // q: quit from profiles, back to profiles from any other view
        (KeyCode::Char('q'), _) => match app.view {
            View::Profiles => app.should_quit = true,
            _ => app.view = View::Profiles,
        },

        // esc: always back to profiles (no-op if already there)
        (KeyCode::Esc, _) => {
            app.view = View::Profiles;
        }

        // ── Profiles view ────────────────────────────────────────────────
        (KeyCode::Char('j') | KeyCode::Down, _) if app.view == View::Profiles => {
            app.move_down();
        }
        (KeyCode::Char('k') | KeyCode::Up, _) if app.view == View::Profiles => {
            app.move_up();
        }
        (KeyCode::Enter, _)
            if app.view == View::Profiles && !app.profiles.is_empty() =>
        {
            app.enter_diff();
        }
        (KeyCode::Char('s'), _) if app.view == View::Profiles => {
            app.set_flash(
                "sync from TUI — coming soon  (use  dapctl sync <profile>  for now)",
            );
        }
        (KeyCode::Char('r'), _) if app.view == View::Profiles => {
            app.refresh_scan();
        }

        // ── Diff view ────────────────────────────────────────────────────
        (KeyCode::Char('j') | KeyCode::Down, _) if app.view == View::Diff => {
            app.move_diff_down();
        }
        (KeyCode::Char('k') | KeyCode::Up, _) if app.view == View::Diff => {
            app.move_diff_up();
        }
        (KeyCode::Tab, _) if app.view == View::Diff => {
            app.cycle_diff_filter();
        }
        (KeyCode::Char('y'), _) if app.view == View::Diff => {
            app.set_flash(
                "sync from TUI — coming soon  (use  dapctl sync <profile>  for now)",
            );
        }
        // r: re-run diff (useful if library changed)
        (KeyCode::Char('r'), _) if app.view == View::Diff => {
            app.enter_diff();
        }

        _ => {}
    }
}

// ── Diff computation ───────────────────────────────────────────────────────

fn compute_diff(app: &mut App) {
    let profile_name = app
        .profiles
        .get(app.profile_idx)
        .map(|(_, p)| p.profile.name.clone())
        .unwrap_or_default();

    if profile_name.is_empty() {
        app.diff_state = DiffState::Error("no profile selected".to_owned());
        return;
    }

    let resolved = match crate::config::resolve(&profile_name) {
        Ok(r) => r,
        Err(e) => {
            app.diff_state = DiffState::Error(format!("profile: {e}"));
            return;
        }
    };

    let source = camino::Utf8PathBuf::from(&resolved.sync.profile.source);
    let destination =
        match crate::scan::resolve_destination(&resolved.sync.profile.destination) {
            Ok(d) => d,
            Err(e) => {
                app.diff_state = DiffState::Error(format!("destination: {e}"));
                return;
            }
        };

    let dap_id = resolved.dap.dap.id.clone();
    let mode = resolved.sync.profile.mode;
    let pname = resolved.sync.profile.name.clone();

    match crate::diff::diff(&resolved, &source, &destination) {
        Ok(result) => {
            app.diff_state = DiffState::Ready {
                result: Box::new(result),
                source,
                destination,
                profile_name: pname,
                dap_id,
                mode,
            };
        }
        Err(e) => {
            app.diff_state = DiffState::Error(format!("diff: {e}"));
        }
    }
}
