# 🎬 MKV Pro Manager (Rust Edition)

> **The ultimate tool for automated video, subtitle, and audio merging.**
>
> 🚀 *High Performance* | 🛡️ *Type Safe* | ⚡ *Multi-threaded*

MKV Pro Manager is a high-performance command-line utility designed to automagically scan, match, and merge video files with separate subtitle and audio tracks. It intelligently organizes your media library into a clean, standardized structure for Plex/Jellyfin/Emby or just local storage.

---

## ✨ Key Features

- **🚀 Blazing Fast**: Written in pure Rust with parallel processing (Rayon) for maximum speed.
- **🧠 Smart Detection**:
    - **Series**: Automatically detects `S01E01`, `1x01`, `Season 1` patterns.
    - **Movies**: Identifies movies vs shows based on year tags (e.g., `(2023)`).
    - **Intelligent Cleaning**: Removes dots, underscores, and garbage text from filenames.
    - **Fingerprint Matching**: Finds subtitles even if filenames aren't perfect matches.
- **🌍 Auto Language Detection**:
    - Identifies subtitle languages (e.g., `.fin.srt`, `_eng.srt`, `2_English.srt` or via content analysis).
    - Sets the "Default" flag on the first regular (non-forced, non-SDH) subtitle in your preferred language.
    - Understands `forced` and `SDH` tags and sets the matching mkvmerge flags.
    - Detects legacy subtitle encodings (e.g. Windows-1252 Finnish) and tells mkvmerge, so `ä`/`ö` survive.
- **🛡️ Safe by default**:
    - Output is written to a `.part` file and renamed only when finished, so an interrupted run never leaves a broken file behind.
    - With `delete_originals`, the originals are deleted only after the new file is verified to contain all merged tracks.
    - `--dry-run` shows the planned output files without touching anything.
- **🎨 Beautiful UI**:
    - Modern, animated terminal dashboard with emojis and progress bars.
    - Interactive setup wizard for first-time use.
- **📦 Single Binary**: A standalone `.exe` file. No Python, no dependencies, no installation.

---

## 📥 Installation

### Option 1: Download Release (Recommended)
1.  Go to the **[Releases](https://github.com/ZoniBoy00/MKV-Pro-Manager/releases/latest)** page.
2.  Download the latest `mkv_pro_manager.exe`.
3.  Place it anywhere you like (e.g., a dedicated folder).
4.  **Prerequisite**: Ensure you have [MKVToolNix](https://mkvtoolnix.download/) installed. The tool needs `mkvmerge.exe` to function.

### Option 2: Build from Source
If you prefer to build it yourself, you need **Rust** installed.

```bash
git clone https://github.com/ZoniBoy00/MKV-Pro-Manager
cd MKV-Pro-Manager
cargo build --release
```
The binary will be located at `target/release/mkv_pro_manager.exe`.

---

## 🚀 Usage

1.  **Double-click** `mkv_pro_manager.exe`.
2.  **First Run Setup**: The tool will launch an interactive wizard to configure:
    - **Source Directory**: Where your unorganized files are.
    - **Output Directory**: Where organized files should go.
    - **mkvmerge Path**: Location of `mkvmerge.exe`.
    - **Preferences**: Language, concurrency, cleanup options.

The settings are saved to `config.toml` (next to the exe, or in the working directory if one exists there). You can edit this file later to change settings without the wizard. Every setting is optional; missing ones use the defaults. Relative folders are relative to the config file. Failures are appended to `mkv_pro_manager.log` next to it.

### Command line
```
mkv_pro_manager [--config <PATH>] [--dry-run] [--yes] [--help] [--version]
```
- `--config <PATH>`: use another config file (required to be present; the wizard is skipped).
- `--dry-run`: show what would be created without writing anything.
- `--yes` / `-y`: no pauses and no "Press Enter" (for scripts). The exit code is 1 if any file failed.

### Example Configuration (`config.toml`)
```toml
root_folder = "C:\\Downloads\\Incoming"
output_root = "D:\\Media\\Library"
mkvmerge_path = "C:\\Program Files\\MKVToolNix\\mkvmerge.exe" # also searched in PATH
concurrent_jobs = 4 # How many files to merge at once
default_sub_lang = "fin" # Preferred subtitle language (fi, fin, Finnish all work)
delete_originals = false # Delete source files after a verified merge?
dry_run = false
```
Optional: `ext_video`, `ext_sub` and `ext_audio` (lists such as `[".srt", ".ass"]`) change which file types are picked up.

---

## 📂 How It Organizes

The tool automatically sorts content into the following structure:

**TV Shows:**
```
Output/
  └── TV Shows/
      └── Breaking Bad/
          └── Season 01/
              └── Breaking Bad S01E01.mkv
```

**Movies:**
```
Output/
  └── Movies/
      └── Inception (2010)/
          └── Inception.mkv
```

---

## ❓ Troubleshooting

- **"mkvmerge not found"**:
  - Make sure you have installed MKVToolNix.
  - Check `config.toml` and ensure `mkvmerge_path` points to the correct executable.

- **Files match but are skipped**:
  - If a file with the target name already exists in the Output directory, it is skipped to prevent accidental overwrites. Delete or move the existing file if you want to redo it. Unfinished files end in `.part` and are never mistaken for finished ones.

- **Subtitles not found**:
  - Names are compared after release tags (`1080p`, `BluRay`, `x264-GROUP`, ...) and the language suffix are removed, so the title (and year, or `SxxExx`) must be equal: `Alien.1979` does not match `Aliens.1986`.
  - Subtitles in `Subs/<video name>/` are found. Generic names like `Subs/2_English.srt` are used only when the folder holds exactly one video.

---

## 📝 License

MIT License. Free to use, modify, and distribute.
