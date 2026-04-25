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

    // Always restore terminal before anything else.
    let _ = disable_raw_mode();
    let _ = terminal.backend_mut().execute(LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    result?;

    // Run pending sync after TUI exits cleanly.
    if app.pending_sync {
        run_sync_from_tui(&app)?;
    }

    Ok(())
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        // Run any pending diff computation immediately after its loading frame.
        if matches!(app.diff_state, DiffState::Loading) {
            compute_diff(app);
            if app.should_quit {
                break;
            }
            continue;
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
            _ => {
                app.view = View::Profiles;
                app.confirm_sync = false;
            }
        },

        // esc: always back to profiles
        (KeyCode::Esc, _) => {
            app.view = View::Profiles;
            app.confirm_sync = false;
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
        (KeyCode::Char('r'), _) if app.view == View::Diff => {
            app.enter_diff();
        }

        // y: confirm sync — requires second press if mirror+orphans
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
        // First press: warn and wait for a second y
        app.confirm_sync = true;
        app.set_flash(format!(
            "Mirror mode will DELETE {orphans} orphan(s).  Press y again to confirm."
        ));
        return;
    }

    // Confirmed — exit TUI and run sync
    app.confirm_sync = false;
    app.pending_sync = true;
    app.should_quit = true;
}

// ── Diff computation ───────────────────────────────────────────────────────

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

// ── Sync from TUI (runs after TUI exits) ──────────────────────────────────

fn run_sync_from_tui(app: &App) -> anyhow::Result<()> {
    use crate::config::Mode;
    use crate::diff::EntryKind;
    use crate::scan::fmt_bytes;
    use crate::transfer::executor::{Options, SyncMode};

    let Some((_, profile)) = app.profiles.get(app.profile_idx) else {
        return Ok(());
    };
    let profile = profile.clone();

    let dap = crate::dap::load(&profile.profile.dap_profile)?;
    let resolved = crate::config::ResolvedProfile { sync: profile.clone(), dap };
    let source = camino::Utf8PathBuf::from(&profile.profile.source);
    let destination = crate::scan::resolve_destination(&profile.profile.destination)?;

    // Header
    println!();
    println!("SYNC  {}  →  {}", profile.profile.source, destination);
    println!(
        "      profile: {}  mode: {:?}  dap: {}",
        profile.profile.name,
        profile.profile.mode,
        resolved.dap.dap.id,
    );
    println!("{}", "─".repeat(62));

    // Repair mtimes for destinations populated before mtime-preservation fix.
    let repaired = crate::transfer::repair_dest_mtimes(&source, &destination);
    if repaired > 0 {
        eprintln!("  repaired mtimes for {repaired} file(s)");
    }

    // Re-diff (files may have changed since TUI diff was computed).
    let result = crate::diff::diff(&resolved, &source, &destination)?;
    let plan = &result.plan;

    let new_b = plan.total_bytes(EntryKind::New);
    let mod_b = plan.total_bytes(EntryKind::Modified);
    let orp_b = plan.total_bytes(EntryKind::Orphan);
    let transfer_total = new_b + mod_b;

    println!(
        "  [+] {:>6}  new        {}",
        plan.count(EntryKind::New),
        fmt_bytes(new_b)
    );
    println!(
        "  [~] {:>6}  modified   {}",
        plan.count(EntryKind::Modified),
        fmt_bytes(mod_b)
    );
    println!(
        "  [-] {:>6}  orphans    {}",
        plan.count(EntryKind::Orphan),
        fmt_bytes(orp_b)
    );
    println!(
        "  [=] {:>6}  unchanged  {}",
        plan.count(EntryKind::Same),
        fmt_bytes(plan.total_bytes(EntryKind::Same))
    );
    println!("{}", "─".repeat(62));
    println!("  transfer: {}", fmt_bytes(transfer_total));
    println!();

    if transfer_total == 0 && plan.count(EntryKind::Orphan) == 0 {
        println!("Nothing to sync.");
        return Ok(());
    }

    let mode = match profile.profile.mode {
        Mode::Mirror => SyncMode::Mirror,
        Mode::Additive | Mode::Selective => SyncMode::Additive,
    };

    let manifest_dir = {
        let dirs = directories::ProjectDirs::from("", "", "dapctl")
            .ok_or_else(|| anyhow::anyhow!("cannot determine data directory"))?;
        let path = dirs.data_local_dir().join("runs");
        camino::Utf8PathBuf::from_path_buf(path)
            .map_err(|p| anyhow::anyhow!("non-UTF-8 data dir: {}", p.display()))?
    };

    let opts = Options {
        dry_run: false,
        mode,
        verify: resolved.sync.transfer.verify,
        run_id: crate::logging::current_run_id(),
        manifest_dir,
    };

    let stats = crate::transfer::execute(plan, &source, &destination, &opts)?;

    println!();
    println!(
        "Sync complete: {} copied, {} deleted, {} failed  ({:.0}s)",
        stats.copied, stats.deleted, stats.failed, stats.elapsed_secs,
    );

    tracing::info!(
        event = "sync_done",
        copied = stats.copied,
        deleted = stats.deleted,
        failed = stats.failed,
        bytes = stats.bytes_written,
    );

    if stats.failed > 0 {
        anyhow::bail!("{} file(s) failed to transfer", stats.failed);
    }

    Ok(())
}
