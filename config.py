from pathlib import Path

class Config:
    # --- PATHS ---
    ROOT_FOLDER = Path(r"C:\Users\USERNAME\Source folder")
    OUTPUT_ROOT = Path(__file__).parent / "MKV_output"
    MKVMERGE_PATH = Path(r"C:\Program Files\MKVToolNix\mkvmerge.exe")
    LOG_FILE = Path(__file__).parent / "process_log.txt"

    # --- SETTINGS ---
    DRY_RUN = False            # Set to False to actually process files
    DELETE_ORIGINALS = False  # Set to True to delete source files after success
    DEFAULT_SUB_LANG = "fin"  
    
    # --- FILE EXTENSIONS ---
    EXT_VIDEO = {".mp4", ".mkv", ".avi", ".mov"}
    EXT_SUB   = {".srt", ".ass", ".ssa", ".vtt"}
    EXT_AUDIO = {".aac", ".mp3", ".m4a", ".flac", ".wav"}

    # ISO-639-1 -> [ISO-639-2, Name]
    LANG_DATA = {
        'fi': ['fin', 'Finnish'], 'en': ['eng', 'English'], 'sv': ['swe', 'Swedish'],
        'de': ['ger', 'German'], 'fr': ['fre', 'French'], 'es': ['spa', 'Spanish'],
        'no': ['nor', 'Norwegian'], 'da': ['dan', 'Danish'], 'ru': ['rus', 'Russian'],
        'it': ['ita', 'Italian'], 'ja': ['jpn', 'Japanese'], 'zh-cn': ['chi', 'Chinese']

    }
