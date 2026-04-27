//! Log tail view: scrollable display of the most recent JSONL run log.

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::tui::app::{App, LogLevel};

pub fn render(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = f.area();

    let run_label = if app.log_run_id.is_empty() {
        " no runs ".to_owned()
    } else {
        format!(" run: {} ", &app.log_run_id)
    };

    let outer = Block::bordered()
        .title(
            Line::from(" dapctl log ")
                .style(Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        )
        .title(
            Line::from(run_label)
                .style(Style::default().fg(theme.muted))
                .alignment(ratatui::layout::Alignment::Right),
        )
        .border_style(Style::default().fg(theme.muted))
        .style(Style::default().bg(theme.bg));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let [list_area, footer_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);

    // ── Footer ────────────────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new(Line::from(vec![
            kb("j/k"),   Span::raw(" scroll  "),
            kb("g"),     Span::raw(" top  "),
            kb("G"),     Span::raw(" bottom  "),
            kb("r"),     Span::raw(" reload  "),
            kb("q"),     Span::raw(" back"),
        ]))
        .style(Style::default().fg(theme.muted)),
        footer_area,
    );

    // ── Log lines ─────────────────────────────────────────────────────────
    if app.log_lines.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  (no log entries)",
                Style::default().fg(theme.muted),
            )))
            .style(Style::default().bg(theme.bg)),
            list_area,
        );
        return;
    }

    let items: Vec<ListItem> = app.log_lines.iter().map(|entry| {
        let level_color = match entry.level {
            LogLevel::Info  => theme.fg,
            LogLevel::Warn  => theme.warn,
            LogLevel::Error => theme.err,
            LogLevel::Other => theme.muted,
        };

        let mut spans = vec![
            Span::styled(
                format!("  {}  ", entry.time),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                format!("{:<5}  ", entry.level_str()),
                Style::default().fg(level_color),
            ),
            Span::styled(
                format!("{:<16}", entry.event),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
        ];
        if !entry.detail.is_empty() {
            spans.push(Span::styled(
                format!("  {}", entry.detail),
                Style::default().fg(theme.muted),
            ));
        }

        ListItem::new(Line::from(spans))
    }).collect();

    let visible = list_area.height as usize;
    let scroll = app.log_scroll.saturating_sub(visible.saturating_sub(1) / 2);

    let list = List::new(items)
        .style(Style::default().bg(theme.bg))
        .highlight_style(
            Style::default()
                .fg(theme.sel_fg)
                .bg(theme.sel_bg),
        );

    let mut state = ListState::default()
        .with_selected(Some(app.log_scroll))
        .with_offset(scroll);
    f.render_stateful_widget(list, list_area, &mut state);
}

fn kb(key: &str) -> Span<'static> {
    Span::styled(
        key.to_owned(),
        Style::default().add_modifier(Modifier::BOLD),
    )
}
