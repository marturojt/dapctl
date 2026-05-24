//! SSH/SFTP source support via the system `ssh` binary.
//!
//! Allows `source = "ssh://[user@]host[:port]/remote/path"` in sync profiles.
//! Authentication is fully delegated to the user's existing SSH configuration
//! (`~/.ssh/config`, `~/.ssh/id_*` keys, `ssh-agent`, `known_hosts`).
//!
//! Requirements on the remote host:
//!   - GNU `find` with `-printf` support (standard on Linux; on macOS install
//!     GNU findutils via Homebrew: `brew install findutils` and add to PATH as `find`).
//!   - `cat` (universal).
//!
//! The `ssh` binary must be in PATH. On Windows 10+ it ships with the OS.

use std::io::{Read, Write as _};
use std::process::{Command, Stdio};

use anyhow::Context;
use camino::Utf8PathBuf;
use globset::GlobSet;

use crate::diff::walker::Entry;

// ── URI ────────────────────────────────────────────────────────────────────────

/// Parsed SSH source URI: `ssh://[user@]host[:port]/absolute/path`
#[derive(Debug, Clone)]
pub struct SshUri {
    pub user: String,
    pub host: String,
    pub port: u16,
    /// Absolute path on the remote host (always starts with `/`).
    pub path: String,
}

impl SshUri {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let rest = s
            .strip_prefix("ssh://")
            .ok_or_else(|| anyhow::anyhow!("not an SSH URI: {s:?}"))?;

        let (userhost, path_tail) = rest.split_once('/').unwrap_or((rest, ""));
        let path = format!("/{path_tail}");

        let (user, hostport) = if let Some((u, h)) = userhost.rsplit_once('@') {
            (u.to_owned(), h)
        } else {
            let user = std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_else(|_| "user".to_owned());
            (user, userhost)
        };

        let (host, port) = if let Some((h, p)) = hostport.rsplit_once(':') {
            let port: u16 = p.parse().context("invalid port in SSH URI")?;
            (h.to_owned(), port)
        } else {
            (hostport.to_owned(), 22)
        };

        Ok(Self { user, host, port, path })
    }

    pub fn is_ssh(s: &str) -> bool {
        s.starts_with("ssh://")
    }
}

// ── Session ────────────────────────────────────────────────────────────────────

/// An established SSH source. All fields are plain strings; the actual SSH
/// connection is opened per-operation via `std::process::Command`.
#[derive(Debug, Clone)]
pub struct SshSession {
    user: String,
    host: String,
    port: u16,
    /// Absolute path on the remote host used as the walk/download root.
    pub remote_root: String,
}

impl SshSession {
    /// Verify that the remote is reachable and auth works by running `true`.
    pub fn connect(uri: &SshUri) -> anyhow::Result<Self> {
        let status = ssh_cmd(&uri.user, &uri.host, uri.port)
            .arg("true")
            .status()
            .context("cannot run `ssh` — is it installed and in PATH?")?;

        if !status.success() {
            anyhow::bail!(
                "SSH connection to {}@{}:{} failed (exit {}). \
                 Ensure your public key is in the remote's authorized_keys \
                 and the host is reachable.",
                uri.user,
                uri.host,
                uri.port,
                status.code().unwrap_or(-1),
            );
        }

        tracing::debug!(
            user = uri.user,
            host = uri.host,
            port = uri.port,
            path = uri.path,
            "SSH connection verified"
        );

        Ok(Self {
            user: uri.user.clone(),
            host: uri.host.clone(),
            port: uri.port,
            remote_root: uri.path.clone(),
        })
    }

    /// Walk `self.remote_root` via `find -printf`, returning entries sorted by
    /// relative path. Glob filtering is applied client-side after listing.
    pub fn walk(
        &self,
        exclude: &GlobSet,
        include: Option<&GlobSet>,
    ) -> anyhow::Result<Vec<Entry>> {
        // find prints: relative_path TAB size TAB mtime_float_secs
        let find_cmd = format!(
            "find {} -type f -printf '%P\\t%s\\t%T@\\n'",
            shell_quote(&self.remote_root)
        );

        let output = self
            .ssh_cmd()
            .arg(&find_cmd)
            .output()
            .context("failed to run remote `find`")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("remote `find` failed: {stderr}");
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut entries = Vec::new();

        for line in text.lines() {
            let mut parts = line.splitn(3, '\t');
            let (Some(rel_raw), Some(size_s), Some(mtime_s)) =
                (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };

            let rel_str = rel_raw.replace('\\', "/");
            let size: u64 = size_s.parse().unwrap_or(0);
            let mtime_ns = mtime_s
                .trim()
                .parse::<f64>()
                .map(|s| (s * 1_000_000_000.0) as i128)
                .unwrap_or(0);

            let rel = Utf8PathBuf::from(&rel_str);

            if exclude.is_match(&rel_str) {
                continue;
            }
            if let Some(inc) = include {
                if !inc.is_match(&rel_str) {
                    continue;
                }
            }

            entries.push(Entry {
                rel,
                size,
                mtime_ns,
                hash: None,   // no remote blake3 in v1.0
                src_ext: None,
            });
        }

        entries.sort_unstable_by(|a, b| a.rel.cmp(&b.rel));
        Ok(entries)
    }

    /// Download `rel_path` (relative to `self.remote_root`) to `dst_tmp` by
    /// streaming `ssh ... "cat /remote/path"` stdout to the local file.
    /// Calls `on_progress(bytes)` after each chunk.
    pub fn download(
        &self,
        rel_path: &str,
        dst_tmp: &camino::Utf8Path,
        mut on_progress: impl FnMut(u64),
    ) -> anyhow::Result<u64> {
        let remote_path = format!(
            "{}/{}",
            self.remote_root.trim_end_matches('/'),
            rel_path
        );
        let cat_cmd = format!("cat {}", shell_quote(&remote_path));

        let mut child = self
            .ssh_cmd()
            .arg(&cat_cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn ssh download")?;

        let mut stdout = child.stdout.take().expect("stdout is piped");

        let dst_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dst_tmp.as_std_path())
            .with_context(|| format!("cannot create {dst_tmp}"))?;

        let mut writer = std::io::BufWriter::with_capacity(SFTP_BUF, dst_file);
        let mut buf = vec![0u8; SFTP_BUF];
        let mut total = 0u64;

        loop {
            let n = stdout.read(&mut buf)?;
            if n == 0 {
                break;
            }
            writer.write_all(&buf[..n])?;
            total += n as u64;
            on_progress(n as u64);
        }

        writer.flush()?;
        writer.into_inner().context("SSH download flush error")?.sync_data()?;

        let status = child.wait()?;
        if !status.success() {
            anyhow::bail!("SSH download exited with error for {rel_path:?}");
        }

        Ok(total)
    }

    fn ssh_cmd(&self) -> Command {
        ssh_cmd(&self.user, &self.host, self.port)
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

const SFTP_BUF: usize = 256 * 1024;

fn ssh_cmd(user: &str, host: &str, port: u16) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.args([
        "-p",
        &port.to_string(),
        "-o",
        "StrictHostKeyChecking=accept-new", // TOFU — add unknown hosts
        "-o",
        "BatchMode=yes", // never prompt for password in TUI
        &format!("{user}@{host}"),
    ]);
    cmd
}

/// POSIX-style single-quote escaping: wrap in `'...'`, escape internal `'`.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}
