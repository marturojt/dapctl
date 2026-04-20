# MTP Workflow (via pre-mount)

dapctl does not speak MTP directly. However, if you mount your DAP's
storage using an external tool, dapctl works transparently — it just
sees a regular directory.

This is useful when you cannot extract the microSD (e.g. devices with
internal storage only, or when a card reader is not at hand).

---

## Linux

Install `jmtpfs` or `go-mtpfs`, then mount and sync:

```bash
# Install (Debian/Ubuntu)
sudo apt install jmtpfs

# Mount
mkdir -p ~/mnt/dap
jmtpfs ~/mnt/dap

# Sync
dapctl sync my-profile   # set destination = /home/<user>/mnt/dap/Music

# Unmount when done
fusermount -u ~/mnt/dap
```

`go-mtpfs` works the same way:

```bash
go install github.com/hanwen/go-mtpfs@latest
go-mtpfs ~/mnt/dap &
dapctl sync my-profile
fusermount -u ~/mnt/dap
```

---

## macOS

```bash
brew install go-mtpfs

mkdir -p ~/mnt/dap
go-mtpfs ~/mnt/dap &
dapctl sync my-profile
umount ~/mnt/dap
```

---

## Windows

Requires [WinFsp](https://winfsp.dev) (free, open source) and
[rclone](https://rclone.org).

```powershell
# One-time: configure the MTP remote
rclone config
# → choose "new remote" → type "mtp" → follow prompts

# Mount as drive letter Z:
rclone mount dap: Z:\ --vfs-cache-mode full

# In a second terminal
dapctl sync my-profile   # set destination = Z:\Music

# Unmount (first terminal)
Ctrl+C
```

Alternatively, use **[WinFsp + SSHFS](https://github.com/winfsp/sshfs-win)**
if your DAP is reachable via SSH (Android DAPs with SSHDroid or
similar).

---

## Sync profile for a mounted DAP

Use a literal path as `destination` instead of `auto:<dap-id>`:

```toml
[profile]
name        = "m11plus-via-mount"
source      = "/mnt/music"
destination = "/home/user/mnt/dap/Music"   # or Z:\Music on Windows
dap_profile = "generic"                    # or your DAP's profile id
mode        = "mirror"
```

---

## Caveats

- **Performance**: MTP throughput is typically 10–15 MB/s, versus
  40–80 MB/s with a card reader. For a first sync of a large library,
  expect significantly longer times.
- **No atomic rename**: MTP does not support `rename()`. The
  temp+rename safety pattern dapctl uses for direct filesystem access
  does not apply here; interrupted transfers may leave partial files.
  Always run `dapctl diff` before `sync` and keep a backup.
- **Screen timeout**: some DAPs disconnect MTP when the screen turns
  off. Disable auto-lock before a long sync.
- **Preferred workflow**: for any sync larger than a few hundred MB,
  the microSD card reader workflow remains faster and safer.
