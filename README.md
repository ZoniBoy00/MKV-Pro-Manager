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
    - Identifies subtitle languages (e.g., `.fin.srt`, `_eng.srt` or via content analysis).
    - Sets the "Default" flag for your preferred language automatically.
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

The settings are saved to `config.toml`. You can edit this file later to change settings without the wizard.

### Example Configuration (`config.toml`)
```toml
root_folder = "C:\\Downloads\\Incoming"
output_root = "D:\\Media\\Library"
mkvmerge_path = "C:\\Program Files\\MKVToolNix\\mkvmerge.exe"
concurrent_jobs = 4 # How many files to merge at once
default_sub_lang = "fin" # Preferred subtitle language (ISO 639-3)
delete_originals = false # Delete source files after success?
```

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
  - If a file with the target name already exists in the Output directory, it is skipped to prevent accidental overwrites. Delete the confirmation file or check your folders.

- **Subtitles not found**:
  - The tool uses "fingerprinting" (alphanumeric matching). Ensure the subtitle filename contains at least the base name of the video file.

---

## 📝 License

MIT License. Free to use, modify, and distribute.
