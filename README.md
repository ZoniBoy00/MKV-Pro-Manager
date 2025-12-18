# 🎬 MKV Pro Manager v1.0

Universal modular system for automated MKV merging and language detection.

MKV Pro Manager is a Python tool for merging video files with matching subtitles and audio tracks. It automatically organizes files into season folders for TV series, but can also handle standalone movies or videos.

---

## 🚀 Key Features

* **Modular Architecture**: Clear separation into config, processor, and engine modules.
* **Smart Folder Organization**: Automatically creates series and season folders (e.g., `The Blacklist/S01/`). Works for TV series or standalone videos.
* **Subtitle Language Detection**: Uses `langdetect` to detect subtitle language and apply proper ISO-639-2 tags (fin, eng, etc.).
* **Audio Tagging**: Sets English (`eng`) for audio tracks, customizable.
* **Dry Run Mode**: Preview folder structure and operations without modifying files.
* **Progress & Logging**: Real-time progress bars and detailed logs in `process_log.txt`.
* **Optional Cleanup**: Deletes original files after successful merging if enabled.

---

## 📂 Project Structure

| File               | Description                                     |
| ------------------ | ----------------------------------------------- |
| `config.py`        | Paths, options, and language definitions.       |
| `engines.py`       | Language detection and file scanning.           |
| `processor.py`     | Handles mkvmerge operations and file deletion.  |
| `main.py`          | Launches the app and handles progress tracking. |
| `requirements.txt` | Required Python libraries.                      |

---

## 🛠 Installation

### Prerequisites

* Python 3.8+
* MKVToolNix ([Download](https://mkvtoolnix.download/downloads.html))

### Install Dependencies

```bash
pip install -r requirements.txt
```

### Configuration

Update `config.py` with:

* **ROOT_FOLDER**: Source folder with video files.
* **OUTPUT_ROOT**: Destination root folder for merged MKVs.
* **MKVMERGE_PATH**: Path to `mkvmerge.exe`.
* **DRY_RUN**: True to test, False to execute.
* **DELETE_ORIGINALS**: True to remove source files after success.
* **DEFAULT_SUB_LANG**: Default subtitle language (`fin`).

---

## 📖 Usage

```bash
python main.py
```

Supports automatic season detection based on filenames:

* `Series.S01E01.mp4` → `Series/S01/Series.S01E01.mkv`
* `Movie_Title.mp4` → `Movie_Title.mkv` in output folder

---

## 🛡️ Safety & Logging

* Originals are only deleted after successful merges.
* Existing files in the output folder are skipped.
* Errors and status are logged in `process_log.txt`.

---

## 📜 License

[MIT License](https://github.com/ZoniBoy00/MKV-Pro-Manager/blob/main/LICENSE)
