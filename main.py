import sys
import time
from pathlib import Path
from rich.console import Console
from rich.table import Table
from rich.progress import Progress, SpinnerColumn, TextColumn, BarColumn, TaskProgressColumn, TimeRemainingColumn
from rich.panel import Panel

# Import modular components
from config import Config
from processor import MKVProcessor

console = Console()

def display_config_info():
    """Displays a clean summary of current settings before starting."""
    table = Table(box=None, show_header=False)
    table.add_row("Source:", f"[cyan]{Config.ROOT_FOLDER}[/cyan]")
    table.add_row("Output:", f"[cyan]{Config.OUTPUT_ROOT}[/cyan]")
    
    dry_run_status = "[bold yellow]Enabled[/bold yellow]" if Config.DRY_RUN else "[bold green]Disabled[/bold green]"
    cleanup_status = "[bold red]On[/bold red]" if Config.DELETE_ORIGINALS else "[bold green]Off[/bold green]"
    
    table.add_row("Dry Run:", dry_run_status)
    table.add_row("Cleanup:", cleanup_status)
    table.add_row("Default Sub:", f"[bold white]{Config.DEFAULT_SUB_LANG.upper()}[/bold white]")

    console.print(Panel(
        table, 
        title="[bold magenta]Configuration Settings[/bold magenta]", 
        border_style="magenta",
        padding=(1, 2)
    ))

def run_application():
    # Header UI
    console.print("\n")
    console.print(Panel(
        "[bold cyan]🎬 MKV PRO MANAGER[/bold cyan]\n[dim]Automated Multi-Track Merging Engine[/dim]", 
        subtitle="[white]v1.0.1[/white]",
        border_style="cyan",
        expand=False
    ))

    # Path Verification
    if not Config.MKVMERGE_PATH.exists():
        console.print(f"[bold red]CRITICAL ERROR:[/bold red] mkvmerge.exe not found at:\n[dim]{Config.MKVMERGE_PATH}[/dim]")
        return

    # Scan for files
    video_files = [p for p in Config.ROOT_FOLDER.rglob("*") if p.suffix.lower() in Config.EXT_VIDEO]
    
    if not video_files:
        console.print(Panel("[bold yellow]No video files found in the source directory.[/bold yellow]", border_style="yellow"))
        return

    display_config_info()
    
    if not Config.DRY_RUN and not Config.DELETE_ORIGINALS:
        console.print("[dim]Starting process in 3 seconds... (Ctrl+C to abort)[/dim]")
        time.sleep(3)

    processor = MKVProcessor()

    # Fixed Progress Bar (Removed 'gradient' for compatibility)
    with Progress(
        SpinnerColumn(spinner_name="dots"),
        TextColumn("[bold blue]{task.fields[filename]}"),
        BarColumn(bar_width=None, style="black on cyan", complete_style="bold green"),
        TaskProgressColumn(),
        TimeRemainingColumn(),
        console=console,
        transient=True
    ) as progress:
        
        main_task = progress.add_task(
            "Processing", 
            total=len(video_files),
            filename=f"Preparing {len(video_files)} items"
        )

        for video in video_files:
            # Update UI with current file
            display_name = (video.name[:27] + '...') if len(video.name) > 30 else video.name
            progress.update(main_task, filename=f"Merging: {display_name}")
            
            processor.process_file(video)
            progress.advance(main_task)

    # Final Report UI
    results = processor.get_final_stats()
    
    summary_table = Table(
        title="\n[bold cyan]PROCESSED SUMMARY[/bold cyan]", 
        header_style="bold white", 
        box=None,
        show_edge=False
    )
    summary_table.add_column("Category", style="dim")
    summary_table.add_column("Count", justify="right", style="bold")

    summary_table.add_row("Successfully Merged", str(results["success"]), style="green")
    summary_table.add_row("Skipped (Exists)", str(results["skipped"]), style="yellow")
    summary_table.add_row("Errors / Failed", str(results["failed"]), style="red")
    
    if Config.DELETE_ORIGINALS:
        summary_table.add_row("Cleanup (Deleted)", str(results["files_deleted"]), style="bright_red")

    console.print(Panel(summary_table, border_style="green", padding=(1, 4)))
    console.print(f"\n[bold green]✔ Done![/bold green] Logs: [underline]{Config.LOG_FILE}[/underline]\n")

if __name__ == "__main__":
    try:
        run_application()
    except KeyboardInterrupt:
        console.print("\n[bold red]🛑 Process manually stopped.[/bold red]\n")
        sys.exit(0)