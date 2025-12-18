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
        logging.basicConfig(
            filename=Config.LOG_FILE,
            level=logging.INFO,
            format='%(asctime)s - %(levelname)s - %(message)s'
        )

    def _get_season(self, filename: str) -> str:
        """Extracts season (e.g., S01) from filename using Regex."""
        match = re.search(r'[sS](\d{1,2})', filename)
        if match:
            return f"S{match.group(1).zfill(2)}"
        return "Other"

    def process_file(self, video_path: Path):
        # 1. Determine Folder Structure
        series_name = Config.ROOT_FOLDER.name
        season_folder_name = self._get_season(video_path.name)
        
        target_dir = Config.OUTPUT_ROOT / series_name / season_folder_name
        output_file = target_dir / f"{video_path.stem}.mkv"

        if output_file.exists():
            self.stats["skipped"] += 1
            return

        # 2. Find Assets
        subs, audios = FileScanner.find_matching_assets(video_path)
        
        # 3. Handle Dry Run
        if Config.DRY_RUN:
            console.print(f"[bold yellow][DRY RUN][/bold yellow] Would create: [cyan]{series_name}/{season_folder_name}/{video_path.stem}.mkv[/cyan]")
            for s in subs:
                lang = LangEngine.detect_subtitle_language(s)
                console.print(f"   + Subtitle found: {s.name} ({lang['name']})")
            return

        # 4. Actual Execution
        target_dir.mkdir(parents=True, exist_ok=True)
        cmd = [str(Config.MKVMERGE_PATH), "-o", str(output_file), str(video_path)]

        for sub in subs:
            lang = LangEngine.detect_subtitle_language(sub)
            is_default = "1" if lang['iso'] == Config.DEFAULT_SUB_LANG else "0"
            cmd.extend(["--language", f"0:{lang['iso']}", "--track-name", f"0:{lang['name']}", "--default-track", f"0:{is_default}", str(sub)])

        for audio in audios:
            cmd.extend(["--language", "0:eng", "--track-name", "0:English", str(audio)])

        result = subprocess.run(cmd, capture_output=True, text=True)

        if result.returncode <= 1:
            self.stats["success"] += 1
            logging.info(f"Success: {output_file.name}")
            if Config.DELETE_ORIGINALS:
                self._delete_source_files(video_path, subs, audios)
        else:
            self.stats["failed"] += 1
            logging.error(f"Error merging {video_path.name}: {result.stderr}")

    def _delete_source_files(self, video, subs, audios):
        try:
            files = [video] + subs + audios
            for f in files:
                f.unlink()
                self.stats["files_deleted"] += 1
        except Exception as e:
            logging.error(f"Deletion failed: {e}")

    def get_final_stats(self):
        return self.stats