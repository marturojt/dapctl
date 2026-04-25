//! Placeholder for views not yet implemented.

use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

use crate::tui::app::App;

pub fn render(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = f.area();

    let view_name = format!("{:?}", app.view).to_lowercase();

    let profile_info = app
        .selected_profile()
        .map(|p| format!("  profile: {}  ({})", p.profile.name, format!("{:?}", p.profile.mode).to_lowercase()))
        .unwrap_or_default();

    let outer = Block::bordered()
        .title(
            Line::from(format!(" dapctl — {view_name} "))
                .style(Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        )
        .border_style(Style::default().fg(theme.muted))
        .style(Style::default().bg(theme.bg));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(5),
        Constraint::Fill(1),
    ])
    .areas(inner);

    f.render_widget(
        Paragraph::new(vec![
            Line::from(
                format!("  {view_name} view — coming in the next milestone")
            )
            .alignment(Alignment::Center),
            Line::from("").alignment(Alignment::Center),
            Line::from(profile_info)
                .style(Style::default().fg(theme.muted))
                .alignment(Alignment::Center),
            Line::from("").alignment(Alignment::Center),
            Line::from("  esc / q  →  back to profiles")
                .style(Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC))
                .alignment(Alignment::Center),
        ])
        .style(Style::default().fg(theme.fg)),
        center,
    );
}
