use std::io::BufReader;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use camino::Utf8Path;

use crate::player::decoder;
use crate::player::queue::{Queue, RepeatMode, TrackInfo};

// ── Public command / event types ──────────────────────────────────────────────

#[derive(Debug)]
pub enum PlayerCommand {
    /// Load and start playing a full queue from position 0.
    PlayQueue(Vec<TrackInfo>),
    /// Push a single track to the end of the queue.
    Enqueue(TrackInfo),
    /// Jump to queue index and start playing.
    JumpTo(usize),
    Pause,
    Resume,
    Stop,
    Next,
    Prev,
    ToggleShuffle,
    CycleRepeat,
    /// Seek to absolute position.
    Seek(Duration),
    /// Seek relative to current position (positive = forward, negative = back).
    SeekRelative(i64),
    /// Set playback volume (0.0 = mute, 1.0 = 100%, 2.0 = 200%).
    Volume(f32),
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    TrackStarted(TrackInfo),
    Position(Duration),
    TrackEnded,
    QueueEmpty,
    QueueUpdated { tracks: Vec<TrackInfo>, cursor: usize },
    Stopped,
    DecodeError { path: String, err: String },
    /// Background tag scan result for a single queue entry.
    TrackMetadata { idx: usize, track: TrackInfo },
}

// ── Player state (returned by handle for TUI display) ─────────────────────────

#[derive(Debug, Clone)]
pub struct PlayerStatus {
    pub current: Option<TrackInfo>,
    pub position: Duration,
    pub paused: bool,
    pub queue_cursor: usize,
    pub queue_tracks: Vec<TrackInfo>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub volume: f32,
}

impl Default for PlayerStatus {
    fn default() -> Self {
        Self {
            current: None,
            position: Duration::ZERO,
            paused: false,
            queue_cursor: 0,
            queue_tracks: Vec::new(),
            shuffle: false,
            repeat: RepeatMode::Off,
            volume: 1.0,
        }
    }
}

impl Default for RepeatMode {
    fn default() -> Self {
        RepeatMode::Off
    }
}

// ── Player handle (TUI-facing API) ────────────────────────────────────────────

/// Cheap-to-clone handle for sending commands to the audio thread.
#[derive(Clone)]
pub struct PlayerHandle {
    tx: Sender<PlayerCommand>,
}

impl PlayerHandle {
    pub fn send(&self, cmd: PlayerCommand) {
        let _ = self.tx.send(cmd);
    }
}

// ── Engine (runs in a dedicated thread) ───────────────────────────────────────

struct Engine {
    cmd_rx: Receiver<PlayerCommand>,
    event_tx: Sender<PlayerEvent>,
    queue: Queue,
    sink: rodio::Sink,
    paused: bool,
    track_done: bool,
}

impl Engine {
    fn new(
        cmd_rx: Receiver<PlayerCommand>,
        event_tx: Sender<PlayerEvent>,
        sink: rodio::Sink,
    ) -> Self {
        Self {
            cmd_rx,
            event_tx,
            queue: Queue::new(),
            sink,
            paused: false,
            track_done: false,
        }
    }

    fn run(mut self) {
        loop {
            // Poll commands without blocking more than ~50 ms so we can
            // emit position events regularly.
            match self.cmd_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(cmd) => {
                    if !self.handle_command(cmd) {
                        break; // Stop command or channel closed
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }

            // Emit position tick.
            if !self.sink.empty() && !self.paused {
                let pos = self.sink.get_pos();
                let _ = self.event_tx.send(PlayerEvent::Position(pos));
            }

            // Detect track end: sink emptied naturally.
            if self.track_done && self.sink.empty() && !self.paused {
                self.track_done = false;
                let _ = self.event_tx.send(PlayerEvent::TrackEnded);
                if self.queue.advance() {
                    self.play_current();
                } else {
                    let _ = self.event_tx.send(PlayerEvent::QueueEmpty);
                }
            }

            // Mark track as done when sink is about to empty.
            if !self.sink.empty() {
                self.track_done = true;
            }
        }
    }

    /// Returns `false` when the engine should exit.
    fn handle_command(&mut self, cmd: PlayerCommand) -> bool {
        match cmd {
            PlayerCommand::PlayQueue(tracks) => {
                self.sink.stop();
                self.queue.set(tracks.clone());
                self.paused = false;
                self.emit_queue_updated();
                self.play_current();

                // Spawn background tag scanner so the queue display fills in
                // progressively without blocking the audio thread.
                let tx = self.event_tx.clone();
                std::thread::spawn(move || {
                    for (idx, track) in tracks.into_iter().enumerate() {
                        let tagged = track.with_tags();
                        if tx.send(PlayerEvent::TrackMetadata { idx, track: tagged }).is_err() {
                            break;
                        }
                    }
                });
            }
            PlayerCommand::Enqueue(track) => {
                self.queue.push(track);
                self.emit_queue_updated();
            }
            PlayerCommand::JumpTo(idx) => {
                self.sink.stop();
                self.queue.jump_to(idx);
                self.paused = false;
                self.play_current();
            }
            PlayerCommand::Pause => {
                self.sink.pause();
                self.paused = true;
            }
            PlayerCommand::Resume => {
                self.sink.play();
                self.paused = false;
            }
            PlayerCommand::Stop => {
                self.sink.stop();
                self.paused = false;
                let _ = self.event_tx.send(PlayerEvent::Stopped);
                return true;
            }
            PlayerCommand::Next => {
                self.sink.stop();
                if self.queue.advance() {
                    self.paused = false;
                    self.play_current();
                } else {
                    let _ = self.event_tx.send(PlayerEvent::QueueEmpty);
                }
            }
            PlayerCommand::Prev => {
                self.sink.stop();
                self.queue.prev();
                self.paused = false;
                self.play_current();
            }
            PlayerCommand::ToggleShuffle => {
                self.queue.toggle_shuffle();
            }
            PlayerCommand::CycleRepeat => {
                self.queue.repeat = self.queue.repeat.next();
            }
            PlayerCommand::Seek(pos) => {
                let _ = self.sink.try_seek(pos);
            }
            PlayerCommand::SeekRelative(delta_secs) => {
                let pos = self.sink.get_pos();
                let new_secs = (pos.as_secs_f64() + delta_secs as f64).max(0.0);
                let _ = self.sink.try_seek(Duration::from_secs_f64(new_secs));
            }
            PlayerCommand::Volume(v) => {
                self.sink.set_volume(v.clamp(0.0, 2.0));
            }
        }
        true
    }

    fn emit_queue_updated(&self) {
        let _ = self.event_tx.send(PlayerEvent::QueueUpdated {
            tracks: self.queue.tracks().to_vec(),
            cursor: self.queue.cursor(),
        });
    }

    fn play_current(&mut self) {
        let Some(track) = self.queue.current().cloned() else { return };
        self.sink.stop();

        // Load tags synchronously for the current track so Now Playing always
        // shows full metadata (artist, album, duration). Brief IO (~few ms).
        let track = track.with_tags();
        if let Some(phys_idx) = self.queue.current_phys_idx() {
            self.queue.update_at(phys_idx, track.clone());
        }

        match play_path(&self.sink, &track.path) {
            Ok(()) => {
                // Emit updated queue first so the TUI sees the tagged current entry.
                self.emit_queue_updated();
                let _ = self.event_tx.send(PlayerEvent::TrackStarted(track));
                self.track_done = false;
            }
            Err(e) => {
                let _ = self.event_tx.send(PlayerEvent::DecodeError {
                    path: track.path.to_string(),
                    err: e.to_string(),
                });
                // Try advancing to next track automatically.
                if self.queue.advance() {
                    self.play_current();
                }
            }
        }
    }
}

/// Append the appropriate source to the sink and start playback.
///
/// - DSD (DSF/DFF): piped through ffmpeg → PCM f32le 176.4 kHz stereo.
///   Returns an error with a clear ⚠ message if ffmpeg is not in PATH.
/// - Everything else: decoded natively via rodio + symphonia.
fn play_path(sink: &rodio::Sink, path: &Utf8Path) -> anyhow::Result<()> {
    if decoder::is_dsd(path) {
        match crate::transcode::ffmpeg::detect() {
            None => anyhow::bail!(
                "⚠  DSD playback requires ffmpeg in PATH — \
                 install ffmpeg to play {}",
                path.file_name().unwrap_or(path.as_str())
            ),
            Some(_) => {
                let src = decoder::DsdSource::open(path)?;
                sink.append(src);
            }
        }
    } else {
        let file = std::fs::File::open(path.as_std_path())
            .map_err(|e| anyhow::anyhow!("cannot open {path}: {e}"))?;
        let src = rodio::Decoder::new(BufReader::new(file))
            .map_err(|e| anyhow::anyhow!("cannot decode {path}: {e}"))?;
        sink.append(src);
    }
    sink.play();
    Ok(())
}

// ── Public constructor ────────────────────────────────────────────────────────

/// Spawn the audio engine on a background thread.
///
/// The `OutputStream` (WASAPI/CoreAudio/ALSA) is created inside the spawned
/// thread because on most platforms it is not `Send`. An availability flag is
/// returned via a one-shot channel.
///
/// Returns `None` when no audio output device is available — callers should
/// display a warning instead of crashing.
pub fn spawn() -> Option<(PlayerHandle, Receiver<PlayerEvent>)> {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<PlayerCommand>();
    let (event_tx, event_rx) = std::sync::mpsc::channel::<PlayerEvent>();

    // Signal whether the audio device was available.
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<bool>(1);

    std::thread::spawn(move || {
        let Ok((stream, stream_handle)) = rodio::OutputStream::try_default() else {
            let _ = ready_tx.send(false);
            return;
        };
        let Ok(sink) = rodio::Sink::try_new(&stream_handle) else {
            let _ = ready_tx.send(false);
            return;
        };
        let _ = ready_tx.send(true);

        // Keep stream alive for the lifetime of the engine.
        let _stream = stream;
        let engine = Engine::new(cmd_rx, event_tx, sink);
        engine.run();
    });

    // Block briefly for the device check — this is at TUI startup, not hot path.
    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(true) => Some((PlayerHandle { tx: cmd_tx }, event_rx)),
        _ => None,
    }
}
