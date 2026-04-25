//! Live progress: total bar, current file, throughput, ETA, event log tail.

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Gauge, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::scan::fmt_bytes;
use crate::tui::app::{App, ProgressState};

pub fn render(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = f.area();

    let Some(ref ps) = app.progress_state else {
        f.render_widget(
            Paragraph::new(" no sync in progress")
                .style(Style::default().fg(theme.muted).bg(theme.bg)),
            area,
        );
        return;
    };

    let title = format!(" dapctl — sync  {} ", ps.profile_name);
    let outer = Block::bordered()
        .title(Line::from(title).style(
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(theme.muted))
        .style(Style::default().bg(theme.bg));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    // ── Layout ───────────────────────────────────────────────────────────────
    // overall bar (3), spacer (1), file bar (3), spacer (1),
    // stats (1), spacer (1), log header (1), log list (fill), footer (1)
    let [overall_area, _, file_area, _, stats_area, _, log_hdr_area, log_area, footer_area] =
        Layout::vertical([
            Constraint::Length(3), // overall gauge
            Constraint::Length(1), // spacer
            Constraint::Length(3), // file gauge
            Constraint::Length(1), // spacer
            Constraint::Length(1), // stats line
            Constraint::Length(1), // spacer
            Constraint::Length(1), // "Recent events" header
            Constraint::Fill(1),   // event tail
            Constraint::Length(1), // footer
        ])
        .areas(inner);

    render_overall(f, app, ps, overall_area);
    render_file_bar(f, app, ps, file_area);
    render_stats(f, app, ps, stats_area);

    // ── Log header ───────────────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Recent events",
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        log_hdr_area,
    );

    render_recent(f, app, ps, log_area);
    render_footer(f, app, ps, footer_area);
}

// ── Sub-renderers ─────────────────────────────────────────────────────────────

fn render_overall(f: &mut Frame, app: &App, ps: &ProgressState, area: ratatui::layout::Rect) {
    let theme = &app.theme;
    let ratio = if ps.total_bytes > 0 {
        (ps.done_bytes as f64 / ps.total_bytes as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let label = format!(
        " {}%  {}/{}",
        (ratio * 100.0) as u8,
        fmt_bytes(ps.done_bytes),
        fmt_bytes(ps.total_bytes),
    );
    let gauge = Gauge::default()
        .block(
            Block::bordered()
                .title(" Overall ")
                .border_style(Style::default().fg(theme.muted)),
        )
        .gauge_style(Style::default().fg(theme.fg).bg(theme.muted))
        .ratio(ratio)
        .label(label);
    f.render_widget(gauge, area);
}

fn render_file_bar(f: &mut Frame, app: &App, ps: &ProgressState, area: ratatui::layout::Rect) {
    let theme = &app.theme;

    if ps.current_file.is_empty() && !ps.finished {
        f.render_widget(
            Block::bordered()
                .title(" Current file ")
                .border_style(Style::default().fg(theme.muted)),
            area,
        );
        return;
    }

    let ratio = if ps.current_file_bytes > 0 {
        (ps.current_file_done as f64 / ps.current_file_bytes as f64).clamp(0.0, 1.0)
    } else {
        1.0
    };

    let file_short = truncate_path(&ps.current_file, area.width.saturating_sub(14) as usize);
    let gauge = Gauge::default()
        .block(
            Block::bordered()
                .title(format!(" {} ", file_short))
                .border_style(Style::default().fg(theme.muted)),
        )
        .gauge_style(Style::default().fg(theme.warn).bg(theme.muted))
        .ratio(ratio)
        .label(format!(
            " {}/{}",
            fmt_bytes(ps.current_file_done),
            fmt_bytes(ps.current_file_bytes)
        ));
    f.render_widget(gauge, area);
}

fn render_stats(f: &mut Frame, app: &App, ps: &ProgressState, area: ratatui::layout::Rect) {
    let theme = &app.theme;

    if ps.finished {
        if let Some(ref s) = ps.finish_stats {
            let line = Line::from(vec![
                Span::styled("  DONE  ", Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
                Span::raw(format!(
                    "copied {}  deleted {}  failed {}  elapsed {}",
                    s.copied,
                    s.deleted,
                    s.failed,
                    fmt_eta(s.elapsed_secs as u64),
                )),
            ]);
            f.render_widget(
                Paragraph::new(line).style(Style::default().fg(theme.fg)),
                area,
            );
            return;
        }
    }

    let speed = ps.speed_bps();
    let eta = ps.eta_secs();
    let speed_str = if speed < 1.0 {
        "—".to_owned()
    } else {
        format!("{}/s", fmt_bytes(speed as u64))
    };
    let eta_str = if ps.total_bytes == 0 || speed < 1.0 {
        "—".to_owned()
    } else {
        fmt_eta(eta)
    };

    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled("speed ", Style::default().fg(theme.muted)),
        Span::styled(format!("{speed_str:<12}"), Style::default().fg(theme.fg)),
        Span::styled("ETA ", Style::default().fg(theme.muted)),
        Span::styled(format!("{eta_str:<10}"), Style::default().fg(theme.fg)),
        Span::styled("copied ", Style::default().fg(theme.muted)),
        Span::styled(format!("{}", ps.copied), Style::default().fg(theme.fg)),
        Span::raw("  "),
        Span::styled("deleted ", Style::default().fg(theme.muted)),
        Span::styled(format!("{}", ps.deleted), Style::default().fg(theme.fg)),
        Span::raw("  "),
        Span::styled("failed ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{}", ps.failed),
            Style::default().fg(if ps.failed > 0 { app.theme.err } else { theme.fg }),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_recent(f: &mut Frame, app: &App, ps: &ProgressState, area: ratatui::layout::Rect) {
    let theme = &app.theme;
    let max_w = area.width.saturating_sub(8) as usize;

    let items: Vec<ListItem> = ps
        .recent
        .iter()
        .map(|line| {
            let color = if line.ok { theme.fg } else { theme.err };
            let path_short = truncate_path(&line.path, max_w);
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{} ", line.icon),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(path_short.to_owned(), Style::default().fg(color)),
            ]))
        })
        .collect();

    // Auto-scroll to bottom: offset so the last items are visible.
    let n = items.len() as u16;
    let visible = area.height;
    let offset = n.saturating_sub(visible) as usize;

    let list = List::new(items).style(Style::default().bg(theme.bg));

    use ratatui::widgets::ListState;
    let mut state = ListState::default().with_offset(offset);
    f.render_stateful_widget(list, area, &mut state);
}

fn render_footer(f: &mut Frame, app: &App, ps: &ProgressState, area: ratatui::layout::Rect) {
    let theme = &app.theme;

    let line = if let Some(ref msg) = app.flash {
        Line::from(Span::styled(
            format!("  {msg}"),
            Style::default().fg(theme.warn),
        ))
    } else if ps.finished {
        Line::from(vec![
            kb("q"),
            Span::raw(" quit"),
            Span::styled("  (sync complete)", Style::default().fg(theme.muted)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                "  syncing…  ",
                Style::default().fg(theme.muted),
            ),
            kb("q"),
            Span::raw(" available when done"),
        ])
    };
    f.render_widget(
        Paragraph::new(line).style(Style::default().fg(theme.muted)),
        area,
    );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn kb(key: &str) -> Span<'static> {
    Span::styled(
        key.to_owned(),
        Style::default().add_modifier(Modifier::BOLD),
    )
}

fn truncate_path(s: &str, max: usize) -> &str {
    if max == 0 || s.len() <= max {
        return s;
    }
    // Try to keep the rightmost part (filename).
    let start = s.len().saturating_sub(max);
    &s[start..]
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
