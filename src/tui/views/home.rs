use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};
use ratatui::Frame;

use crate::tui::app::App;
use crate::tui::theme::Theme;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// Three menu entries — index matches app.home_cursor.
const MENU: &[(&str, &str)] = &[
    ("sync & profiles", "manage and run sync"),
    ("player",          "browse & play your library"),
    ("log",             "last sync run history"),
];

pub fn render(f: &mut Frame, app: &mut App) {
    let theme = &app.theme;
    let area = f.area();

    let outer = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "▸ dapctl",
                Style::default()
                    .fg(theme.fg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title(
            Line::from(Span::styled(
                format!(" v{VERSION} "),
                Style::default().fg(theme.muted),
            ))
            .alignment(Alignment::Right),
        )
        .border_style(Style::default().fg(theme.muted))
        .style(Style::default().bg(theme.bg));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let [
        _b1,
        tagline_area,
        _b2,
        sep1_area,
        _b3,
        menu_area,
        _b4,
        sep2_area,
        _b5,
        daps_area,
        _spacer,
        hints_area,
    ] = Layout::vertical([
        Constraint::Length(1),  // blank
        Constraint::Length(1),  // tagline
        Constraint::Length(1),  // blank
        Constraint::Length(1),  // separator
        Constraint::Length(1),  // blank
        Constraint::Length(MENU.len() as u16),
        Constraint::Length(1),  // blank
        Constraint::Length(1),  // separator
        Constraint::Length(1),  // blank
        Constraint::Length(1),  // DAP status
        Constraint::Fill(1),
        Constraint::Length(1),  // hints
    ])
    .areas(inner);

    // ── Tagline ───────────────────────────────────────────────────────────────
    f.render_widget(
        Paragraph::new(Span::styled(
            "  TUI/CLI sync for HiFi Digital Audio Players",
            Style::default().fg(theme.muted),
        )),
        tagline_area,
    );

    // ── Separators ────────────────────────────────────────────────────────────
    let sep_line = {
        let w = inner.width.saturating_sub(4) as usize;
        format!("  {}", "─".repeat(w))
    };
    let sep_style = Style::default().fg(theme.muted);
    for area in [sep1_area, sep2_area] {
        f.render_widget(
            Paragraph::new(Span::styled(sep_line.clone(), sep_style)),
            area,
        );
    }

    // ── Menu ──────────────────────────────────────────────────────────────────
    draw_menu(f, menu_area, app, theme);

    // ── Connected DAPs ────────────────────────────────────────────────────────
    draw_daps(f, daps_area, app, theme);

    // ── Hints ─────────────────────────────────────────────────────────────────
    let flash = app.flash.clone();
    if let Some(ref msg) = flash {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!("  {msg}"),
                Style::default().fg(theme.warn),
            )),
            hints_area,
        );
    } else {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                kb("j/k", theme),
                Span::raw(" navigate  ·  "),
                kb("Enter", theme),
                Span::raw(" select  ·  "),
                kb("r", theme),
                Span::raw(" rescan  ·  "),
                kb("q", theme),
                Span::raw(" quit"),
            ])),
            hints_area,
        );
    }
}

// ── Sub-renders ───────────────────────────────────────────────────────────────

fn draw_menu(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let cursor = app.home_cursor;
    let w = area.width as usize;
    // Layout: 2 leading spaces + 3 (marker+space) + 20 (label) + 2 + description
    let label_col = 2 + 3 + 20 + 2;
    let desc_w = w.saturating_sub(label_col);

    let lines: Vec<Line> = MENU
        .iter()
        .enumerate()
        .map(|(i, (label, static_desc))| {
            let sel = i == cursor;
            let desc = if i == 0 {
                let n = app.profiles.len();
                format!("{n} profile{}", if n == 1 { "" } else { "s" })
            } else {
                static_desc.to_string()
            };

            let marker = if sel { "▶  " } else { "   " };
            let label_s = format!("{:<20}", label);
            let desc_s  = if desc_w > 0 {
                format!("{:>desc_w$}", trunc(&desc, desc_w))
            } else {
                String::new()
            };

            if sel {
                Line::from(Span::styled(
                    format!("  {marker}{label_s}  {desc_s}"),
                    Style::default()
                        .fg(theme.sel_fg)
                        .bg(theme.sel_bg)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("  {marker}{label_s}  "),
                        Style::default().fg(theme.fg),
                    ),
                    Span::styled(desc_s, Style::default().fg(theme.muted)),
                ])
            }
        })
        .collect();

    f.render_widget(Paragraph::new(lines), area);
}

fn draw_daps(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let identified = &app.scan.identified;
    let line = if identified.is_empty() {
        Line::from(Span::styled(
            "  no DAPs detected  ·  press r to scan",
            Style::default().fg(theme.muted),
        ))
    } else {
        let mut spans: Vec<Span> = vec![Span::raw("  ")];
        for (i, id) in identified.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(
                    "  ·  ",
                    Style::default().fg(theme.muted),
                ));
            }
            spans.push(Span::styled(
                id.dap_id.clone(),
                Style::default().fg(theme.fg),
            ));
        }
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(line), area);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn kb<'a>(key: &'a str, theme: &Theme) -> Span<'a> {
    Span::styled(key, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD))
}

fn trunc(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}
