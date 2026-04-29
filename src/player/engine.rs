use std::io::BufReader;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use camino::Utf8Path;

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
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    TrackStarted(TrackInfo),
    Position(Duration),
    TrackEnded,
    QueueEmpty,
    Stopped,
    DecodeError { path: String, err: String },
}

// ── Player state (returned by handle for TUI display) ─────────────────────────

#[derive(Debug, Clone, Default)]
pub struct PlayerStatus {
    pub current: Option<TrackInfo>,
    pub position: Duration,
    pub paused: bool,
    pub queue_cursor: usize,
    pub queue_tracks: Vec<TrackInfo>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
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
                self.queue.set(tracks);
                self.paused = false;
                self.play_current();
            }
            PlayerCommand::Enqueue(track) => {
                self.queue.push(track);
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
        }
        true
    }

    fn play_current(&mut self) {
        let Some(track) = self.queue.current().cloned() else { return };
        match open_source(&track.path) {
            Ok(src) => {
                self.sink.stop();
                self.sink.append(src);
                self.sink.play();
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

/// Open a file and return a rodio Source. Uses rodio's symphonia backend.
fn open_source(path: &Utf8Path) -> anyhow::Result<rodio::Decoder<BufReader<std::fs::File>>> {
    let file = std::fs::File::open(path.as_std_path())
        .map_err(|e| anyhow::anyhow!("cannot open {path}: {e}"))?;
    let src = rodio::Decoder::new(BufReader::new(file))
        .map_err(|e| anyhow::anyhow!("cannot decode {path}: {e}"))?;
    Ok(src)
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
