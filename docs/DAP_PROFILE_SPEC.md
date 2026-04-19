# DAP Profile Spec (v1)

A DAP profile describes a physical device's constraints and quirks so
`dapctl` can sync to it safely. This document is the contract between
the tool and the profile files in `profiles/` (builtin) or
`$XDG_CONFIG_HOME/dapctl/profiles/` (user overrides).

## Stability guarantee

`schema_version = 1` is stable for the duration of the 0.x line. Fields
may be added; none will be removed or change meaning silently. Unknown
fields are a hard error (`deny_unknown_fields`) to force schema bumps.

## Top-level structure

```toml
schema_version = 1

[dap]           # identity
[filesystem]    # storage medium constraints
[codecs]        # format support matrix
[layout]        # expected directory layout on the DAP
[exclude]       # globs that must never be written or compared
[quirks]        # device-specific behavioural notes
```

## Sections

### `[dap]`

| Field         | Type     | Required | Meaning |
|---------------|----------|----------|---------|
| `id`          | string   | yes      | slug, kebab-case, globally unique (`fiio-m21`) |
| `name`        | string   | yes      | display name |
| `vendor`      | string   | yes      | manufacturer |
| `firmware_min`| string   | no       | lowest tested firmware |
| `sources`     | array    | no       | URLs/threads backing the claims below |

### `[filesystem]`

| Field                | Type      | Meaning |
|----------------------|-----------|---------|
| `preferred`          | string    | `exFAT` / `FAT32` / `ext4` / `NTFS` |
| `supported`          | array     | any of the above |
| `max_filename_bytes` | u32       | per-path-component byte cap |
| `max_path_bytes`     | u32       | full path byte cap |
| `case_sensitive`     | bool      | how the DAP treats file names |

### `[codecs]`

| Field                | Type      | Meaning |
|----------------------|-----------|---------|
| `lossless`           | array     | `FLAC`, `ALAC`, `WAV`, `APE`, `DSF`, `DFF`, ... |
| `lossy`              | array     | `MP3`, `AAC`, `OGG`, `OPUS`, ... |
| `max_sample_rate_hz` | u32       | PCM maximum |
| `max_bit_depth`      | u32       | PCM maximum |
| `dsd`                | array     | `DSD64`, `DSD128`, `DSD256`, `DSD512` |

### `[layout]`

| Field                       | Type   | Meaning |
|-----------------------------|--------|---------|
| `music_root`                | string | path on the DAP root where music lives |
| `prefers_artist_album_tree` | bool   | used by future playlist/export heuristics |

### `[exclude]`

| Field   | Type  | Meaning |
|---------|-------|---------|
| `globs` | array | patterns inherited by every sync profile targeting this DAP |

### `[quirks]`

Free-form recommendations, not enforced. Examples:

- `warn_on_embedded_art_mb` (u32): warn if embedded cover art exceeds N MB.
- `normalize_unicode` (string: `NFC` | `NFD`): write filenames in this form.

## Validation checklist for contributors

- `id` matches the filename (`profiles/<id>.toml`).
- `sources` cites at least one third-party confirmation of `codecs`.
- `firmware_min` reflects the firmware you actually tested on.
- A fixture library under `tests/fixtures/profiles/<id>/` exercises the
  edge cases (max-length name, largest supported sample rate).
