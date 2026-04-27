//! Step-by-step wizard for creating a new sync profile from the TUI.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::tui::app::{App, FileBrowserState, WizardStep};
use crate::tui::theme::Theme;

const TOTAL_STEPS: usize = 4;

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
            Line::from(format!(" {step_label} "))
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

    // ── Step content ──────────────────────────────────────────────────────
    match wiz.step {
        WizardStep::Name => render_name(f, app, content_area, footer_area),
        WizardStep::Source => render_browser_step(
            f, app, content_area, footer_area,
            "Source — your music library",
            &wiz.source_browser,
        ),
        WizardStep::Destination => render_destination(f, app, content_area, footer_area),
        WizardStep::Mode => render_mode(f, app, content_area, footer_area),
        WizardStep::Confirm => render_confirm(f, app, content_area, footer_area),
    }
}

// ── Step renderers ────────────────────────────────────────────────────────────

fn render_name(f: &mut Frame, app: &App, content: Rect, footer: Rect) {
    let theme = &app.theme;
    let wiz = app.wizard.as_ref().unwrap();

    let [_, heading_area, _, input_area, _, hint_area, _] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(content);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Profile name",
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        heading_area,
    );

    let input_width = input_area.width.saturating_sub(2) as usize;
    let scroll = wiz.name.visual_scroll(input_width);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            wiz.name.value(),
            Style::default().fg(theme.fg).bg(theme.bg),
        )))
        .scroll((0, scroll as u16))
        .block(
            Block::bordered()
                .border_style(Style::default().fg(theme.warn))
                .style(Style::default().bg(theme.bg)),
        ),
        input_area,
    );

    let visual = wiz.name.visual_cursor().min(input_width.saturating_sub(1));
    f.set_cursor_position((input_area.x + 1 + visual as u16, input_area.y + 1));

    let hint_text = if let Some(ref src) = wiz.cloned_from {
        format!("  cloning from '{src}' — change the name then press enter")
    } else {
        "  A short identifier (e.g. hiby-r4-flac). Used as the filename.".to_owned()
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint_text,
            Style::default().fg(theme.muted),
        ))),
        hint_area,
    );

    render_text_footer(f, app, footer);
}

fn render_browser_step(
    f: &mut Frame,
    app: &App,
    content: Rect,
    footer: Rect,
    heading: &str,
    browser: &FileBrowserState,
) {
    let theme = &app.theme;

    let [_, heading_area, path_area, _, list_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(content);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  {heading}"),
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        heading_area,
    );

    let (path_str, path_style) = if browser.at_drives_root {
        ("  [ select a drive ]", Style::default().fg(theme.muted))
    } else {
        (browser.current.as_str(), Style::default().fg(theme.warn).add_modifier(Modifier::BOLD))
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            if browser.at_drives_root { path_str.to_owned() } else { format!("  {path_str}") },
            path_style,
        ))),
        path_area,
    );

    render_browser_list(f, theme, browser, list_area);
    render_browser_footer(f, app, footer, !browser.at_drives_root);
}

fn render_destination(f: &mut Frame, app: &App, content: Rect, footer: Rect) {
    let theme = &app.theme;
    let wiz = app.wizard.as_ref().unwrap();
    let manual_idx = app.scan.identified.len();

    // If "Browse…" is selected, show the file browser.
    if wiz.dest_choice == manual_idx {
        if let Some(ref browser) = wiz.dest_browser {
            render_browser_step(f, app, content, footer,
                "Destination — browse to the folder on your DAP", browser);
            return;
        }
    }

    // Otherwise show the DAP list.
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
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        heading_area,
    );

    let mut items: Vec<ListItem> = app.scan.identified.iter().map(|id| {
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("  {:<22}", id.dap_id),
                Style::default().fg(theme.fg),
            ),
            Span::styled(
                format!("auto:{:<16}  {}", id.dap_id, id.mount.mount_point),
                Style::default().fg(theme.muted),
            ),
        ]))
    }).collect();
    items.push(ListItem::new(Line::from(Span::styled(
        "  Browse filesystem…",
        Style::default().fg(theme.muted).add_modifier(Modifier::ITALIC),
    ))));

    let list = List::new(items)
        .style(Style::default().bg(theme.bg))
        .highlight_style(
            Style::default()
                .fg(theme.sel_fg)
                .bg(theme.sel_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    let mut state = ListState::default().with_selected(Some(wiz.dest_choice));
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
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        ))),
        heading_area,
    );

    let modes = [
        ("additive", "copy new + modified, never delete from destination"),
        ("mirror",   "copy new + modified, DELETE orphans from destination"),
    ];

    let items: Vec<ListItem> = modes.iter().map(|(name, desc)| {
        ListItem::new(Line::from(vec![
            Span::styled(
                format!("  {name:<12}"),
                Style::default().fg(theme.fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {desc}"), Style::default().fg(theme.muted)),
        ]))
    }).collect();

    let list = List::new(items)
        .style(Style::default().bg(theme.bg))
        .highlight_style(
            Style::default()
                .fg(theme.sel_fg)
                .bg(theme.sel_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    let mut state = ListState::default().with_selected(Some(wiz.mode_choice));
    f.render_stateful_widget(list, list_area, &mut state);
    render_list_footer(f, app, footer);
}

fn render_confirm(f: &mut Frame, app: &App, content: Rect, footer: Rect) {
    let theme = &app.theme;
    let wiz = app.wizard.as_ref().unwrap();
    let source = wiz.source();
    let dest = wiz.destination(&app.scan);
    let dir = crate::config::profiles_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~/.config/dapctl/profiles/".to_owned());
    let filename = sanitize_name(wiz.name.value());

    let lines = vec![
        Line::raw(""),
        Line::from(Span::styled(
            "  Profile ready to write",
            Style::default()
                .fg(theme.fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::raw(""),
        summary_row("  name        ", wiz.name.value().trim(), theme),
        summary_row("  source      ", &source, theme),
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

    f.render_widget(
        Paragraph::new(Line::from(vec![
            kb("enter"), Span::raw(" confirm  "),
            kb("esc"),   Span::raw(" back  "),
            kb("q"),     Span::raw(" cancel"),
        ]))
        .style(Style::default().fg(theme.muted)),
        footer,
    );
}

// ── File browser list ─────────────────────────────────────────────────────────

fn render_browser_list(
    f: &mut Frame,
    theme: &Theme,
    browser: &FileBrowserState,
    area: Rect,
) {
    let visible = area.height as usize;
    let scroll = browser.cursor.saturating_sub(visible.saturating_sub(1));

    let items: Vec<ListItem> = if browser.at_drives_root {
        // Drives list — no "select" header, just the drive letters.
        browser.entries.iter().map(|drive| {
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(drive.as_str(), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
            ]))
        }).collect()
    } else {
        // Normal directory listing.
        let select = ListItem::new(Line::from(Span::styled(
            "  [ ✓ select this directory ]",
            Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
        )));
        let mut v = vec![select];
        v.extend(browser.entries.iter().map(|name| {
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(name.as_str(), Style::default().fg(theme.fg)),
                Span::styled("/", Style::default().fg(theme.muted)),
            ]))
        }));
        v
    };

    if items.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  (empty directory)",
                Style::default().fg(theme.muted),
            ))),
            area,
        );
        return;
    }

    let list = List::new(items)
        .style(Style::default().bg(theme.bg))
        .highlight_style(
            Style::default()
                .fg(theme.sel_fg)
                .bg(theme.sel_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    let mut state = ListState::default()
        .with_selected(Some(browser.cursor))
        .with_offset(scroll);
    f.render_stateful_widget(list, area, &mut state);
}

// ── Footer helpers ────────────────────────────────────────────────────────────

fn render_text_footer(f: &mut Frame, app: &App, area: Rect) {
    f.render_widget(
        Paragraph::new(Line::from(vec![
            kb("enter"), Span::raw(" next  "),
            kb("esc"),   Span::raw(" back  "),
            kb("ctrl+c"), Span::raw(" quit"),
        ]))
        .style(Style::default().fg(app.theme.muted)),
        area,
    );
}

fn render_browser_footer(f: &mut Frame, app: &App, area: Rect, can_go_up: bool) {
    let mut spans = vec![
        kb("j/k"),       Span::raw(" navigate  "),
        kb("enter/l/→"), Span::raw(" open/select  "),
    ];
    if can_go_up {
        spans.push(kb("h/←"));
        spans.push(Span::raw(" parent  "));
    }
    spans.push(kb("esc"));
    spans.push(Span::raw(" back"));

    f.render_widget(
        Paragraph::new(Line::from(spans))
            .style(Style::default().fg(app.theme.muted)),
        area,
    );
}

fn render_list_footer(f: &mut Frame, app: &App, area: Rect) {
    f.render_widget(
        Paragraph::new(Line::from(vec![
            kb("j/k"),   Span::raw(" move  "),
            kb("enter"), Span::raw(" select  "),
            kb("esc"),   Span::raw(" back"),
        ]))
        .style(Style::default().fg(app.theme.muted)),
        area,
    );
}

// ── Small helpers ─────────────────────────────────────────────────────────────

fn summary_row<'a>(label: &'a str, value: &'a str, theme: &'a Theme) -> Line<'a> {
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

pub fn sanitize_name(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase()
}
