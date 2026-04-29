use std::sync::mpsc::Receiver;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph};

use crate::player::engine::{PlayerEvent, PlayerHandle, PlayerStatus};
use crate::tui::theme::Theme;

// ── Player view state ─────────────────────────────────────────────────────────

/// Which library root the player is browsing / playing from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSource {
    /// Source music library (pre-sync).
    Library,
    /// Mounted DAP destination.
    Destination,
}

impl PlayerSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Library => "L library",
            Self::Destination => "D destination",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Library => Self::Destination,
            Self::Destination => Self::Library,
        }
    }
}

pub struct PlayerState {
    pub status: PlayerStatus,
    pub source: PlayerSource,
    pub queue_list_state: ListState,
    /// Error or info flash shown below the progress bar.
    pub flash: Option<String>,
    /// Whether the audio device is available.
    pub available: bool,
}

impl PlayerState {
    pub fn new(available: bool) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            status: PlayerStatus::default(),
            source: PlayerSource::Library,
            queue_list_state: list_state,
            flash: None,
            available,
        }
    }

    /// Consume pending player events and update local status.
    pub fn drain_events(&mut self, rx: &Receiver<PlayerEvent>) {
        while let Ok(event) = rx.try_recv() {
            match event {
                PlayerEvent::TrackStarted(track) => {
                    self.status.current = Some(track);
                    self.status.position = Duration::ZERO;
                    self.status.paused = false;
                    self.flash = None;
                    self.sync_list_cursor();
                }
                PlayerEvent::Position(pos) => {
                    self.status.position = pos;
                }
                PlayerEvent::TrackEnded => {}
                PlayerEvent::QueueEmpty => {
                    self.status.current = None;
                    self.status.position = Duration::ZERO;
                }
                PlayerEvent::Stopped => {
                    self.status.current = None;
                    self.status.position = Duration::ZERO;
                    self.status.paused = false;
                }
                PlayerEvent::DecodeError { path, err } => {
                    self.flash = Some(format!("decode error: {path}: {err}"));
                }
            }
        }
    }

    fn sync_list_cursor(&mut self) {
        self.queue_list_state.select(Some(self.status.queue_cursor));
    }
}

// ── Rendering ─────────────────────────────────────────────────────────────────

pub fn draw(f: &mut Frame, area: Rect, state: &mut PlayerState, theme: &Theme) {
    if !state.available {
        let msg = Paragraph::new("No audio output device found. Connect headphones or speakers.")
            .style(Style::default().fg(theme.err))
            .block(Block::default().title(" player ").borders(Borders::ALL).border_style(Style::default().fg(theme.muted)));
        f.render_widget(msg, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),  // now playing + progress
            Constraint::Min(5),     // queue list
            Constraint::Length(2),  // hints
        ])
        .split(area);

    draw_now_playing(f, chunks[0], state, theme);
    draw_queue(f, chunks[1], state, theme);
    draw_hints(f, chunks[2], state, theme);
}

fn draw_now_playing(f: &mut Frame, area: Rect, state: &PlayerState, theme: &Theme) {
    let source_label = state.source.label();
    let title = format!(" player  [{source_label}] ");
    let block = Block::default()
        .title(title.as_str())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.fg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines: Vec<Line> = match &state.status.current {
        None => vec![
            Line::from(Span::styled("  — idle —", Style::default().fg(theme.muted))),
        ],
        Some(track) => {
            let artist_album = match (&track.artist, &track.album) {
                (Some(a), Some(al)) => format!("  {a}  —  {al}"),
                (Some(a), None)     => format!("  {a}"),
                (None, Some(al))    => format!("  {al}"),
                (None, None)        => String::new(),
            };
            let pause_mark = if state.status.paused { " [paused]" } else { "" };
            vec![
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&track.title, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
                    Span::styled(pause_mark, Style::default().fg(theme.warn)),
                ]),
                Line::from(Span::styled(artist_album, Style::default().fg(theme.muted))),
                Line::raw(""),
            ]
        }
    };

    // Text lines
    let text_height = lines.len() as u16;
    let [text_area, gauge_area] = if inner.height >= text_height + 2 {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(text_height), Constraint::Min(1)])
            .split(inner);
        [split[0], split[1]]
    } else {
        [inner, inner]
    };

    let para = Paragraph::new(lines);
    f.render_widget(para, text_area);

    // Progress gauge
    let (pos, dur) = (
        state.status.position,
        state.status.current.as_ref().and_then(|t| t.duration_secs).map(Duration::from_secs_f64),
    );
    let ratio = match dur {
        Some(d) if d.as_secs() > 0 => (pos.as_secs_f64() / d.as_secs_f64()).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let label = format!("  {}  /  {}", fmt_dur(pos), dur.map(fmt_dur).unwrap_or_else(|| "?:??".to_owned()));
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(theme.fg).bg(Color::DarkGray))
        .ratio(ratio)
        .label(label);
    f.render_widget(gauge, gauge_area);

    // Flash line below gauge (decode errors, etc.)
    if let Some(ref msg) = state.flash {
        let flash_area = Rect {
            y: gauge_area.y + gauge_area.height.saturating_sub(1),
            height: 1,
            ..gauge_area
        };
        let flash = Paragraph::new(Span::styled(
            format!("  {msg}"),
            Style::default().fg(theme.err),
        ));
        f.render_widget(flash, flash_area);
    }
}

fn draw_queue(f: &mut Frame, area: Rect, state: &mut PlayerState, theme: &Theme) {
    let block = Block::default()
        .title(" queue ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));

    let tracks = &state.status.queue_tracks;
    let cursor = state.status.queue_cursor;

    let items: Vec<ListItem> = tracks.iter().enumerate().map(|(i, t)| {
        let is_current = i == cursor;
        let marker = if is_current { "▶ " } else { "  " };
        let style = if is_current {
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };
        let dur = t.duration_secs
            .map(|s| format!("  {}", fmt_dur(Duration::from_secs_f64(s))))
            .unwrap_or_default();
        let label = format!("{marker}{}{dur}", t.title);
        ListItem::new(Line::from(Span::styled(label, style)))
    }).collect();

    let list = List::new(items).block(block);
    f.render_stateful_widget(list, area, &mut state.queue_list_state);
}

fn draw_hints(f: &mut Frame, area: Rect, state: &PlayerState, theme: &Theme) {
    let repeat_label = state.status.repeat.label();
    let shuffle_label = if state.status.shuffle { "on" } else { "off" };
    let hint = format!(
        "  space play/pause · n/p next/prev · L/D toggle source · r repeat:{repeat_label} · s shuffle:{shuffle_label} · q back"
    );
    let para = Paragraph::new(Span::styled(hint, Style::default().fg(theme.muted)));
    f.render_widget(para, area);
}

fn fmt_dur(d: Duration) -> String {
    let t = d.as_secs();
    let m = t / 60;
    let s = t % 60;
    if m >= 60 {
        let h = m / 60;
        let m = m % 60;
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

// ── Key handling ──────────────────────────────────────────────────────────────

pub fn handle_key(
    state: &mut PlayerState,
    handle: &PlayerHandle,
    key: crossterm::event::KeyEvent,
) -> bool {
    use crossterm::event::KeyCode as K;
    match key.code {
        K::Char(' ') => {
            if state.status.paused {
                handle.send(crate::player::engine::PlayerCommand::Resume);
                state.status.paused = false;
            } else {
                handle.send(crate::player::engine::PlayerCommand::Pause);
                state.status.paused = true;
            }
        }
        K::Char('n') => handle.send(crate::player::engine::PlayerCommand::Next),
        K::Char('p') => handle.send(crate::player::engine::PlayerCommand::Prev),
        K::Char('r') => handle.send(crate::player::engine::PlayerCommand::CycleRepeat),
        K::Char('s') => handle.send(crate::player::engine::PlayerCommand::ToggleShuffle),
        K::Char('l') | K::Char('L') => state.source = PlayerSource::Library,
        K::Char('d') | K::Char('D') => state.source = PlayerSource::Destination,
        K::Char('j') | K::Down => {
            let next = state.queue_list_state.selected().unwrap_or(0) + 1;
            let len = state.status.queue_tracks.len();
            if next < len {
                state.queue_list_state.select(Some(next));
            }
        }
        K::Char('k') | K::Up => {
            let prev = state.queue_list_state.selected().unwrap_or(0).saturating_sub(1);
            state.queue_list_state.select(Some(prev));
        }
        K::Enter => {
            if let Some(idx) = state.queue_list_state.selected() {
                handle.send(crate::player::engine::PlayerCommand::JumpTo(idx));
            }
        }
        K::Char('q') | K::Esc => return true, // signal "go back"
        _ => {}
    }
    false
}
