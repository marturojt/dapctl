//! Step-by-step wizard for creating a new sync profile from the TUI.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::tui::app::{App, WizardStep};

const TOTAL_STEPS: usize = 5;

pub fn render(f: &mut Frame, app: &App) {
    let theme = &app.theme;
    let area = f.area();

    let Some(ref wiz) = app.wizard else { return };

    let step_label = if wiz.step == WizardStep::Confirm {
        "confirm".to_owned()
    } else {
        format!("step {}/{TOTAL_STEPS}: {}", wiz.step.number(), wiz.step.label())
    };

    let outer = Block::bordered()
        .title(Line::from(" dapctl — new profile ").style(
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
        ))
        .title(
            ratatui::text::Line::from(format!(" {step_label} "))
                .style(Style::default().fg(theme.muted))
                .alignment(ratatui::layout::Alignment::Right),
        )
        .border_style(Style::default().fg(theme.muted))
        .style(Style::default().bg(theme.bg));

    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let [content_area, error_area, footer_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    // ── Error line ────────────────────────────────────────────────────────
    if let Some(ref err) = wiz.error {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  ! {err}"),
                Style::default().fg(theme.err),
            ))),
            error_area,
        );
    }

    // ── Content ───────────────────────────────────────────────────────────
    match wiz.step {
        WizardStep::Name => render_text_step(
            f, app, content_area, footer_area,
            "Profile name",
            "A short identifier for this profile (e.g. hiby-r4-flac)",
            &wiz.name,
            true,
        ),
        WizardStep::Source => render_text_step(
            f, app, content_area, footer_area,
            "Source path",
            "Absolute path to your music library (e.g. /home/user/Music)",
            &wiz.source,
            true,
        ),
        WizardStep::Destination => render_destination(f, app, content_area, footer_area),
        WizardStep::DapProfile => render_dap(f, app, content_area, footer_area),
        WizardStep::Mode => render_mode(f, app, content_area, footer_area),
        WizardStep::Confirm => render_confirm(f, app, content_area, footer_area),
    }
}

// ── Step renderers ────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_text_step(
    f: &mut Frame,
    app: &App,
    content: Rect,
    footer: Rect,
    heading: &str,
    hint: &str,
    input: &tui_input::Input,
    show_cursor: bool,
) {
    let theme = &app.theme;

    let [_, heading_area, _, input_area, _, hint_area, _] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3), // border + 1 line content + border
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(content);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {heading}"),
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        heading_area,
    );

    // inner width = total width minus left+right borders
    let input_width = input_area.width.saturating_sub(2) as usize;
    let scroll = input.visual_scroll(input_width);
    let input_para = Paragraph::new(Line::from(Span::styled(
        input.value(),
        Style::default().fg(theme.fg).bg(theme.bg),
    )))
    .scroll((0, scroll as u16))
    .block(
        Block::bordered()
            .border_style(Style::default().fg(theme.warn))
            .style(Style::default().bg(theme.bg)),
    );
    f.render_widget(input_para, input_area);

    if show_cursor {
        let visual = input.visual_cursor().min(input_width.saturating_sub(1));
        let cursor_x = input_area.x + 1 + visual as u16;
        let cursor_y = input_area.y + 1;
        f.set_cursor_position((cursor_x, cursor_y));
    }

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {hint}"),
            Style::default().fg(theme.muted),
        ))),
        hint_area,
    );

    render_text_footer(f, app, footer);
}

fn render_destination(f: &mut Frame, app: &App, content: Rect, footer: Rect) {
    let theme = &app.theme;
    let wiz = app.wizard.as_ref().unwrap();

    if wiz.dest_manual_active {
        render_text_step(
            f, app, content, footer,
            "Destination path",
            "Absolute path or drive letter (e.g. F:\\Music or /mnt/dap/Music)",
            &wiz.dest_manual,
            true,
        );
        return;
    }

    let [_, heading_area, _, list_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(content);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Destination",
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        heading_area,
    );

    let mut items: Vec<ListItem> = app.scan.identified.iter().map(|id| {
        ListItem::new(Line::from(vec![
            Span::styled(format!("  {:<20}", id.dap_id), Style::default().fg(theme.fg)),
            Span::styled(
                format!("  auto:{:<16}  {}", id.dap_id, id.mount.mount_point),
                Style::default().fg(theme.muted),
            ),
        ]))
    }).collect();
    items.push(ListItem::new(Line::from(Span::styled(
        "  Manual path…",
        Style::default().fg(theme.muted),
    ))));

    let list = List::new(items)
        .style(Style::default().bg(theme.bg))
        .highlight_style(
            Style::default().fg(theme.sel_fg).bg(theme.sel_bg).add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    let mut state = ListState::default().with_selected(Some(wiz.dest_choice));
    f.render_stateful_widget(list, list_area, &mut state);

    render_list_footer(f, app, footer);
}

fn render_dap(f: &mut Frame, app: &App, content: Rect, footer: Rect) {
    let theme = &app.theme;
    let wiz = app.wizard.as_ref().unwrap();

    let [_, heading_area, _, list_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(content);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  DAP profile",
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        heading_area,
    );

    let items: Vec<ListItem> = wiz.dap_ids.iter().map(|id| {
        ListItem::new(Line::from(Span::raw(format!("  {id}"))))
    }).collect();

    let list = List::new(items)
        .style(Style::default().fg(theme.fg).bg(theme.bg))
        .highlight_style(
            Style::default().fg(theme.sel_fg).bg(theme.sel_bg).add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    let mut state = ListState::default().with_selected(Some(wiz.dap_choice));
    f.render_stateful_widget(list, list_area, &mut state);

    render_list_footer(f, app, footer);
}

fn render_mode(f: &mut Frame, app: &App, content: Rect, footer: Rect) {
    let theme = &app.theme;
    let wiz = app.wizard.as_ref().unwrap();

    let [_, heading_area, _, list_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(content);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Sync mode",
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        heading_area,
    );

    let modes = [
        ("additive", "copy new + modified, never delete from destination"),
        ("mirror",   "copy new + modified, DELETE orphans from destination"),
    ];

    let items: Vec<ListItem> = modes.iter().map(|(name, desc)| {
        ListItem::new(Line::from(vec![
            Span::styled(format!("  {name:<12}"), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  {desc}"), Style::default().fg(theme.muted)),
        ]))
    }).collect();

    let list = List::new(items)
        .style(Style::default().bg(theme.bg))
        .highlight_style(
            Style::default().fg(theme.sel_fg).bg(theme.sel_bg).add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    let mut state = ListState::default().with_selected(Some(wiz.mode_choice));
    f.render_stateful_widget(list, list_area, &mut state);

    render_list_footer(f, app, footer);
}

fn render_confirm(f: &mut Frame, app: &App, content: Rect, footer: Rect) {
    let theme = &app.theme;
    let wiz = app.wizard.as_ref().unwrap();

    let dest = wiz.destination(&app.scan);
    let dir = crate::config::profiles_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.config/dapctl/profiles/".to_owned());
    let filename = new_profile_filename(wiz.name.value());

    let lines = vec![
        Line::raw(""),
        Line::from(Span::styled(
            "  Profile ready to write",
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::raw(""),
        summary_row("  name        ", wiz.name.value().trim(), theme),
        summary_row("  source      ", wiz.source.value().trim(), theme),
        summary_row("  destination ", &dest, theme),
        summary_row("  DAP profile ", wiz.selected_dap(), theme),
        summary_row("  mode        ", wiz.selected_mode(), theme),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  will write  ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{dir}/{filename}.toml"),
                Style::default().fg(theme.warn),
            ),
        ]),
    ];

    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.bg)),
        content,
    );

    // Footer
    let footer_line = Line::from(vec![
        kb("enter"),
        Span::raw(" confirm  "),
        kb("esc"),
        Span::raw(" back  "),
        kb("q"),
        Span::raw(" cancel"),
    ]);
    f.render_widget(
        Paragraph::new(footer_line).style(Style::default().fg(theme.muted)),
        footer,
    );
}

// ── Shared footer helpers ─────────────────────────────────────────────────────

fn render_text_footer(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let line = Line::from(vec![
        kb("enter"),
        Span::raw(" next  "),
        kb("esc"),
        Span::raw(" back  "),
        kb("ctrl+c"),
        Span::raw(" quit"),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().fg(theme.muted)),
        area,
    );
}

fn render_list_footer(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let line = Line::from(vec![
        kb("j/k"),
        Span::raw(" move  "),
        kb("enter"),
        Span::raw(" select  "),
        kb("esc"),
        Span::raw(" back"),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().fg(theme.muted)),
        area,
    );
}

fn summary_row<'a>(label: &'a str, value: &'a str, theme: &'a crate::tui::theme::Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(label, Style::default().fg(theme.muted)),
        Span::styled(value, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
    ])
}

fn kb(key: &str) -> Span<'static> {
    Span::styled(
        key.to_owned(),
        Style::default().add_modifier(Modifier::BOLD),
    )
}

// ── Filename helper ───────────────────────────────────────────────────────────

pub fn new_profile_filename(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    slug.trim_matches('-').to_lowercase()
}
