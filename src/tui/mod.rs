//! Ratatui app. Every action the TUI exposes has a CLI equivalent.

pub mod app;
pub mod theme;
pub mod views;

use std::io::{self, stdout};
use std::sync::mpsc;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
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
        // Drain progress, player, and library scan events before drawing.
        app.drain_progress();
        app.drain_player();
        app.drain_scan();

        terminal.draw(|f| draw(f, app))?;

        // Trigger diff computation the frame after its "Computing…" screen.
        if matches!(app.diff_state, DiffState::Loading) {
            compute_diff(app);
            if app.should_quit {
                break;
            }
            continue;
        }

        if event::poll(Duration::from_millis(100))? {
            let ev = event::read()?;
            if let Event::Key(key) = ev {
                // Ignore OS key-repeat events; only act on the initial Press.
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if app.view == View::NewProfile {
                    handle_wizard_key(app, key);
                } else if app.view == View::Player {
                    handle_player_key(app, key);
                } else {
                    handle_key(app, key.code, key.modifiers);
                }
            }
        }

        app.tick_flash();

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn draw(f: &mut ratatui::Frame, app: &mut App) {
    match app.view {
        View::Home => views::home::render(f, app),
        View::Profiles => views::profiles::render(f, app),
        View::Diff => views::diff::render(f, app),
        View::Progress => views::progress::render(f, app),
        View::Log => views::log::render(f, app),
        View::NewProfile => views::new_profile::render(f, app),
        View::Player => {
            if let Some(ref mut ps) = app.player_state {
                views::player::draw(f, f.area(), ps, &app.theme);
            }
        }
    }
}

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    // Reset delete confirmation on any key that isn't the confirming second D press.
    if !(code == KeyCode::Char('D') && app.view == View::Profiles) {
        app.delete_confirm = false;
    }

    match (code, modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }

        (KeyCode::Char('q'), _) => match app.view {
            View::Home => app.should_quit = true,
            View::Profiles => app.view = View::Home,
            View::Progress => {
                let done = app.progress_state.as_ref().is_some_and(|p| p.finished);
                if done {
                    app.view = View::Home;
                }
            }
            _ => {
                app.view = View::Profiles;
                app.confirm_sync = false;
            }
        },
        (KeyCode::Char('l'), _) if app.view == View::Progress => {
            let done = app.progress_state.as_ref().is_some_and(|p| p.finished);
            if done {
                app.load_log();
            }
        }

        (KeyCode::Esc, _) if app.view != View::Progress => match app.view {
            View::Home | View::Profiles => {}
            _ => {
                app.view = View::Profiles;
                app.confirm_sync = false;
            }
        },

        // ── Home ─────────────────────────────────────────────────────────
        (KeyCode::Char('j') | KeyCode::Down, _) if app.view == View::Home => {
            app.home_move_down();
        }
        (KeyCode::Char('k') | KeyCode::Up, _) if app.view == View::Home => {
            app.home_move_up();
        }
        (KeyCode::Enter, _) if app.view == View::Home => {
            navigate_home(app);
        }
        (KeyCode::Char('s'), _) if app.view == View::Home => {
            app.view = View::Profiles;
        }
        (KeyCode::Char('m'), _) if app.view == View::Home => {
            app.enter_player_from_profile();
        }
        (KeyCode::Char('l'), _) if app.view == View::Home => {
            app.load_log();
        }
        (KeyCode::Char('r'), _) if app.view == View::Home => {
            app.refresh_scan();
        }

        // ── Profiles ─────────────────────────────────────────────────────
        (KeyCode::Char('j') | KeyCode::Down, _) if app.view == View::Profiles => {
            app.move_down();
        }
        (KeyCode::Char('k') | KeyCode::Up, _) if app.view == View::Profiles => {
            app.move_up();
        }
        (KeyCode::Enter, _) if app.view == View::Profiles && !app.profiles.is_empty() => {
            app.enter_diff();
        }
        (KeyCode::Char('r'), _) if app.view == View::Profiles => {
            app.refresh_scan();
        }
        (KeyCode::Char('n'), _) if app.view == View::Profiles => {
            app.enter_new_profile();
        }
        (KeyCode::Char('l'), _) if app.view == View::Profiles => {
            app.load_log();
        }
        (KeyCode::Char('c'), _) if app.view == View::Profiles => {
            if app.profiles.is_empty() {
                app.set_flash("no profiles to clone");
            } else {
                app.enter_clone_profile();
            }
        }
        (KeyCode::Char('m'), _) if app.view == View::Profiles => {
            app.enter_player_from_profile();
        }
        (KeyCode::Char('D'), _) if app.view == View::Profiles => {
            if app.profiles.is_empty() {
                app.set_flash("no profiles to delete");
            } else if app.delete_confirm {
                app.delete_confirm = false;
                let name = app
                    .profiles
                    .get(app.profile_idx)
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default();
                match app.delete_current_profile() {
                    Ok(()) => app.set_flash(format!("deleted '{name}'")),
                    Err(e) => app.set_flash(format!("delete failed: {e}")),
                }
            } else {
                let name = app
                    .profiles
                    .get(app.profile_idx)
                    .map(|(n, _)| n.as_str())
                    .unwrap_or("?");
                app.set_flash(format!("press D again to delete '{name}'"));
                app.delete_confirm = true;
            }
        }

        // ── Log ──────────────────────────────────────────────────────────
        (KeyCode::Char('j') | KeyCode::Down, _) if app.view == View::Log => {
            let max = app.log_lines.len().saturating_sub(1);
            app.log_scroll = (app.log_scroll + 1).min(max);
        }
        (KeyCode::Char('k') | KeyCode::Up, _) if app.view == View::Log => {
            app.log_scroll = app.log_scroll.saturating_sub(1);
        }
        (KeyCode::Char('g'), _) if app.view == View::Log => {
            app.log_scroll = 0;
        }
        (KeyCode::Char('G'), _) if app.view == View::Log => {
            app.log_scroll = app.log_lines.len().saturating_sub(1);
        }
        (KeyCode::Char('r'), _) if app.view == View::Log => {
            app.load_log();
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
        (KeyCode::Char('x'), _) if app.view == View::Diff => {
            toggle_selective(app);
        }
        (KeyCode::Char(' '), _) if app.view == View::Diff => {
            enqueue_diff_entry(app);
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

    let DiffState::Ready {
        result,
        source,
        destination,
        profile_name,
        mode,
        ..
    } = &app.diff_state
    else {
        return;
    };

    // Clone everything the thread needs.
    let mut plan = result.plan.clone();
    let source = source.clone();
    let destination = destination.clone();
    let profile_name = profile_name.clone();
    let mode = *mode;

    // Selective mode: filter plan + persist the selection to the profile TOML.
    if matches!(mode, Mode::Selective) {
        let sel = &app.selective_paths;
        plan.entries.retain(|e| {
            let parent = e.path.parent().map(|p| p.as_str()).unwrap_or("");
            sel.contains(parent)
        });
        // Write back selected paths, preserving comments via toml_edit.
        if let Some((file_stem, _)) = app.profiles.get(app.profile_idx) {
            if let Ok(dir) = crate::config::profiles_dir() {
                let toml_path = dir.join(format!("{file_stem}.toml"));
                let mut paths: Vec<String> = app.selective_paths.iter().cloned().collect();
                paths.sort();
                let _ = crate::config::save_selective_paths(&toml_path, &paths);
            }
        }
    }

    let Some((_, profile)) = app.profiles.get(app.profile_idx) else {
        return;
    };
    let verify = profile.transfer.verify;
    let total_bytes = plan.transfer_bytes();

    let sync_mode = match mode {
        Mode::Mirror => SyncMode::Mirror,
        Mode::Additive | Mode::Selective => SyncMode::Additive,
    };

    let manifest_dir = match manifest_dir() {
        Ok(d) => d,
        Err(e) => {
            app.set_flash(format!("manifest dir error: {e}"));
            return;
        }
    };

    let run_id = crate::logging::current_run_id();

    // Create SSH session before spawning the thread (auth errors surface immediately).
    let ssh_source = if crate::ssh::SshUri::is_ssh(source.as_str()) {
        match crate::ssh::SshUri::parse(source.as_str())
            .and_then(|uri| crate::ssh::SshSession::connect(&uri))
        {
            Ok(s) => Some(s),
            Err(e) => {
                app.set_flash(format!("SSH error: {e}"));
                return;
            }
        }
    } else {
        None
    };

    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let opts = Options {
            dry_run: false,
            mode: sync_mode,
            verify,
            run_id,
            manifest_dir,
            progress_tx: Some(tx),
            transcode: None,
            ssh_source,
        };
        let _ = crate::transfer::execute(&plan, &source, &destination, &opts);
    });

    app.progress_rx = Some(rx);
    app.progress_state = Some(ProgressState::new(profile_name, total_bytes));
    app.view = View::Progress;
}

// ── Selective mode toggle ─────────────────────────────────────────────────────

fn toggle_selective(app: &mut App) {
    use crate::config::Mode;

    let DiffState::Ready { result, mode, .. } = &app.diff_state else {
        return;
    };
    if !matches!(mode, Mode::Selective) {
        app.set_flash("x = toggle selection (only in selective mode profiles)");
        return;
    }

    let filter = app.diff_entry_filter;
    let filtered: Vec<_> = result
        .plan
        .entries
        .iter()
        .filter(|e| filter.matches(e.kind))
        .collect();

    let Some(entry) = filtered.get(app.diff_entry_idx) else {
        return;
    };

    // Album-level granularity: key = parent directory of the entry.
    let parent = entry
        .path
        .parent()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_default();
    if parent.is_empty() {
        app.set_flash("cannot select root-level files individually");
        return;
    }

    if app.selective_paths.contains(&parent) {
        app.selective_paths.remove(&parent);
    } else {
        app.selective_paths.insert(parent);
    }
}

// ── New profile wizard ────────────────────────────────────────────────────────

fn handle_wizard_key(app: &mut App, key: crossterm::event::KeyEvent) {
    use crossterm::event::KeyCode as K;
    use tui_input::backend::crossterm::EventHandler;

    let Some(ref wiz) = app.wizard else { return };

    if key.code == K::Char('c')
        && key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        app.should_quit = true;
        return;
    }

    if key.code == K::Esc {
        // In destination browse mode, Esc goes back to the DAP list.
        if wiz.step == WizardStep::Destination
            && wiz.dest_choice == app.scan.identified.len()
            && wiz.dest_browser.is_some()
        {
            // dest_browser is always Some; Esc here just resets dest_choice
            app.wizard.as_mut().unwrap().dest_choice = 0;
            return;
        }
        match wiz.step.prev() {
            Some(prev) => app.wizard.as_mut().unwrap().step = prev,
            None => {
                app.view = View::Profiles;
                app.wizard = None;
            }
        }
        return;
    }

    match wiz.step {
        // ── Name (text input) ─────────────────────────────────────────────
        WizardStep::Name => {
            if key.code == K::Enter {
                let name = app.wizard.as_ref().unwrap().name.value().trim().to_owned();
                if name.is_empty() {
                    app.wizard.as_mut().unwrap().error = Some("name cannot be empty".to_owned());
                } else {
                    let filename = views::new_profile::sanitize_name(&name);
                    let duplicate = crate::config::profiles_dir()
                        .map(|dir| dir.join(format!("{filename}.toml")).exists())
                        .unwrap_or(false);
                    if duplicate {
                        app.wizard.as_mut().unwrap().error = Some(format!(
                            "'{filename}.toml' already exists — choose a different name"
                        ));
                    } else {
                        app.wizard.as_mut().unwrap().error = None;
                        app.wizard.as_mut().unwrap().step = WizardStep::Source;
                    }
                }
            } else {
                app.wizard
                    .as_mut()
                    .unwrap()
                    .name
                    .handle_event(&crossterm::event::Event::Key(key));
            }
        }

        // ── Source (file browser) ─────────────────────────────────────────
        WizardStep::Source => {
            let wiz = app.wizard.as_mut().unwrap();
            match key.code {
                K::Char('j') | K::Down => wiz.source_browser.move_down(),
                K::Char('k') | K::Up => wiz.source_browser.move_up(),
                K::Char('h') | K::Left => wiz.source_browser.go_up(),
                K::Enter | K::Char('l') | K::Right if wiz.source_browser.enter_selected() => {
                    wiz.error = None;
                    wiz.step = WizardStep::Destination;
                }
                K::Enter | K::Char('l') | K::Right => {}
                _ => {}
            }
        }

        // ── Destination (DAP list + file browser for manual) ──────────────
        WizardStep::Destination => {
            let manual_idx = app.scan.identified.len();
            let in_browser = app.wizard.as_ref().unwrap().dest_choice == manual_idx;

            if in_browser {
                let wiz = app.wizard.as_mut().unwrap();
                let browser = wiz.dest_browser.as_mut().unwrap();
                match key.code {
                    K::Char('j') | K::Down => browser.move_down(),
                    K::Char('k') | K::Up => browser.move_up(),
                    K::Char('h') | K::Left => browser.go_up(),
                    K::Enter | K::Char('l') | K::Right if browser.enter_selected() => {
                        wiz.error = None;
                        wiz.step = WizardStep::Mode;
                    }
                    K::Enter | K::Char('l') | K::Right => {}
                    _ => {}
                }
            } else {
                let total = manual_idx + 1;
                let wiz = app.wizard.as_mut().unwrap();
                match key.code {
                    K::Char('j') | K::Down => {
                        wiz.dest_choice = (wiz.dest_choice + 1).min(total - 1)
                    }
                    K::Char('k') | K::Up => wiz.dest_choice = wiz.dest_choice.saturating_sub(1),
                    K::Enter if wiz.dest_choice < manual_idx => {
                        wiz.error = None;
                        wiz.step = WizardStep::Mode;
                    }
                    K::Enter => {} // dest_choice == manual_idx → browser active on next frame
                    _ => {}
                }
            }
        }

        // ── Mode selector ─────────────────────────────────────────────────
        WizardStep::Mode => {
            let wiz = app.wizard.as_mut().unwrap();
            match key.code {
                K::Char('j') | K::Down => wiz.mode_choice = (wiz.mode_choice + 1).min(2),
                K::Char('k') | K::Up => wiz.mode_choice = wiz.mode_choice.saturating_sub(1),
                K::Enter => wiz.step = WizardStep::Confirm,
                _ => {}
            }
        }

        // ── Confirm ───────────────────────────────────────────────────────
        WizardStep::Confirm => match key.code {
            K::Enter => match write_new_profile(app) {
                Ok(name) => {
                    app.view = View::Profiles;
                    if let Ok(discovered) = crate::config::discover() {
                        app.profiles.clear();
                        for (fname, path) in discovered {
                            match crate::config::load(&path) {
                                Ok(p) => app.profiles.push((fname, p)),
                                Err(e) => tracing::warn!(err = %e, "skipping profile"),
                            }
                        }
                        if let Some(idx) = app
                            .profiles
                            .iter()
                            .position(|(_, p)| p.profile.name == name)
                        {
                            app.profile_idx = idx;
                        }
                    }
                    app.wizard = None;
                    app.set_flash(format!("profile '{name}' created"));
                }
                Err(e) => app.wizard.as_mut().unwrap().error = Some(e.to_string()),
            },
            K::Char('q') => {
                app.view = View::Profiles;
                app.wizard = None;
            }
            _ => {}
        },
    }
}

fn write_new_profile(app: &App) -> anyhow::Result<String> {
    let wiz = app.wizard.as_ref().unwrap();
    let name = wiz.name.value().trim().to_owned();
    let source = wiz.source();
    let destination = wiz.destination(&app.scan);
    let dap = wiz.selected_dap().to_owned();
    let mode = wiz.selected_mode();

    if name.is_empty() {
        anyhow::bail!("name is empty");
    }
    if source.is_empty() {
        anyhow::bail!("source is empty");
    }
    if destination.is_empty() {
        anyhow::bail!("destination is empty");
    }

    let filename = views::new_profile::sanitize_name(&name);
    let dir = crate::config::profiles_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{filename}.toml"));
    if path.exists() {
        anyhow::bail!("'{filename}.toml' already exists");
    }

    // Escape backslashes for TOML basic strings (Windows paths).
    let source_toml = source.replace('\\', "\\\\");
    let destination_toml = destination.replace('\\', "\\\\");

    std::fs::write(
        &path,
        format!(
            "schema_version = 1\n\
         \n\
         [profile]\n\
         name        = \"{name}\"\n\
         source      = \"{source_toml}\"\n\
         destination = \"{destination_toml}\"\n\
         dap_profile = \"{dap}\"\n\
         mode        = \"{mode}\"\n\
         \n\
         [filters]\n\
         include_globs = []\n\
         exclude_globs = []\n\
         \n\
         [transfer]\n\
         verify          = \"size_mtime\"\n\
         dry_run_default = true\n\
         parallelism     = 4\n"
        ),
    )?;
    Ok(name)
}

// ── Home navigation ───────────────────────────────────────────────────────────

fn navigate_home(app: &mut App) {
    match app.home_cursor {
        0 => app.view = View::Profiles,
        1 => app.enter_player_from_profile(),
        2 => app.load_log(),
        _ => {}
    }
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
                "DAP profile '{}': {e}",
                profile.profile.dap_profile
            ));
            return;
        }
    };
    let resolved = crate::config::ResolvedProfile {
        sync: profile.clone(),
        dap,
    };

    let source = camino::Utf8PathBuf::from(&profile.profile.source);
    let destination = match crate::scan::resolve_destination(&profile.profile.destination) {
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
            // First time opening a selective profile with no saved selection:
            // default to all album directories selected.
            if app.selective_init_pending {
                app.selective_paths = result
                    .plan
                    .entries
                    .iter()
                    .filter_map(|e| e.path.parent())
                    .map(|p| p.as_str().to_owned())
                    .filter(|p| !p.is_empty())
                    .collect();
                app.selective_init_pending = false;
            }
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

// ── Diff → Player integration ─────────────────────────────────────────────────

fn enqueue_diff_entry(app: &mut App) {
    use crate::diff::EntryKind;

    let (source, entry_path, transcode_from) = match &app.diff_state {
        DiffState::Ready { result, source, .. } => {
            let filter = app.diff_entry_filter;
            let filtered: Vec<_> = result
                .plan
                .entries
                .iter()
                .filter(|e| filter.matches(e.kind))
                .collect();

            let Some(entry) = filtered.get(app.diff_entry_idx) else {
                return;
            };

            if entry.kind == EntryKind::Orphan {
                app.set_flash("orphans exist only on destination — use L/D in player to toggle");
                return;
            }

            (
                source.clone(),
                entry.path.clone(),
                entry.transcode_from.clone(),
            )
        }
        _ => return,
    };

    // Resolve source path: use original extension for transcoded entries.
    let src_path = if let Some(from_ext) = transcode_from {
        source.join(entry_path.with_extension(from_ext))
    } else {
        source.join(&entry_path)
    };

    if !src_path.exists() {
        app.set_flash(format!("not found on source: {src_path}"));
        return;
    }

    app.enter_player();

    let track = crate::player::queue::TrackInfo::from_path(src_path).with_tags();
    if let Some(ref handle) = app.player_handle {
        handle.send(crate::player::engine::PlayerCommand::Enqueue(track));
    }

    app.set_flash("track queued — opening player");
    app.view = View::Player;
}

// ── Player key handler ────────────────────────────────────────────────────────

fn handle_player_key(app: &mut App, key: crossterm::event::KeyEvent) {
    use crossterm::event::KeyCode as K;

    if key.code == K::Char('c')
        && key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        app.should_quit = true;
        return;
    }

    let go_back = if let Some(ref mut ps) = app.player_state {
        if let Some(ref handle) = app.player_handle {
            views::player::handle_key(ps, handle, key)
        } else {
            matches!(key.code, K::Char('q') | K::Esc)
        }
    } else {
        true
    };

    // Sync shuffle/repeat back into PlayerStatus from last commands.
    if let Some(ref mut ps) = app.player_state {
        match key.code {
            K::Char('r') => ps.status.repeat = ps.status.repeat.next(),
            K::Char('s') => ps.status.shuffle = !ps.status.shuffle,
            _ => {}
        }
    }

    if go_back {
        app.view = View::Profiles;
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
