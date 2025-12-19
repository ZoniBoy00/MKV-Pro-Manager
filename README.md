# 🎬 MKV Pro Manager v1.0.1

Universal modular system for automated MKV merging and language detection.

**MKV Pro Manager** is a Python tool for merging video files with matching subtitles and audio tracks. It is specifically optimized for content from services like StreamFab, automatically organizing files into season folders and matching tracks even with complex filenames.

---

## 🚀 Key Features

**Modular Architecture**
Clear separation into `config`, `processor`, and `engine` modules.

**Fingerprint Matching (NEW)**
Uses a robust alphanumeric *fingerprint* logic to match subtitles to videos, ignoring dots, underscores, and special characters.

**Smart Folder Organization**
Automatically detects series and season patterns (e.g. `S01E01`) and routes content to the correct output location.

**Series output**
Detected TV series are placed directly under the main output directory:

```
/MKV-Pro-Manager/MKV_output/
└── Example_Series/
    └── S01/
```

**Standalone / Other content**
Movies and non-series files (no season/episode pattern detected) are routed to a separate folder:

```
/MKV-Pro-Manager/MKV_output/Standalone/Other
```

**Hybrid Language Detection**

* **Filename Analysis**: Prioritizes standard tags like `.fi.` and `.en.`
* **Content Analysis**: Uses `langdetect` as a fallback to verify language from subtitle text

**Audio Tagging**
Sets proper ISO-639-2 language tags.

**Dry Run Mode**
Preview the entire plan and matching results without modifying any files.

**Progress & Logging**
Real-time progress bars and persistent logging in `process_log.txt`.

**Optional Cleanup**
Safely deletes original source files *only after* a successful merge.

---

## 📂 Project Structure

| File               | Description                                             |
| ------------------ | ------------------------------------------------------- |
| `config.py`        | Paths, options, and language definitions                |
| `engines.py`       | Logic core: fingerprint scanning and language detection |
| `processor.py`     | Handles `mkvmerge` operations and file cleanup          |
| `main.py`          | UI, progress tracking, and orchestration                |
| `requirements.txt` | Required Python libraries (`langdetect`, `rich`)        |

---

## 🛠 Installation

### Prerequisites

* Python 3.8+
* MKVToolNix (ensure `mkvmerge` is accessible in PATH or via full path)

### Install Dependencies

```bash
pip install -r requirements.txt
```

---

## ⚙️ Configuration

Update `config.py` with your environment details:

* `ROOT_FOLDER` – Source directory containing your videos and subtitles.
  The input folder can be located anywhere, as long as it contains the video file and its corresponding subtitles.

  Example:

  ```
  /MKV-Pro-Manager/input
  ```

* `OUTPUT_ROOT` – Destination for organized MKV files
  Example:

  ```
  /MKV-Pro-Manager/MKV_output
  ```

* `MKVMERGE_PATH` – Full path to `mkvmerge` binary

* `DRY_RUN` – Set to `False` when ready to process files

* `DELETE_ORIGINALS` – Enable cleanup after successful merging

---

## 📖 Usage

Run the main script:

```bash
python main.py
```

---

## 🧠 Matching Logic Example

The system handles complex naming conventions automatically:

* **Video**:
  `Example.Series.S01E01.Pilot.mp4`

* **Subtitle**:
  `Example_Series_S01E01_Pilot.fi.subtitles.srt`

* **Result**:
  Matched via fingerprint:

```
exampleseriess01e01pilot
```

Tagged correctly as **Finnish**.

---

## 🛡️ Safety & Logging

* **Verification**: Original files are deleted only if `mkvmerge` returns a success code
* **No Overwrites**: Skips processing if the target MKV already exists
* **Detailed Logs**: Full history available in `process_log.txt`

---

## 📜 License

MIT License
