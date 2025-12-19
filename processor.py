import subprocess
import logging
import re
from pathlib import Path
from rich.console import Console
from config import Config
from engines import LangEngine, FileScanner

console = Console()

class MKVProcessor:
    def __init__(self):
        self.stats = {"success": 0, "skipped": 0, "failed": 0, "files_deleted": 0}
        self._setup_logging()

    def _setup_logging(self):
        logging.basicConfig(
            filename=Config.LOG_FILE,
            level=logging.INFO,
            format='%(asctime)s - %(levelname)s - %(message)s',
            filemode='a',
            encoding='utf-8'
        )

    def _get_season_tag(self, filename: str) -> str:
        """Extracts season tag (e.g., S01) using regex."""
        match = re.search(r'[sS](\d{1,2})', filename)
        return f"S{match.group(1).zfill(2)}" if match else "Other"

    def process_file(self, video_path: Path):
        """Processes a single video file and handles the merging logic."""
        try:
            # Determine Series Name and Season
            rel_path = video_path.relative_to(Config.ROOT_FOLDER)
            series_name = rel_path.parts[0] if len(rel_path.parts) > 1 else "Standalone"
            season_folder = self._get_season_tag(video_path.name)
            
            target_dir = Config.OUTPUT_ROOT / series_name / season_folder
            output_file = target_dir / f"{video_path.stem}.mkv"

            if output_file.exists():
                self.stats["skipped"] += 1
                return

            subs, audios = FileScanner.find_matching_assets(video_path)
            
            if Config.DRY_RUN:
                console.print(f"[bold yellow][DRY RUN][/bold yellow] Plan: [cyan]{series_name}/{season_folder}/{video_path.name}[/cyan]")
                return

            target_dir.mkdir(parents=True, exist_ok=True)
            
            # Build mkvmerge command
            cmd = [str(Config.MKVMERGE_PATH), "-o", str(output_file), str(video_path)]

            # Add Subtitles with Language Detection
            for sub in subs:
                lang = LangEngine.detect_subtitle_language(sub)
                is_default = "1" if lang['iso'] == Config.DEFAULT_SUB_LANG else "0"
                cmd.extend([
                    "--language", f"0:{lang['iso']}",
                    "--track-name", f"0:{lang['name']}",
                    "--default-track", f"0:{is_default}",
                    str(sub)
                ])

            # Add Audio Tracks (Defaulted to English)
            for audio in audios:
                cmd.extend(["--language", "0:eng", "--track-name", "0:English", str(audio)])

            # Execute merging process
            result = subprocess.run(cmd, capture_output=True, text=True, encoding='utf-8')

            if result.returncode <= 1:
                self.stats["success"] += 1
                logging.info(f"SUCCESS: {output_file.name}")
                if Config.DELETE_ORIGINALS:
                    self._cleanup_sources(video_path, subs, audios)
            else:
                self.stats["failed"] += 1
                logging.error(f"FAILED: {video_path.name} - Error: {result.stderr}")

        except Exception as e:
            self.stats["failed"] += 1
            logging.error(f"CRITICAL ERROR processing {video_path.name}: {str(e)}")

    def _cleanup_sources(self, video: Path, subs: list, audios: list):
        """Deletes original files after successful merge."""
        for f in [video] + subs + audios:
            try:
                if f.exists():
                    f.unlink()
                    self.stats["files_deleted"] += 1
            except Exception as e:
                logging.warning(f"Cleanup failed for {f.name}: {e}")

    def get_final_stats(self) -> dict:
        return self.stats