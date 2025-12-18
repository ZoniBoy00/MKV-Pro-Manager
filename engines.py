import logging
from pathlib import Path
from langdetect import detect, DetectorFactory
from config import Config

# Ensure consistent results from the language detector
DetectorFactory.seed = 0

class LangEngine:
    @staticmethod
    def detect_subtitle_language(file_path: Path) -> dict:
        """Analyzes subtitle content to detect the language."""
        try:
            with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
                sample_text = ""
                for line in f:
                    # Filter for actual speech (skip numbers and timestamps)
                    clean_line = ''.join(filter(str.isalpha, line))
                    if len(clean_line) > 5:
                        sample_text += line + " "
                    if len(sample_text) > 1000: # Sufficient sample size
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
    def find_matching_assets(video_path: Path):
        """Finds subtitles and audio files that share the same base name as the video."""
        base_name = video_path.stem
        parent_dir = video_path.parent
        
        subtitles = [p for p in parent_dir.glob(f"{base_name}*") if p.suffix.lower() in Config.EXT_SUB]
        audios = [p for p in parent_dir.glob(f"{base_name}*") if p.suffix.lower() in Config.EXT_AUDIO]
        
        return subtitles, audios