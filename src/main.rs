mod config;
mod lang;
mod processor;
mod scanner;

use std::path::PathBuf;
use std::time::Instant;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle, HumanDuration};
use console::{style, Emoji, Term};
use rayon::prelude::*;
use crate::config::load_config_interactive;
use crate::processor::{Processor, ProcessStatus};

// --- THEME & CONSTANTS ---
static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", "* ");
static ROCKET: Emoji<'_, '_> = Emoji("🚀 ", "> ");
static FOLDER: Emoji<'_, '_> = Emoji("📂 ", "");
static GEAR:    Emoji<'_, '_> = Emoji("⚙️  ", "");
static SUCCESS: Emoji<'_, '_> = Emoji("✅ ", "+");
static SKIPPED: Emoji<'_, '_> = Emoji("⏭️  ", "~");
static FAILED:  Emoji<'_, '_> = Emoji("❌ ", "x");
static TRASH:   Emoji<'_, '_> = Emoji("🗑️  ", "");

/// Draws a stylish box with a title and content lines
fn draw_panel(title: &str, content: &[String], color_func: fn(&str) -> console::StyledObject<&str>) {
    let term = Term::stdout();
    let (_, width) = term.size();
    let width = (width as usize).min(80); // Cap width for readability
    
    // Top Border
    let horiz = "━".repeat(width - 2);
    println!("{}", color_func(&format!("╭{}╮", horiz)));
    
    // Title Line
    let title_line = format!("│{:^width$}│", title, width = width - 2);
    println!("{}", color_func(&title_line));
    
    // Separator
    println!("{}", color_func(&format!("├{}┤", horiz)));

    // Content
    let inner_width = width.saturating_sub(4); // width - (2 borders + 2 spaces)
    for line in content {
        // pad_str handles visual width, ANSI codes, and truncation in one call
        let processed_line = console::pad_str(
            line, 
            inner_width, 
            console::Alignment::Left, 
            Some("...")
        );
        println!("{}", color_func(&format!("│ {} │", processed_line)));
    }
    
    // Bottom
    println!("{}", color_func(&format!("╰{}╯", horiz)));
}

fn main() {
    let term = Term::stdout();
    term.clear_screen().ok();
    
    // 1. Config & Setup
    let config = Arc::new(load_config_interactive());
    
    // UI Header
    let version = env!("CARGO_PKG_VERSION");
    let title = format!("MKV PRO MANAGER V{}", version);
    
    let info_lines = vec![
        format!("{} Source: {}", FOLDER, style(config.root_folder.display()).cyan()),
        format!("{} Output: {}", FOLDER, style(config.output_root.display()).cyan()),
        format!("{} Custom: {}", GEAR,   if !config.mkvmerge_path.exists() { style("Missing MKVMerge!").red() } else { style("Ready").green() }),
        format!("{} Mode:   {}", ROCKET, if config.dry_run { style("DRY RUN").yellow().bold() } else { style("PRODUCTION").green().bold() }),
    ];

    draw_panel(&title, &info_lines, |s| style(s).magenta().bold());

    if !config.mkvmerge_path.exists() {
        println!("\n{} {}", FAILED, style("Critical Error: mkvmerge.exe not found.").red());
        return;
    }

    // 2. Scan
    println!("\n{} {}", style("SCANNING LIBRARY...").bold(), style("Please wait").dim());
    let start_scan = Instant::now();

    let video_extensions = &config.ext_video;
    let video_files: Vec<PathBuf> = WalkDir::new(&config.root_folder)
        .into_iter()
        .par_bridge()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| {
            if let Some(ext) = e.path().extension().and_then(|e| e.to_str()) {
                let fmt = format!(".{}", ext.to_ascii_lowercase());
                video_extensions.contains(&fmt)
            } else {
                false
            }
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let scan_time = start_scan.elapsed();

    if video_files.is_empty() {
        println!("\n{} {}", style("NO FILES FOUND").yellow().bold(), "Check your source directory.");
        println!("{}", style("Press Enter to exit...").white().dim());
        let _ = std::io::stdin().read_line(&mut String::new());
        return;
    }

    // Scan Result Panel
    println!("{}", style(format!("Found {} files in {}", video_files.len(), HumanDuration(scan_time))).green());

    if !config.dry_run && !config.delete_originals {
        println!("{}", style("\nStarting processing in 3 seconds...").dim());
        std::thread::sleep(std::time::Duration::from_secs(3));
    }

    // 3. Process
    println!();
    let multiprogress = MultiProgress::new();
    
    // We strive for a look like: 
    // [██████████████------------] 50/100 (1m 30s)
    let pb = multiprogress.add(ProgressBar::new(video_files.len() as u64));
    pb.set_style(ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos:>4}/{len:4} {msg}"
    ).unwrap().progress_chars("█▓▒░  "));
    
    // Stats (Success, Skipped, Failed)
    let stats = Arc::new(Mutex::new((0, 0, 0)));
    let processor = Processor::new((*config).clone());

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.concurrent_jobs)
        .build()
        .unwrap();

    pool.install(|| {
        video_files.par_iter().for_each(|video| {
           let name = video.file_name().unwrap_or_default().to_string_lossy();
           // Just show filename, truncate if too long
           let display_name = if name.len() > 30 { format!("{}...", &name[..27]) } else { name.to_string() };
           
           pb.set_message(format!("Activity: {}", display_name));

           match processor.process_file(video) {
               ProcessStatus::Success { subs, audios } => {
                   stats.lock().unwrap().0 += 1;
                   if subs == 0 && audios == 0 {
                       let _ = multiprogress.println(format!("{} {} -> {}", style("⚠️").yellow(), display_name, style("Merged without extra assets").dim()));
                   } else {
                       let sub_info = if subs > 0 { format!("{} subs", subs) } else { String::new() };
                       let aud_info = if audios > 0 { format!("{} audios", audios) } else { String::new() };
                       let info = vec![sub_info, aud_info].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
                       let _ = multiprogress.println(format!("{} {} -> {}", SUCCESS, display_name, style(format!("Merged ({})", info)).green()));
                   }
               },
               ProcessStatus::Skipped => {
                   stats.lock().unwrap().1 += 1;
               },
               ProcessStatus::Failed(e) => {
                   stats.lock().unwrap().2 += 1;
                   let _ = multiprogress.println(format!("{} {} -> {}", FAILED, display_name, style(e).red()));
               }
           }
           pb.inc(1);
        });
    });

    pb.finish_with_message("Done");

    // 4. Summary Panel
    let final_stats = stats.lock().unwrap();
    let (success, skipped, failed) = *final_stats;
    let deleted = if config.delete_originals { success } else { 0 };

    println!();
    let summary_lines = vec![
        format!("{} {:<18} {}", SUCCESS, "Successfully Merged:", style(success).green().bold()),
        format!("{} {:<18} {}", SKIPPED, "Skipped (Exists):", style(skipped).yellow()),
        format!("{} {:<18} {}", FAILED, "Failures:", style(failed).red()),
        format!("{} {:<18} {}", TRASH, "Originals Deleted:", if deleted > 0 { style(deleted.to_string()).red().bold() } else { style("0".to_string()).dim() }),
        String::new(),
        format!("{} Total Time: {}", SPARKLE, HumanDuration(start_scan.elapsed())),
    ];

    draw_panel("PROCESSING SUMMARY", &summary_lines, |s| style(s).cyan());

    println!();
    println!("{}", style("Press Enter to exit...").white().dim());
    let _ = std::io::stdin().read_line(&mut String::new());
}
