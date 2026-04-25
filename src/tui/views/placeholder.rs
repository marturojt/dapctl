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
    let outer = Block::bordered()
        .title(format!(" dapctl — {view_name} "))
        .border_style(Style::default().fg(theme.muted))
        .style(Style::default().bg(theme.bg));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(3),
        Constraint::Fill(1),
    ])
    .areas(inner);

    f.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(format!("  {view_name} view — coming soon")).alignment(Alignment::Center),
            Line::from("  press q to return".to_string()).alignment(Alignment::Center),
        ])
        .style(
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::ITALIC),
        ),
        center,
    );
}
