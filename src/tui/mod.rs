//! Ratatui app. Every action the TUI exposes has a CLI equivalent.

pub mod app;
pub mod theme;
pub mod views;

use std::io::{self, stdout};
use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{App, DiffState, ProgressState, View, WizardStep};

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
        // Drain progress events before drawing so we always render fresh state.
        app.drain_progress();

        terminal.draw(|f| draw(f, app))?;

        // Trigger diff computation the frame after its "Computing…" screen.
        if matches!(app.diff_state, DiffState::Loading) {
            compute_diff(app);
            if app.should_quit { break; }
            continue;
        }

        if event::poll(Duration::from_millis(100))? {
            let ev = event::read()?;
            if let Event::Key(key) = ev {
                if app.view == View::NewProfile {
                    handle_wizard_key(app, key);
                } else {
                    handle_key(app, key.code, key.modifiers);
                }
            }
        }

        app.tick_flash();

        if app.should_quit { break; }
    }
    Ok(())
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    match app.view {
        View::Profiles => views::profiles::render(f, app),
        View::Diff => views::diff::render(f, app),
        View::Progress => views::progress::render(f, app),
        View::Log => views::placeholder::render(f, app),
        View::NewProfile => views::new_profile::render(f, app),
    }
}

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match (code, modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }

        (KeyCode::Char('q'), _) => match app.view {
            View::Profiles => app.should_quit = true,
            View::Progress => {
                // Only allow quit once the sync is done.
                let done = app.progress_state.as_ref().is_some_and(|p| p.finished);
                if done { app.should_quit = true; }
            }
            _ => {
                app.view = View::Profiles;
                app.confirm_sync = false;
            }
        },

        (KeyCode::Esc, _) if app.view != View::Progress => {
            app.view = View::Profiles;
            app.confirm_sync = false;
        }

        // ── Profiles ─────────────────────────────────────────────────────
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
        (KeyCode::Char('r'), _) if app.view == View::Profiles => {
            app.refresh_scan();
        }
        (KeyCode::Char('n'), _) if app.view == View::Profiles => {
            app.enter_new_profile();
        }

        // ── Diff ─────────────────────────────────────────────────────────
        (KeyCode::Char('j') | KeyCode::Down, _) if app.view == View::Diff => {
            app.move_diff_down();
        }
        (KeyCode::Char('k') | KeyCode::Up, _) if app.view == View::Diff => {
            app.move_diff_up();
        }
        (KeyCode::Tab, _) if app.view == View::Diff => {
            app.cycle_diff_filter();
        }
        (KeyCode::Char('r'), _) if app.view == View::Diff => {
            app.enter_diff();
        }
        (KeyCode::Char('y'), _) if app.view == View::Diff => {
            handle_sync_confirm(app);
        }

        _ => {}
    }
}

fn handle_sync_confirm(app: &mut App) {
    use crate::config::Mode;
    use crate::diff::EntryKind;

    let DiffState::Ready { result, mode, .. } = &app.diff_state else {
        app.set_flash("no diff ready — press enter on a profile first");
        return;
    };

    let is_mirror = matches!(mode, Mode::Mirror);
    let orphans = result.plan.count(EntryKind::Orphan);

    if is_mirror && orphans > 0 && !app.confirm_sync {
        app.confirm_sync = true;
        app.set_flash(format!(
            "Mirror mode will DELETE {orphans} orphan(s).  Press y again to confirm."
        ));
        return;
    }

    app.confirm_sync = false;
    launch_sync(app);
}

fn launch_sync(app: &mut App) {
    use crate::config::Mode;
    use crate::transfer::executor::{Options, SyncMode};

    let DiffState::Ready { result, source, destination, profile_name, mode, .. } =
        &app.diff_state
    else {
        return;
    };

    // Clone everything the thread needs.
    let plan = result.plan.clone();
    let source = source.clone();
    let destination = destination.clone();
    let profile_name = profile_name.clone();
    let mode = *mode;

    let Some((_, profile)) = app.profiles.get(app.profile_idx) else { return };
    let verify = profile.transfer.verify;
    let total_bytes = plan.transfer_bytes();

    let sync_mode = match mode {
        Mode::Mirror => SyncMode::Mirror,
        Mode::Additive | Mode::Selective => SyncMode::Additive,
    };

    let manifest_dir = match manifest_dir() {
        Ok(d) => d,
        Err(e) => { app.set_flash(format!("manifest dir error: {e}")); return; }
    };

    let run_id = crate::logging::current_run_id();

    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let opts = Options {
            dry_run: false,
            mode: sync_mode,
            verify,
            run_id,
            manifest_dir,
            progress_tx: Some(tx),
        };
        let _ = crate::transfer::execute(&plan, &source, &destination, &opts);
    });

    app.progress_rx = Some(rx);
    app.progress_state = Some(ProgressState::new(profile_name, total_bytes));
    app.view = View::Progress;
}

// ── New profile wizard ────────────────────────────────────────────────────────

fn handle_wizard_key(app: &mut App, key: crossterm::event::KeyEvent) {
    use crossterm::event::KeyCode as K;
    use tui_input::backend::crossterm::EventHandler;

    let Some(ref mut wiz) = app.wizard else { return };

    // Ctrl-C always quits.
    if key.code == K::Char('c') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }

    // Esc: go back one step or exit wizard.
    if key.code == K::Esc {
        if wiz.dest_manual_active {
            wiz.dest_manual_active = false;
            return;
        }
        match wiz.step.prev() {
            Some(prev) => wiz.step = prev,
            None => {
                app.view = View::Profiles;
                app.wizard = None;
            }
        }
        return;
    }

    match wiz.step {
        // ── Text input steps ──────────────────────────────────────────────
        WizardStep::Name => {
            if key.code == K::Enter {
                if wiz.name.value().trim().is_empty() {
                    app.wizard.as_mut().unwrap().error = Some("name cannot be empty".to_owned());
                    return;
                }
                app.wizard.as_mut().unwrap().error = None;
                app.wizard.as_mut().unwrap().step = WizardStep::Source;
            } else {
                app.wizard.as_mut().unwrap().name
                    .handle_event(&crossterm::event::Event::Key(key));
            }
        }
        WizardStep::Source => {
            if key.code == K::Enter {
                if wiz.source.value().trim().is_empty() {
                    app.wizard.as_mut().unwrap().error = Some("source cannot be empty".to_owned());
                    return;
                }
                app.wizard.as_mut().unwrap().error = None;
                app.wizard.as_mut().unwrap().step = WizardStep::Destination;
            } else {
                app.wizard.as_mut().unwrap().source
                    .handle_event(&crossterm::event::Event::Key(key));
            }
        }

        // ── Destination list / manual input ───────────────────────────────
        WizardStep::Destination => {
            let manual_idx = app.scan.identified.len();
            let wiz = app.wizard.as_mut().unwrap();

            if wiz.dest_manual_active {
                if key.code == K::Enter {
                    if wiz.dest_manual.value().trim().is_empty() {
                        wiz.error = Some("destination cannot be empty".to_owned());
                        return;
                    }
                    wiz.error = None;
                    wiz.dest_manual_active = false;
                    wiz.step = WizardStep::DapProfile;
                } else {
                    wiz.dest_manual.handle_event(&crossterm::event::Event::Key(key));
                }
                return;
            }

            let total_choices = manual_idx + 1;
            match key.code {
                K::Char('j') | K::Down => {
                    wiz.dest_choice = (wiz.dest_choice + 1).min(total_choices - 1);
                }
                K::Char('k') | K::Up => {
                    wiz.dest_choice = wiz.dest_choice.saturating_sub(1);
                }
                K::Enter => {
                    if wiz.dest_choice == manual_idx {
                        wiz.dest_manual_active = true;
                    } else {
                        wiz.error = None;
                        wiz.step = WizardStep::DapProfile;
                    }
                }
                _ => {}
            }
        }

        // ── DAP profile list ──────────────────────────────────────────────
        WizardStep::DapProfile => {
            let wiz = app.wizard.as_mut().unwrap();
            let n = wiz.dap_ids.len().max(1);
            match key.code {
                K::Char('j') | K::Down => wiz.dap_choice = (wiz.dap_choice + 1).min(n - 1),
                K::Char('k') | K::Up => wiz.dap_choice = wiz.dap_choice.saturating_sub(1),
                K::Enter => wiz.step = WizardStep::Mode,
                _ => {}
            }
        }

        // ── Mode selector ─────────────────────────────────────────────────
        WizardStep::Mode => {
            let wiz = app.wizard.as_mut().unwrap();
            match key.code {
                K::Char('j') | K::Down => wiz.mode_choice = (wiz.mode_choice + 1).min(1),
                K::Char('k') | K::Up => wiz.mode_choice = wiz.mode_choice.saturating_sub(1),
                K::Enter => wiz.step = WizardStep::Confirm,
                _ => {}
            }
        }

        // ── Confirm ───────────────────────────────────────────────────────
        WizardStep::Confirm => {
            if key.code == K::Enter {
                match write_new_profile(app) {
                    Ok(filename) => {
                        app.view = View::Profiles;
                        // Reload profiles so the new one appears.
                        if let Ok(discovered) = crate::config::discover() {
                            app.profiles.clear();
                            for (name, path) in discovered {
                                match crate::config::load(&path) {
                                    Ok(p) => app.profiles.push((name, p)),
                                    Err(e) => tracing::warn!(err = %e, "skipping profile"),
                                }
                            }
                            // Select the new profile.
                            if let Some(idx) = app.profiles.iter().position(|(_, p)| {
                                p.profile.name == filename
                            }) {
                                app.profile_idx = idx;
                            }
                        }
                        app.wizard = None;
                        app.set_flash(format!("profile '{filename}' created"));
                    }
                    Err(e) => {
                        app.wizard.as_mut().unwrap().error = Some(e.to_string());
                    }
                }
            } else if key.code == K::Char('q') {
                app.view = View::Profiles;
                app.wizard = None;
            }
        }
    }
}

/// Write the wizard state to a `.toml` file in the profiles directory.
/// Returns the internal profile name on success.
fn write_new_profile(app: &App) -> anyhow::Result<String> {
    let wiz = app.wizard.as_ref().unwrap();

    let name = wiz.name.value().trim().to_owned();
    let source = wiz.source.value().trim().to_owned();
    let destination = wiz.destination(&app.scan);
    let dap = wiz.selected_dap().to_owned();
    let mode = wiz.selected_mode();

    if name.is_empty() { anyhow::bail!("name is empty"); }
    if source.is_empty() { anyhow::bail!("source is empty"); }
    if destination.is_empty() { anyhow::bail!("destination is empty"); }

    let filename = sanitize_filename(&name);
    let dir = crate::config::profiles_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{filename}.toml"));

    if path.exists() {
        anyhow::bail!("profile file '{filename}.toml' already exists");
    }

    let content = format!(
        "schema_version = 1\n\
         \n\
         [profile]\n\
         name        = \"{name}\"\n\
         source      = \"{source}\"\n\
         destination = \"{destination}\"\n\
         dap_profile = \"{dap}\"\n\
         mode        = \"{mode}\"\n\
         \n\
         [filters]\n\
         include_globs = []\n\
         exclude_globs = []\n\
         \n\
         [transfer]\n\
         verify          = \"size+mtime\"\n\
         dry_run_default = true\n\
         parallelism     = 4\n"
    );

    std::fs::write(&path, content)?;
    Ok(name)
}

fn sanitize_filename(s: &str) -> String {
    let slug: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    slug.trim_matches('-').to_lowercase()
}

// ── Diff computation ──────────────────────────────────────────────────────────

fn compute_diff(app: &mut App) {
    let Some((_, profile)) = app.profiles.get(app.profile_idx) else {
        app.diff_state = DiffState::Error("no profile selected".to_owned());
        return;
    };
    let profile = profile.clone();

    let dap = match crate::dap::load(&profile.profile.dap_profile) {
        Ok(d) => d,
        Err(e) => {
            app.diff_state = DiffState::Error(format!(
                "DAP profile '{}': {e}", profile.profile.dap_profile
            ));
            return;
        }
    };
    let resolved = crate::config::ResolvedProfile { sync: profile.clone(), dap };

    let source = camino::Utf8PathBuf::from(&profile.profile.source);
    let destination =
        match crate::scan::resolve_destination(&profile.profile.destination) {
            Ok(d) => d,
            Err(e) => {
                app.diff_state = DiffState::Error(format!("destination: {e}"));
                return;
            }
        };

    let dap_id = resolved.dap.dap.id.clone();
    let mode = profile.profile.mode;
    let pname = profile.profile.name.clone();

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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn manifest_dir() -> anyhow::Result<camino::Utf8PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "dapctl")
        .ok_or_else(|| anyhow::anyhow!("cannot determine data directory"))?;
    let path = dirs.data_local_dir().join("runs");
    camino::Utf8PathBuf::from_path_buf(path)
        .map_err(|p| anyhow::anyhow!("non-UTF-8 data dir: {}", p.display()))
}
