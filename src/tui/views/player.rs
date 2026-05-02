use std::sync::mpsc::Receiver;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph};

use crate::player::engine::{PlayerEvent, PlayerHandle, PlayerStatus};
use crate::player::library::{LibraryIndex, LibraryNode};
use crate::player::queue::TrackInfo;
use crate::tui::theme::Theme;

// ── Focus ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerFocus {
    Library,
    Queue,
}

impl PlayerFocus {
    fn toggle(self) -> Self {
        match self {
            Self::Library => Self::Queue,
            Self::Queue   => Self::Library,
        }
    }
}

// ── Library UI state ──────────────────────────────────────────────────────────

pub struct LibraryState {
    pub index: LibraryIndex,
    pub expanded: Vec<bool>,
    pub flat: Vec<LibraryNode>,
    pub cursor: usize,
    pub list_state: ListState,
    pub search_active: bool,
    pub search_input: tui_input::Input,
}

impl LibraryState {
    pub fn new(index: LibraryIndex) -> Self {
        let n = index.artists.len();
        let expanded = vec![false; n];
        let flat = index.build_flat(&expanded, "");
        let mut list_state = ListState::default();
        if !flat.is_empty() {
            list_state.select(Some(0));
        }
        Self { index, expanded, flat, cursor: 0, list_state, search_active: false, search_input: tui_input::Input::default() }
    }

    pub fn rebuild_flat(&mut self) {
        let query = if self.search_active { self.search_input.value() } else { "" };
        self.flat = self.index.build_flat(&self.expanded, query);
        if self.flat.is_empty() {
            self.list_state.select(None);
        } else {
            self.cursor = self.cursor.min(self.flat.len() - 1);
            self.list_state.select(Some(self.cursor));
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.flat.len() {
            self.cursor += 1;
            self.list_state.select(Some(self.cursor));
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.list_state.select(Some(self.cursor));
        }
    }

    /// Toggle expand for the current Artist row; no-op on Album rows.
    pub fn toggle_expand(&mut self) {
        if let Some(LibraryNode::Artist(ai)) = self.flat.get(self.cursor).copied() {
            if let Some(e) = self.expanded.get_mut(ai) {
                *e = !*e;
                self.rebuild_flat();
            }
        }
    }

    /// Tracks for the currently selected Album node, if any.
    pub fn selected_album_tracks(&self) -> Option<Vec<TrackInfo>> {
        if let Some(LibraryNode::Album { artist: ai, album: ali }) = self.flat.get(self.cursor) {
            self.index.artists.get(*ai)?
                .albums.get(*ali)
                .map(|al| al.tracks.clone())
        } else {
            None
        }
    }
}

// ── Player source ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSource {
    Library,
    Destination,
}

impl PlayerSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Library     => "L library",
            Self::Destination => "D destination",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Library     => Self::Destination,
            Self::Destination => Self::Library,
        }
    }
}

// ── Player view state ─────────────────────────────────────────────────────────

pub struct PlayerState {
    pub status: PlayerStatus,
    pub source: PlayerSource,
    pub queue_list_state: ListState,
    pub flash: Option<String>,
    pub available: bool,
    pub volume: f32,
    pub library: Option<LibraryState>,
    pub focus: PlayerFocus,
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
            library: None,
            focus: PlayerFocus::Queue,
        }
    }

    pub fn set_library(&mut self, index: LibraryIndex) {
        self.library = Some(LibraryState::new(index));
        self.focus = PlayerFocus::Library;
    }

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
            .block(
                Block::default()
                    .title(" player ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.muted)),
            );
        f.render_widget(msg, area);
        return;
    }

    // Outer split: [main content] / [2-line hints footer]
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(2)])
        .split(area);
    let (main_area, hints_area) = (outer[0], outer[1]);

    if state.library.is_some() {
        // 3-pane: [library (38%)] | [now_playing (top) + queue (bottom) (62%)]
        let horiz = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(main_area);
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(3)])
            .split(horiz[1]);

        draw_library(f, horiz[0], state, theme);
        draw_now_playing(f, right[0], state, theme);
        draw_queue(f, right[1], state, theme);
    } else {
        // 2-pane: [now_playing (8)] / [queue (fills)]
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(4)])
            .split(main_area);
        draw_now_playing(f, chunks[0], state, theme);
        draw_queue(f, chunks[1], state, theme);
    }

    draw_hints(f, hints_area, state, theme);
}

// ── Library pane ──────────────────────────────────────────────────────────────

fn draw_library(f: &mut Frame, area: Rect, state: &mut PlayerState, theme: &Theme) {
    let Some(ref mut lib) = state.library else { return };
    let is_focused = state.focus == PlayerFocus::Library;
    let border_style = Style::default().fg(if is_focused { theme.fg } else { theme.muted });

    let n_artists = lib.index.artists.len();
    let block_title = if lib.search_active {
        format!(" / {}_ ", lib.search_input.value())
    } else if n_artists > 0 {
        format!(" library  ({n_artists}) ")
    } else {
        " library ".to_owned()
    };

    let block = Block::default()
        .title(block_title.as_str())
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if lib.flat.is_empty() {
        let hint = if lib.index.is_empty() {
            "  — empty library —"
        } else {
            "  — no results —"
        };
        f.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(theme.muted))),
            inner,
        );
        return;
    }

    let w = inner.width as usize;
    let cursor = lib.cursor;

    let items: Vec<ListItem> = lib.flat.iter().enumerate().map(|(i, node)| {
        let sel = i == cursor;
        match node {
            LibraryNode::Artist(ai) => {
                let artist = &lib.index.artists[*ai];
                let exp = lib.expanded.get(*ai).copied().unwrap_or(false);
                let icon = if exp { "▼ " } else { "▶ " };
                let label = trunc(&format!("{icon}{}", artist.name), w);
                let style = if sel && is_focused {
                    Style::default()
                        .fg(theme.sel_fg)
                        .bg(theme.sel_bg)
                        .add_modifier(Modifier::BOLD)
                } else if sel {
                    Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.fg)
                };
                ListItem::new(Line::from(Span::styled(label, style)))
            }
            LibraryNode::Album { artist: ai, album: ali } => {
                let album = &lib.index.artists[*ai].albums[*ali];
                let count = format!("({})", album.tracks.len());
                let name_w = w.saturating_sub(count.len() + 4);
                let label = format!("  {:<name_w$}  {count}", trunc(&album.name, name_w));
                let style = if sel && is_focused {
                    Style::default().fg(theme.sel_fg).bg(theme.sel_bg)
                } else if sel {
                    Style::default().fg(theme.fg)
                } else {
                    Style::default().fg(theme.muted)
                };
                ListItem::new(Line::from(Span::styled(label, style)))
            }
        }
    }).collect();

    let list = List::new(items);
    f.render_stateful_widget(list, inner, &mut lib.list_state);
}

// ── Now Playing ───────────────────────────────────────────────────────────────

fn draw_now_playing(f: &mut Frame, area: Rect, state: &PlayerState, theme: &Theme) {
    let source_label = state.source.label();
    let title = format!(" player  [{source_label}] ");
    let block = Block::default()
        .title(title.as_str())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.fg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Idle state
    if state.status.current.is_none() && state.status.queue_tracks.is_empty() {
        let idle_hint = if state.library.is_some() {
            "  — idle —  press Enter on an album in the library to play"
        } else {
            "  — idle —  press m from profiles to load library, or space on a diff entry"
        };
        f.render_widget(
            Paragraph::new(Span::styled(idle_hint, Style::default().fg(theme.muted))),
            inner,
        );
        return;
    }

    let track = state.status.current.as_ref();

    let title_line = match track {
        None    => Line::from(Span::styled("  — idle —", Style::default().fg(theme.muted))),
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
        None    => Line::raw(""),
        Some(t) => {
            let text = match (&t.artist, &t.album) {
                (Some(a), Some(al)) => format!("  {}  ·  {}", a, al),
                (Some(a), None)     => format!("  {}", a),
                (None,    Some(al)) => format!("  {}", al),
                (None,    None)     => String::new(),
            };
            Line::from(Span::styled(text, Style::default().fg(theme.muted)))
        }
    };

    let meta_line = Line::from(Span::styled(
        match track {
            Some(t) => fmt_hifi_line(t, state.volume),
            None    => format!("  vol {}%", (state.volume * 100.0).round() as u32),
        },
        Style::default().fg(theme.muted),
    ));

    let text_lines: Vec<Line> = vec![title_line, artist_album_line, meta_line, Line::raw("")];
    let text_height = text_lines.len() as u16;

    if inner.height < text_height + 1 {
        f.render_widget(Paragraph::new(text_lines), inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(text_height), Constraint::Min(1)])
        .split(inner);
    let (text_area, bottom_area) = (chunks[0], chunks[1]);
    f.render_widget(Paragraph::new(text_lines), text_area);

    // Progress gauge
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

    f.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(theme.fg).bg(Color::DarkGray))
            .ratio(ratio)
            .label(label),
        gauge_area,
    );

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

// ── Queue ─────────────────────────────────────────────────────────────────────

fn draw_queue(f: &mut Frame, area: Rect, state: &mut PlayerState, theme: &Theme) {
    let n = state.status.queue_tracks.len();
    let block_title = if n > 0 {
        format!(" queue  ({n} tracks) ")
    } else {
        " queue ".to_owned()
    };

    let has_library = state.library.is_some();
    let queue_focused = !has_library || state.focus == PlayerFocus::Queue;
    let border_style = Style::default().fg(
        if queue_focused && has_library { theme.fg } else { theme.muted }
    );

    let block = Block::default()
        .title(block_title.as_str())
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    let tracks = &state.status.queue_tracks;
    let cursor = state.status.queue_cursor;

    let w = inner.width as usize;
    let marker_w = 2usize;
    let dur_w    = 7usize;
    let sep      = 2usize;
    let artist_w = (w / 4).max(10).min(22);
    let title_w  = w.saturating_sub(marker_w + artist_w + sep + sep + dur_w).max(4);

    let items: Vec<ListItem> = tracks.iter().enumerate().map(|(i, t)| {
        let is_current = i == cursor;
        let marker = if is_current { "▶ " } else { "  " };
        let base_style = if is_current {
            Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg)
        };

        let artist = t.artist.as_deref().unwrap_or("");
        let dur = t.duration_secs
            .map(|s| fmt_dur(Duration::from_secs_f64(s)))
            .unwrap_or_default();

        let label = format!(
            "{marker}{:<artist_w$}  {:<title_w$}  {:>dur_w$}",
            trunc(artist, artist_w),
            trunc(&t.title, title_w),
            dur,
        );
        ListItem::new(Line::from(Span::styled(label, base_style)))
    }).collect();

    let list = List::new(items).block(block);
    f.render_stateful_widget(list, area, &mut state.queue_list_state);
}

// ── Hints ─────────────────────────────────────────────────────────────────────

fn draw_hints(f: &mut Frame, area: Rect, state: &PlayerState, theme: &Theme) {
    let repeat_label  = state.status.repeat.label();
    let shuffle_label = if state.status.shuffle { "on" } else { "off" };

    let (line1, line2) = if state.library.is_some() {
        match state.focus {
            PlayerFocus::Library => (
                "  space pause · n/p next/prev · ←/→ seek · +/- vol · Tab→queue · q back",
                "  j/k nav · Enter expand/play · / search",
            ),
            PlayerFocus::Queue => (
                "  space pause · n/p next/prev · ←/→ seek · +/- vol · Tab→library · q back",
                &*format!("  j/k scroll · Enter jump · r repeat:{repeat_label} · s shuffle:{shuffle_label}"),
            ),
        }
    } else {
        (
            "  space play/pause · n/p next/prev · ←/→ seek ±30s · +/- vol · q back",
            &*format!("  j/k scroll · Enter jump · r repeat:{repeat_label} · s shuffle:{shuffle_label} · L/D source"),
        )
    };

    let style = Style::default().fg(theme.muted);
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(line1, style)),
            Line::from(Span::styled(line2, style)),
        ]),
        area,
    );
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

fn fmt_hifi_line(track: &TrackInfo, volume: f32) -> String {
    let fmt = track.path.extension().unwrap_or("").to_uppercase();
    let vol_pct = (volume * 100.0).round() as u32;

    let mut parts: Vec<String> = Vec::new();
    if !fmt.is_empty() {
        parts.push(fmt);
    }
    match (track.sample_rate_hz, track.bit_depth) {
        (Some(sr), Some(bd)) => parts.push(format!("{}/{bd}bit", fmt_sr(sr))),
        (Some(sr), None)     => parts.push(fmt_sr(sr)),
        (None,     Some(bd)) => parts.push(format!("{bd}bit")),
        (None,     None)     => {}
    }
    if let Some(ch) = track.channels {
        parts.push(format!("{ch}ch"));
    }
    if let Some(br) = track.bitrate_kbps {
        parts.push(format!("{br}kbps"));
    }
    parts.push(format!("vol {vol_pct}%"));

    format!("  {}", parts.join("  ·  "))
}

fn fmt_sr(hz: u32) -> String {
    if hz % 1000 == 0 {
        format!("{}kHz", hz / 1000)
    } else {
        format!("{:.1}kHz", hz as f64 / 1000.0)
    }
}

fn trunc(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_owned()
    } else {
        let end = max.saturating_sub(1);
        format!("{}…", chars[..end].iter().collect::<String>())
    }
}

// ── Key handling ──────────────────────────────────────────────────────────────

pub fn handle_key(
    state: &mut PlayerState,
    handle: &PlayerHandle,
    key: crossterm::event::KeyEvent,
) -> bool {
    use crossterm::event::KeyCode as K;

    // Route to search handler when library search is active and library is focused
    if state.focus == PlayerFocus::Library
        && state.library.as_ref().map_or(false, |l| l.search_active)
    {
        return handle_search_key(state, handle, key);
    }

    match key.code {
        // Go back
        K::Char('q') | K::Esc => return true,

        // Switch focus between library and queue panes
        K::Tab => {
            if state.library.is_some() {
                state.focus = state.focus.toggle();
            }
        }

        // Playback controls (global — work regardless of focus)
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
        K::Left  => handle.send(crate::player::engine::PlayerCommand::SeekRelative(-30)),
        K::Right => handle.send(crate::player::engine::PlayerCommand::SeekRelative(30)),

        // Activate library search (library must be focused)
        K::Char('/') if state.focus == PlayerFocus::Library => {
            if let Some(ref mut lib) = state.library {
                lib.search_active = true;
            }
        }

        // Navigation — behaviour depends on focus
        K::Char('j') | K::Down => match state.focus {
            PlayerFocus::Library => {
                if let Some(ref mut lib) = state.library {
                    lib.move_down();
                }
            }
            PlayerFocus::Queue => {
                let next = state.queue_list_state.selected().unwrap_or(0) + 1;
                if next < state.status.queue_tracks.len() {
                    state.queue_list_state.select(Some(next));
                }
            }
        },
        K::Char('k') | K::Up => match state.focus {
            PlayerFocus::Library => {
                if let Some(ref mut lib) = state.library {
                    lib.move_up();
                }
            }
            PlayerFocus::Queue => {
                let prev = state.queue_list_state.selected().unwrap_or(0).saturating_sub(1);
                state.queue_list_state.select(Some(prev));
            }
        },
        K::Enter => match state.focus {
            PlayerFocus::Library => handle_library_enter(state, handle),
            PlayerFocus::Queue   => {
                if let Some(idx) = state.queue_list_state.selected() {
                    handle.send(crate::player::engine::PlayerCommand::JumpTo(idx));
                }
            }
        },

        _ => {}
    }
    false
}

/// Handle keys while library search is active.
fn handle_search_key(
    state: &mut PlayerState,
    handle: &PlayerHandle,
    key: crossterm::event::KeyEvent,
) -> bool {
    use crossterm::event::KeyCode as K;
    use tui_input::backend::crossterm::EventHandler;

    match key.code {
        // Cancel search
        K::Esc | K::Char('/') => {
            if let Some(ref mut lib) = state.library {
                lib.search_active = false;
                lib.search_input = tui_input::Input::default();
                lib.rebuild_flat();
            }
        }
        // Switch focus (search stays active)
        K::Tab => {
            state.focus = state.focus.toggle();
        }
        // Navigation through filtered results
        K::Char('j') | K::Down => {
            if let Some(ref mut lib) = state.library {
                lib.move_down();
            }
        }
        K::Char('k') | K::Up => {
            if let Some(ref mut lib) = state.library {
                lib.move_up();
            }
        }
        // Select (expand artist / load album)
        K::Enter => handle_library_enter(state, handle),
        // Anything else feeds the search input
        _ => {
            if let Some(ref mut lib) = state.library {
                lib.search_input.handle_event(&crossterm::event::Event::Key(key));
                lib.rebuild_flat();
            }
        }
    }
    false
}

/// Expand/collapse artist or load album into queue.
fn handle_library_enter(state: &mut PlayerState, handle: &PlayerHandle) {
    enum LibAction {
        ToggleExpand,
        PlayAlbum(Vec<TrackInfo>),
    }

    let action = state.library.as_ref().and_then(|lib| {
        match lib.flat.get(lib.cursor)? {
            LibraryNode::Artist(_) => Some(LibAction::ToggleExpand),
            LibraryNode::Album { artist: ai, album: ali } => {
                let tracks = lib.index.artists.get(*ai)?
                    .albums.get(*ali)
                    .map(|al| al.tracks.clone())?;
                Some(LibAction::PlayAlbum(tracks))
            }
        }
    });

    match action {
        Some(LibAction::ToggleExpand) => {
            if let Some(ref mut lib) = state.library {
                lib.toggle_expand();
            }
        }
        Some(LibAction::PlayAlbum(tracks)) => {
            handle.send(crate::player::engine::PlayerCommand::PlayQueue(tracks));
            state.focus = PlayerFocus::Queue;
        }
        None => {}
    }
}
