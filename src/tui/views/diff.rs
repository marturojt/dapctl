//! Diff preview: summary + filterable entry list.

use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::diff::EntryKind;
use crate::scan::fmt_bytes;
use crate::tui::app::{App, DiffState};
use crate::tui::theme::Theme;

const ESTIMATED_SPEED_BPS: u64 = 30 * 1024 * 1024;

pub fn render(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = f.area();

    match &app.diff_state {
        DiffState::Idle => render_idle(f, app, area),
        DiffState::Loading => render_loading(f, app, area),
        DiffState::Error(msg) => render_error(f, app, area, msg),
        DiffState::Ready { result, source, destination, profile_name, dap_id, mode } => {
            render_ready(f, app, theme, area, result, source, destination, profile_name, dap_id, *mode);
        }
    }
}

// ── Loading / idle / error screens ─────────────────────────────────────────

fn render_loading(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let theme = &app.theme;
    let outer = chrome(app, " dapctl — diff ");
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Computing diff…",
            Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC),
        )))
        .alignment(Alignment::Center),
        center,
    );
}

fn render_idle(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let outer = chrome(app, " dapctl — diff ");
    let inner = outer.inner(area);
    f.render_widget(outer, area);
    let theme = &app.theme;

    let [_, center, _] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1), Constraint::Fill(1)])
            .areas(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  press enter on a profile to compute diff",
            Style::default().fg(theme.muted),
        )))
        .alignment(Alignment::Center),
        center,
    );
}

fn render_error(f: &mut Frame, app: &App, area: ratatui::layout::Rect, msg: &str) {
    let theme = &app.theme;
    let outer = chrome(app, " dapctl — diff ");
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let [_, center, _] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(3), Constraint::Fill(1)])
            .areas(inner);

    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled("  Error", Style::default().fg(theme.err).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(format!("  {msg}"), Style::default().fg(theme.err))),
            Line::from(Span::styled("  press esc to go back", Style::default().fg(theme.muted))),
        ]),
        center,
    );
}

// ── Main diff view ──────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_ready(
    f: &mut Frame,
    app: &App,
    theme: &Theme,
    area: ratatui::layout::Rect,
    result: &crate::diff::DiffResult,
    source: &camino::Utf8PathBuf,
    destination: &camino::Utf8PathBuf,
    profile_name: &str,
    dap_id: &str,
    mode: crate::config::Mode,
) {
    let plan = &result.plan;

    let title = format!(" dapctl — diff  {profile_name} ");
    let outer = chrome(app, &title);
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // ── Layout: summary | list_header | list | footer ───────────────────
    let [summary_area, list_header_area, list_area, footer_area] = Layout::vertical([
        Constraint::Length(10),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    render_summary(f, theme, summary_area, plan, source, destination, dap_id, mode);
    render_list_header(f, theme, list_header_area, app, plan);
    render_entry_list(f, app, theme, list_area, plan);
    render_footer(f, app, theme, footer_area);
}

#[allow(clippy::too_many_arguments)]
fn render_summary(
    f: &mut Frame,
    theme: &Theme,
    area: ratatui::layout::Rect,
    plan: &crate::diff::Plan,
    source: &camino::Utf8PathBuf,
    destination: &camino::Utf8PathBuf,
    dap_id: &str,
    mode: crate::config::Mode,
) {
    let src_short = truncate(source.as_str(), 45);
    let dst_short = truncate(destination.as_str(), 45);
    let mode_str = format!("{mode:?}").to_lowercase();

    let new_b = plan.total_bytes(EntryKind::New);
    let mod_b = plan.total_bytes(EntryKind::Modified);
    let orp_b = plan.total_bytes(EntryKind::Orphan);
    let same_b = plan.total_bytes(EntryKind::Same);
    let transfer = new_b + mod_b;
    let eta = plan.eta_secs(ESTIMATED_SPEED_BPS);

    let lines = vec![
        // source → dest
        Line::from(vec![
            Span::styled(format!("  {src_short}"), Style::default().fg(theme.fg)),
            Span::styled("  →  ", Style::default().fg(theme.muted)),
            Span::styled(dst_short.to_string(), Style::default().fg(theme.fg)),
            Span::styled(format!("  ({dap_id}, {mode_str})"), Style::default().fg(theme.muted)),
        ]),
        Line::from(""),
        // counts
        Line::from(vec![
            Span::styled("  [+]", Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::raw(format!("  {:>6}  new          {}", plan.count(EntryKind::New), fmt_bytes(new_b))),
        ]),
        Line::from(vec![
            Span::styled("  [~]", Style::default().fg(theme.warn).add_modifier(Modifier::BOLD)),
            Span::raw(format!("  {:>6}  modified     {}", plan.count(EntryKind::Modified), fmt_bytes(mod_b))),
        ]),
        Line::from(vec![
            Span::styled("  [-]", Style::default().fg(theme.err).add_modifier(Modifier::BOLD)),
            Span::raw(format!("  {:>6}  orphans      {}", plan.count(EntryKind::Orphan), fmt_bytes(orp_b))),
        ]),
        Line::from(vec![
            Span::styled("  [=]", Style::default().fg(theme.muted).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  {:>6}  unchanged    {}", plan.count(EntryKind::Same), fmt_bytes(same_b)), Style::default().fg(theme.muted)),
        ]),
        Line::from(""),
        // ETA
        Line::from(vec![
            Span::raw("  transfer: "),
            Span::styled(fmt_bytes(transfer), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::raw("   ETA: "),
            Span::styled(fmt_eta(eta), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
    ];

    f.render_widget(
        Paragraph::new(lines).style(Style::default().fg(theme.fg).bg(theme.bg)),
        area,
    );
}

fn render_list_header(
    f: &mut Frame,
    theme: &Theme,
    area: ratatui::layout::Rect,
    app: &App,
    plan: &crate::diff::Plan,
) {
    let filter = app.diff_entry_filter;
    let total = plan.entries.iter().filter(|e| filter.matches(e.kind)).count();

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            filter.label(),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ),
        Span::styled(
            format!("  ({total} entries)   tab to cycle filter"),
            Style::default().fg(theme.muted),
        ),
    ]);

    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme.bg)),
        area,
    );
}

fn render_entry_list(
    f: &mut Frame,
    app: &App,
    theme: &Theme,
    area: ratatui::layout::Rect,
    plan: &crate::diff::Plan,
) {
    let filter = app.diff_entry_filter;
    let path_width = (area.width as usize).saturating_sub(20).max(30);

    let filtered: Vec<&crate::diff::Entry> =
        plan.entries.iter().filter(|e| filter.matches(e.kind)).collect();

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|e| {
            let (tag, tag_style) = match e.kind {
                EntryKind::New => ("[+]", Style::default().fg(theme.fg)),
                EntryKind::Modified => ("[~]", Style::default().fg(theme.warn)),
                EntryKind::Orphan => ("[-]", Style::default().fg(theme.err)),
                EntryKind::Same => ("[=]", Style::default().fg(theme.muted)),
            };
            let path = truncate(e.path.as_str(), path_width);
            let size = fmt_bytes(e.size_bytes);
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(tag, tag_style.add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(
                    format!("{path:<path_width$}"),
                    if e.kind == EntryKind::Same {
                        Style::default().fg(theme.muted)
                    } else {
                        Style::default().fg(theme.fg)
                    },
                ),
                Span::styled(
                    format!("  {size:>10}"),
                    Style::default().fg(theme.muted),
                ),
            ]))
        })
        .collect();

    let selected = app.diff_entry_idx.min(filtered.len().saturating_sub(1));
    let mut state = ListState::default();
    if !filtered.is_empty() {
        state.select(Some(selected));
    }

    let list = List::new(items)
        .style(Style::default().bg(theme.bg))
        .highlight_style(
            Style::default()
                .fg(theme.sel_fg)
                .bg(theme.sel_bg)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, &mut state);
}

fn render_footer(f: &mut Frame, app: &App, theme: &Theme, area: ratatui::layout::Rect) {
    let footer_line = if let Some(ref msg) = app.flash {
        Line::from(Span::styled(
            format!("  {msg}"),
            Style::default().fg(theme.warn),
        ))
    } else {
        Line::from(vec![
            kb("j/k", theme),
            Span::raw(" scroll  "),
            kb("tab", theme),
            Span::raw(" filter  "),
            kb("r", theme),
            Span::raw(" re-diff  "),
            kb("y", theme),
            Span::raw(" sync  "),
            kb("esc", theme),
            Span::raw(" back"),
        ])
    };
    f.render_widget(
        Paragraph::new(footer_line).style(Style::default().fg(theme.muted)),
        area,
    );
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn chrome<'a>(app: &'a App, title: &'a str) -> Block<'a> {
    Block::bordered()
        .title(Line::from(title).style(
            Style::default()
                .fg(app.theme.fg)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(app.theme.muted))
        .style(Style::default().bg(app.theme.bg))
}

fn kb<'a>(key: &'a str, theme: &'a Theme) -> Span<'a> {
    Span::styled(
        key.to_string(),
        Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
    )
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

fn fmt_eta(secs: u64) -> String {
    if secs == 0 {
        return "< 1s".to_owned();
    }
    if secs < 60 {
        return format!("{secs}s");
    }
    let m = secs / 60;
    let s = secs % 60;
    if m < 60 {
        return format!("{m}m {s:02}s");
    }
    let h = m / 60;
    let m = m % 60;
    format!("{h}h {m:02}m")
}
