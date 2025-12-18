import sys
from rich.console import Console
from rich.table import Table
from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn, TaskProgressColumn, TimeRemainingColumn
from rich.panel import Panel

# Import modular components
from config import Config
from processor import MKVProcessor

console = Console()

def run_application():
    console.print(Panel(
        "[bold cyan]MKV PRO MANAGER[/bold cyan]\n[white]Modular Engine v1.0[/white]", 
        subtitle="Universal Language Detection & Auto-Merge",
        expand=False
    ))

    # Verification
    if not Config.MKVMERGE_PATH.exists():
        console.print("[bold red]ERROR:[/bold red] mkvmerge.exe not found. Please update [bold]config.py[/bold]")
        return

    video_files = [p for p in Config.ROOT_FOLDER.rglob("*") if p.suffix.lower() in Config.EXT_VIDEO]
    
    if not video_files:
        console.print("[yellow]Notice: No video files found in the source directory.[/yellow]")
        return

    processor = MKVProcessor()

    # Progress Tracking
    with Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        BarColumn(bar_width=40),
        TaskProgressColumn(),
        TimeRemainingColumn(),
        console=console
    ) as progress:
        
        main_task = progress.add_task(f"[white]Merging {len(video_files)} videos...", total=len(video_files))

        for video in video_files:
            processor.process_file(video)
            progress.advance(main_task)

    # Display Summary Table
    results = processor.get_final_stats()
    summary_table = Table(title="Process Summary", header_style="bold cyan", box=None)
    summary_table.add_column("Metric", style="white")
    summary_table.add_column("Result", justify="right")

    summary_table.add_row("Successfully Merged", str(results["success"]), style="green")
    summary_table.add_row("Skipped (Existing)", str(results["skipped"]), style="yellow")
    summary_table.add_row("Failed / Errors", str(results["failed"]), style="red")
    
    if Config.DELETE_ORIGINALS:
        summary_table.add_row("Original Files Deleted", str(results["files_deleted"]), style="magenta")

    console.print("\n", summary_table)
    console.print(f"[bold green]Process Complete![/bold green] Logs saved to: [dim]{Config.LOG_FILE}[/dim]\n")

if __name__ == "__main__":
    try:
        run_application()
    except KeyboardInterrupt:
        console.print("\n[bold red]Process aborted by user.[/bold red]")
        sys.exit(0)