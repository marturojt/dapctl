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
    Library,
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
    /// Current volume (0.0–2.0). Tracked locally; sent to engine on change.
    pub volume: f32,
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
            volume: 1.0,
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
                PlayerEvent::QueueUpdated { tracks, cursor } => {
                    self.status.queue_tracks = tracks;
                    self.status.queue_cursor = cursor;
                    self.queue_list_state.select(Some(cursor));
                }
                PlayerEvent::TrackMetadata { idx, track } => {
                    // Progressive tag load: update individual queue entry.
                    if idx < self.status.queue_tracks.len() {
                        self.status.queue_tracks[idx] = track;
                    }
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
                    self.flash = Some(format!("{err}  ({path})"));
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
            Constraint::Length(8),  // now playing panel (6 inner + 2 borders)
            Constraint::Min(4),     // queue list
            Constraint::Length(2),  // key hints (2 lines)
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

    // ── Idle state ────────────────────────────────────────────────────────────
    if state.status.current.is_none() && state.status.queue_tracks.is_empty() {
        let msg = Paragraph::new(Span::styled(
            "  — idle —  press m from profiles to load library, or space on a diff entry",
            Style::default().fg(theme.muted),
        ));
        f.render_widget(msg, inner);
        return;
    }

    // ── Now playing ───────────────────────────────────────────────────────────
    let track = state.status.current.as_ref();

    let title_line = match track {
        None => Line::from(Span::styled("  — idle —", Style::default().fg(theme.muted))),
        Some(t) => {
            let pause_mark = if state.status.paused { "  [paused]" } else { "" };
            Line::from(vec![
                Span::raw("  "),
                Span::styled(&t.title, Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
                Span::styled(pause_mark, Style::default().fg(theme.warn)),
            ])
        }
    };

    let artist_album_line = match track {
        None => Line::raw(""),
        Some(t) => {
            let text = match (&t.artist, &t.album) {
                (Some(a), Some(al)) => format!("  {}  ·  {}", a, al),
                (Some(a), None)     => format!("  {}", a),
                (None, Some(al))    => format!("  {}", al),
                (None, None)        => String::new(),
            };
            Line::from(Span::styled(text, Style::default().fg(theme.muted)))
        }
    };

    let meta_line = {
        let fmt = track
            .map(|t| t.path.extension().unwrap_or("").to_uppercase())
            .unwrap_or_default();
        let vol_pct = (state.volume * 100.0).round() as u32;
        let text = if fmt.is_empty() {
            format!("  vol {}%", vol_pct)
        } else {
            format!("  {}  ·  vol {}%", fmt, vol_pct)
        };
        Line::from(Span::styled(text, Style::default().fg(theme.muted)))
    };

    let text_lines: Vec<Line> = vec![title_line, artist_album_line, meta_line, Line::raw("")];
    let text_height = text_lines.len() as u16;

    if inner.height < text_height + 1 {
        // Terminal too small — just render text.
        f.render_widget(Paragraph::new(text_lines), inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(text_height), Constraint::Min(1)])
        .split(inner);
    let (text_area, bottom_area) = (chunks[0], chunks[1]);

    f.render_widget(Paragraph::new(text_lines), text_area);

    // ── Progress gauge ────────────────────────────────────────────────────────
    let pos = state.status.position;
    let dur = track.and_then(|t| t.duration_secs).map(Duration::from_secs_f64);
    let ratio = match dur {
        Some(d) if d.as_secs() > 0 => (pos.as_secs_f64() / d.as_secs_f64()).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let dur_str = dur.map(fmt_dur).unwrap_or_else(|| "?:??".to_owned());
    let label = format!("  {}  /  {}", fmt_dur(pos), dur_str);

    let has_flash = state.flash.is_some();
    let gauge_height = if has_flash && bottom_area.height >= 2 {
        bottom_area.height.saturating_sub(1).max(1)
    } else {
        bottom_area.height
    };
    let gauge_area = Rect { height: gauge_height, ..bottom_area };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(theme.fg).bg(Color::DarkGray))
        .ratio(ratio)
        .label(label);
    f.render_widget(gauge, gauge_area);

    // ── Flash message ─────────────────────────────────────────────────────────
    if let Some(ref msg) = state.flash {
        if bottom_area.height >= 2 {
            let flash_area = Rect {
                y: gauge_area.y + gauge_area.height,
                height: 1,
                ..gauge_area
            };
            f.render_widget(
                Paragraph::new(Span::styled(
                    format!("  {msg}"),
                    Style::default().fg(theme.err),
                )),
                flash_area,
            );
        }
    }
}

fn draw_queue(f: &mut Frame, area: Rect, state: &mut PlayerState, theme: &Theme) {
    let n = state.status.queue_tracks.len();
    let block_title = if n > 0 {
        format!(" queue  ({n} tracks) ")
    } else {
        " queue ".to_owned()
    };
    let block = Block::default()
        .title(block_title.as_str())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));

    let inner = block.inner(area);
    let tracks = &state.status.queue_tracks;
    let cursor = state.status.queue_cursor;

    // Column widths relative to inner area.
    let w = inner.width as usize;
    let marker_w = 2usize;
    let dur_w = 7usize; // max "H:MM:SS"
    let sep = 2usize;
    let artist_w = (w / 4).max(10).min(22);
    let title_w = w
        .saturating_sub(marker_w + artist_w + sep + sep + dur_w)
        .max(4);

    let items: Vec<ListItem> = tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_current = i == cursor;
            let marker = if is_current { "▶ " } else { "  " };
            let base_style = if is_current {
                Style::default()
                    .fg(theme.fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg)
            };

            let artist = t.artist.as_deref().unwrap_or("");
            let dur = t
                .duration_secs
                .map(|s| fmt_dur(Duration::from_secs_f64(s)))
                .unwrap_or_default();

            let label = format!(
                "{marker}{:<artist_w$}  {:<title_w$}  {:>dur_w$}",
                trunc(artist, artist_w),
                trunc(&t.title, title_w),
                dur,
            );
            ListItem::new(Line::from(Span::styled(label, base_style)))
        })
        .collect();

    let list = List::new(items).block(block);
    f.render_stateful_widget(list, area, &mut state.queue_list_state);
}

fn draw_hints(f: &mut Frame, area: Rect, state: &PlayerState, theme: &Theme) {
    let repeat_label = state.status.repeat.label();
    let shuffle_label = if state.status.shuffle { "on" } else { "off" };

    let line1 = format!(
        "  space play/pause · n/p next/prev · ←/→ seek ±30s · +/- vol · q back"
    );
    let line2 = format!(
        "  j/k scroll · Enter jump · r repeat:{repeat_label} · s shuffle:{shuffle_label} · L/D source"
    );

    let style = Style::default().fg(theme.muted);
    let para = Paragraph::new(vec![
        Line::from(Span::styled(line1, style)),
        Line::from(Span::styled(line2, style)),
    ]);
    f.render_widget(para, area);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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

/// Truncate a string to at most `max` chars, appending `…` if trimmed.
fn trunc(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_owned()
    } else {
        let end = max.saturating_sub(1);
        let truncated: String = chars[..end].iter().collect();
        format!("{truncated}…")
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
        K::Char('+') | K::Char('=') => {
            state.volume = (state.volume + 0.05).min(2.0);
            handle.send(crate::player::engine::PlayerCommand::Volume(state.volume));
        }
        K::Char('-') => {
            state.volume = (state.volume - 0.05).max(0.0);
            handle.send(crate::player::engine::PlayerCommand::Volume(state.volume));
        }
        K::Left => handle.send(crate::player::engine::PlayerCommand::SeekRelative(-30)),
        K::Right => handle.send(crate::player::engine::PlayerCommand::SeekRelative(30)),
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
