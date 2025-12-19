import logging
import re
from pathlib import Path
from langdetect import detect, DetectorFactory
from config import Config

# Ensure language detection is consistent
DetectorFactory.seed = 0

class LangEngine:
    @staticmethod
    def detect_subtitle_language(file_path: Path) -> dict:
        """Analyzes subtitle filename or content to detect the language."""
        try:
            name_lower = file_path.name.lower()
            
            # Priority 1: Check filename for language tags (StreamFab style: .fi.subtitles.srt)
            for iso1, info in Config.LANG_DATA.items():
                patterns = [f".{iso1}.", f"_{iso1}.", f".{info[0]}.", f"_{info[0]}."]
                if any(p in name_lower for p in patterns):
                    return {"iso": info[0], "name": info[1]}

            # Priority 2: Content analysis
            with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
                sample_text = ""
                for line in f:
                    clean_line = ''.join(filter(str.isalpha, line))
                    if len(clean_line) > 5:
                        sample_text += line + " "
                    if len(sample_text) > 1500:
                        break
                
                if not sample_text.strip():
                    return {"iso": "und", "name": "Undefined"}
                
                detected_iso = detect(sample_text)
                lang_info = Config.LANG_DATA.get(detected_iso, ["und", "Unknown"])
                return {"iso": lang_info[0], "name": lang_info[1]}
        except Exception as e:
            logging.error(f"Language detection failed for {file_path.name}: {e}")
            return {"iso": "und", "name": "Undefined"}

class FileScanner:
    @staticmethod
    def _get_fingerprint(text: str) -> str:
        """Creates a normalized string for comparison by removing all non-alphanumerics."""
        return re.sub(r'[^a-zA-Z0-9]', '', text).lower()

    @staticmethod
    def find_matching_assets(video_path: Path):
        """Finds subtitles and audio files using a robust fingerprint matching logic."""
        # Use stem for video but name for potential assets to avoid partial stem cuts
        video_fingerprint = FileScanner._get_fingerprint(video_path.stem)
        parent_dir = video_path.parent
        
        subtitles = []
        audios = []
        
        try:
            # List all items in the same directory as the video
            for item in parent_dir.iterdir():
                # Skip directories and the video file itself
                if not item.is_file() or item.name == video_path.name:
                    continue
                
                item_ext = item.suffix.lower()
                # We sanitize the full filename of the potential asset
                item_fingerprint = FileScanner._get_fingerprint(item.name)

                # Matching: The video's name fingerprint must be inside the asset's name fingerprint
                # Example: 'theblacklists05e01' is in 'theblacklists05e01fisubtitlessrt'
                if video_fingerprint in item_fingerprint:
                    if item_ext in Config.EXT_SUB:
                        subtitles.append(item)
                    elif item_ext in Config.EXT_AUDIO:
                        audios.append(item)
                        
        except Exception as e:
            logging.error(f"Error scanning directory {parent_dir}: {e}")
        
        return subtitles, audios