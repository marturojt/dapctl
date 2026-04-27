//! Profile selector: list sync profiles and connected DAPs, allow launching
//! a diff or a sync from here.

use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::scan::fmt_bytes;
use crate::tui::app::App;

pub fn render(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = f.area();

    // ── Outer chrome ────────────────────────────────────────────────────────
    let n_daps = app.scan.identified.len();
    let dap_summary = format!(
        " scan: {} DAP{} ",
        n_daps,
        if n_daps == 1 { "" } else { "s" }
    );

    let outer = Block::bordered()
        .title(
            Line::from(" dapctl ")
                .style(Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        )
        .title(
            Line::from(dap_summary)
                .style(Style::default().fg(theme.muted))
                .alignment(Alignment::Right),
        )
        .border_style(Style::default().fg(theme.muted))
        .style(Style::default().bg(theme.bg));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // ── Inner split: content + footer (1 line) ────────────────────────────
    let [content_area, footer_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);

    // ── Footer: flash message or key hints ───────────────────────────────
    let footer_line = if let Some(ref msg) = app.flash {
        Line::from(Span::styled(
            format!("  {msg}"),
            Style::default().fg(theme.warn),
        ))
    } else {
        Line::from(vec![
            kb("j/k", app),
            Span::raw(" move  "),
            kb("enter", app),
            Span::raw(" diff  "),
            kb("n", app),
            Span::raw(" new  "),
            kb("c", app),
            Span::raw(" clone  "),
            kb("r", app),
            Span::raw(" refresh  "),
            kb("q", app),
            Span::raw(" quit"),
        ])
    };
    f.render_widget(
        Paragraph::new(footer_line).style(Style::default().fg(theme.muted)),
        footer_area,
    );

    // ── Content: profiles section + DAPs section ─────────────────────────
    let profiles_height = ((app.profiles.len() as u16).saturating_add(2))
        .max(3)
        .min(content_area.height.saturating_sub(4));

    let [profiles_area, daps_area] = Layout::vertical([
        Constraint::Length(profiles_height),
        Constraint::Fill(1),
    ])
    .areas(content_area);

    render_profiles(f, app, profiles_area);
    render_daps(f, app, daps_area);
}

fn render_profiles(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let theme = &app.theme;

    let [header_area, list_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  SYNC PROFILES",
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        header_area,
    );

    if app.profiles.is_empty() {
        let dir = crate::config::profiles_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "~/.config/dapctl/profiles/".to_owned());
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  (no profiles found — add .toml files to {dir})"),
                Style::default().fg(theme.muted),
            ))),
            list_area,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .profiles
        .iter()
        .map(|(_, p)| {
            let mode = format!("{:?}", p.profile.mode).to_lowercase();
            let src = truncate(&p.profile.source, 28);
            let dst = truncate(&p.profile.destination, 28);
            let line = format!(
                "  {:<22}  {:<28} → {:<28}  [{}]",
                p.profile.name, src, dst, mode
            );
            ListItem::new(line)
        })
        .collect();

    let mut state = ListState::default();
    if !app.profiles.is_empty() {
        state.select(Some(app.profile_idx));
    }

    let list = List::new(items)
        .style(Style::default().fg(theme.fg).bg(theme.bg))
        .highlight_style(
            Style::default()
                .fg(theme.sel_fg)
                .bg(theme.sel_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    f.render_stateful_widget(list, list_area, &mut state);
}

fn render_daps(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let theme = &app.theme;

    if area.height == 0 {
        return;
    }

    let [header_area, body_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  CONNECTED DAPs",
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        header_area,
    );

    let mut lines: Vec<Line> = Vec::new();

    if app.scan.identified.is_empty() && app.scan.unidentified.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no removable drives detected — press r to refresh)",
            Style::default().fg(theme.muted),
        )));
    } else {
        for id in &app.scan.identified {
            let total = id.mount.total_bytes.map(fmt_bytes).unwrap_or_default();
            let free = id.mount.free_bytes.map(fmt_bytes).unwrap_or_default();
            let fs = id.mount.filesystem.as_deref().unwrap_or("?");
            let label = id.mount.label.as_deref().unwrap_or(&id.dap_id);
            lines.push(Line::from(vec![
                Span::raw(format!("  {:<20}  ", id.dap_id)),
                Span::styled(
                    format!("{:<28}", id.mount.mount_point),
                    Style::default().fg(theme.fg),
                ),
                Span::raw(format!("  {:<8}", fs)),
                Span::styled(
                    format!("  free {} / {}", free, total),
                    Style::default().fg(theme.muted),
                ),
                Span::styled(
                    format!("  ({})", label),
                    Style::default().fg(theme.muted),
                ),
            ]));
        }
        for m in &app.scan.unidentified {
            let total = m.total_bytes.map(fmt_bytes).unwrap_or_default();
            let label = m.label.as_deref().unwrap_or("(no label)");
            let fs = m.filesystem.as_deref().unwrap_or("?");
            lines.push(Line::from(Span::styled(
                format!(
                    "  {:<20}  {:<28}  {:<8}  {}",
                    label, m.mount_point, fs, total
                ),
                Style::default().fg(theme.muted),
            )));
        }
    }

    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.bg)),
        body_area,
    );
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

fn kb<'a>(key: &'a str, app: &'a App) -> Span<'a> {
    Span::styled(
        key,
        Style::default()
            .fg(app.theme.fg)
            .add_modifier(Modifier::BOLD),
    )
}

#[allow(dead_code)]
fn dap_separator(theme: &crate::tui::theme::Theme) -> Block<'static> {
    Block::new()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(theme.muted))
}
