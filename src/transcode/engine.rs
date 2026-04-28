use std::process::Command;

use anyhow::Context;
use camino::Utf8Path;

use crate::config::TranscodeRule;

/// Transcode `src` to `out_path` using the given rule's ffmpeg params.
///
/// ffmpeg is invoked as:
///   ffmpeg -i <src> [params...] -y <out_path>
///
/// The caller is responsible for creating parent directories and renaming
/// `out_path` to its final destination. Returns an error if ffmpeg exits
/// with a non-zero status.
pub fn transcode(src: &Utf8Path, out_path: &Utf8Path, rule: &TranscodeRule) -> anyhow::Result<()> {
    let extra: Vec<&str> = rule.params.split_whitespace().collect();

    let output = Command::new("ffmpeg")
        .arg("-i")
        .arg(src.as_str())
        .args(&extra)
        .arg("-y") // overwrite output if it exists
        .arg(out_path.as_str())
        .output()
        .with_context(|| format!("failed to launch ffmpeg for {src}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let hint = stderr.lines().last().unwrap_or("(no stderr)");
        anyhow::bail!(
            "ffmpeg exited {} transcoding {src}: {hint}",
            output.status.code().unwrap_or(-1),
        );
    }

    Ok(())
}
