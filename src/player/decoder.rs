use std::io::{BufReader, Read};
use std::process::{Child, ChildStdout, Command, Stdio};

use camino::Utf8Path;

/// File extensions that indicate DSD audio (require ffmpeg for playback).
const DSD_EXTENSIONS: &[&str] = &["dsf", "dff"];

pub fn is_dsd(path: &Utf8Path) -> bool {
    path.extension()
        .map(|e| DSD_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// A rodio `Source` backed by an ffmpeg pipe decoding DSD to PCM f32le.
///
/// ffmpeg is invoked as:
///   ffmpeg -i <path> -f f32le -ar 176400 -ac 2 -vn pipe:1
///
/// Output: stereo, 176.4 kHz, f32 little-endian.
pub struct DsdSource {
    _child: Child,
    reader: BufReader<ChildStdout>,
    buf: [u8; 4],
}

impl DsdSource {
    pub fn open(path: &Utf8Path) -> anyhow::Result<Self> {
        let mut child = Command::new("ffmpeg")
            .args([
                "-i",
                path.as_str(),
                "-f",
                "f32le",
                "-ar",
                "176400",
                "-ac",
                "2",
                "-vn", // no video
                "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("cannot launch ffmpeg for DSD playback: {e}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("ffmpeg stdout not captured"))?;

        Ok(Self {
            _child: child,
            reader: BufReader::new(stdout),
            buf: [0u8; 4],
        })
    }
}

impl Iterator for DsdSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.reader.read_exact(&mut self.buf).ok()?;
        Some(f32::from_le_bytes(self.buf))
    }
}

impl rodio::Source for DsdSource {
    fn current_frame_len(&self) -> Option<usize> {
        None // streaming — frame length unknown
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        176_400
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None // DSD duration unknown without full parse
    }
}
