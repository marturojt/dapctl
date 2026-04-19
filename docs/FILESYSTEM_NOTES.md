# Filesystem notes

Gotchas that bit the author (or are documented as likely to bite) on
each target filesystem. Updated as we learn.

## FAT32

- Max filename: 255 UTF-16 code units (LFN). ASCII-only assumptions break
  on Unicode titles — budget bytes per character.
- Path separator: `\` on Windows APIs, `/` on POSIX. Normalise to `/`
  internally; translate only at the syscall boundary.
- mtime granularity: 2 seconds. Never compare mtimes exactly; use a
  tolerance of `>= 2s` for `Verify::SizeMtime`.
- No atomic rename semantics guaranteed when overwriting. Document the
  window in `transfer::executor`; fall back to delete-then-rename.
- 4 GB per-file cap. Most hi-res FLAC is fine; WAV rips can exceed.

## exFAT

- Effectively unlimited filename length for our purposes (255 UTF-16).
- Same mtime granularity as FAT32 in practice on most implementations.
- Atomic rename: varies by driver. Do not rely on it; still use
  temp + fsync + rename but handle failure explicitly.

## ext4

- Case-sensitive by default. A DAP that mounts ext4 may differ from one
  that mounts exFAT for the same logical filename; honour
  `filesystem.case_sensitive` in the DAP profile.
- mtime granularity: ns. No workarounds needed.

## NTFS

- Reserved names (`CON`, `PRN`, `AUX`, `NUL`, `COM1..9`, `LPT1..9`) and
  trailing dot/space. Refuse to write these; log as `verify_fail` with
  a clear reason.
- Case-insensitive by default but case-preserving. Treat like exFAT.

## Windows vs WSL vs Linux-native on FAT32

The same physical microSD, mounted from each host, can report different
mtimes (timezone handling) and different filename casing behaviour. The
`Verify::SizeMtime` comparator must tolerate a ±1h offset when the
destination filesystem is FAT32 and the platform is Windows, because
WSL writes UTC and Windows writes local time on some kernel versions.
Document this in the integration test for resume.
