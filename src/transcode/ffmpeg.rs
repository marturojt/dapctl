use std::process::Command;

#[derive(Debug, Clone)]
pub struct FfmpegInfo {
    /// Version string reported by `ffmpeg -version`, e.g. `"7.0"`.
    pub version: String,
}

/// Probe for `ffmpeg` in PATH. Returns `None` if not found or not executable.
pub fn detect() -> Option<FfmpegInfo> {
    let output = Command::new("ffmpeg").arg("-version").output().ok()?;

    // ffmpeg writes its banner to stdout; exit code is 0 even for -version.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let version = stdout
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("ffmpeg version "))
        .and_then(|s| s.split_whitespace().next())
        .unwrap_or("unknown")
        .to_string();

    Some(FfmpegInfo { version })
}
