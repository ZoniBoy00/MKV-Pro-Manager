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
    // Prevent immediate close on panic
    std::panic::set_hook(Box::new(|info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            },
        };
        println!("\n{} {} {}", style("CRITICAL ERROR:").red().bold(), style(msg).red(), style(info.location().unwrap()).dim());
        println!("{}", style("\nPress Enter to close...").dim());
        let _ = std::io::stdin().read_line(&mut String::new());
        std::process::exit(1);
    }));

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

    // Scan Result
    let scan_msg = format!("Found {} files in {}", video_files.len(), HumanDuration(scan_time));
    println!("{} {}\n", style("SCAN COMPLETE").green().bold(), style(scan_msg).dim());

    if !config.dry_run {
        println!("{}", style("Processing will start shortly...").dim());
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }

    // 3. Process
    println!();
    let multiprogress = MultiProgress::new();
    
    // Master Progress Bar
    let pb = multiprogress.add(ProgressBar::new(video_files.len() as u64));
    pb.set_style(ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({percent}%) | ETA: {eta_precise}"
    ).unwrap().progress_chars("━╾─"));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    
    // Create a pool of "worker bars" to show what's actually happening
    let active_bars = Arc::new(Mutex::new(Vec::new()));
    for i in 0..config.concurrent_jobs {
        let job_pb = multiprogress.add(ProgressBar::new_spinner());
        job_pb.set_style(ProgressStyle::with_template("  {spinner:.dim} {msg}").unwrap());
        job_pb.set_message(style(format!("Worker {} waiting...", i + 1)).dim().to_string());
        job_pb.enable_steady_tick(std::time::Duration::from_millis(150));
        active_bars.lock().unwrap().push(job_pb);
    }

    // Stats (Success, Skipped, Failed)
    let stats = Arc::new(Mutex::new((0, 0, 0)));
    let processor = Processor::new((*config).clone());

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.concurrent_jobs)
        .build()
        .unwrap();

    pool.install(|| {
        video_files.par_iter().enumerate().for_each(|(idx, video)| {
            let worker_id = idx % config.concurrent_jobs;
            let job_pb = {
                let bars = active_bars.lock().unwrap();
                bars[worker_id].clone()
            };

            let name = video.file_name().unwrap_or_default().to_string_lossy();
            let display_name = if name.chars().count() > 35 { 
                format!("{}...", name.chars().take(32).collect::<String>()) 
            } else { 
                name.to_string() 
            };
            
            job_pb.set_style(ProgressStyle::with_template("  {spinner:.yellow} {msg}").unwrap());
            job_pb.set_message(format!("Processing: {}", style(&display_name).cyan()));

            let result = processor.process_file(video);
            
            match result {
                ProcessStatus::Success { subs, audios } => {
                    stats.lock().unwrap().0 += 1;
                    let info = if subs > 0 || audios > 0 {
                        let sub_info = if subs > 0 { format!("{} subs", subs) } else { String::new() };
                        let aud_info = if audios > 0 { format!("{} audios", audios) } else { String::new() };
                        let parts = vec![sub_info, aud_info].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
                        format!("Merged ({})", parts)
                    } else {
                        "Merged (no extra assets)".to_string()
                    };
                    let _ = multiprogress.println(format!("{} {} -> {}", SUCCESS, display_name, style(info).green()));
                },
                ProcessStatus::Skipped => {
                    stats.lock().unwrap().1 += 1;
                    let _ = multiprogress.println(format!("{} {} -> {}", SKIPPED, display_name, style("Already exists").yellow()));
                },
                ProcessStatus::Failed(e) => {
                    stats.lock().unwrap().2 += 1;
                    let _ = multiprogress.println(format!("{} {} -> {}", FAILED, display_name, style(e).red()));
                }
            }

            job_pb.set_style(ProgressStyle::with_template("  {spinner:.dim} {msg}").unwrap());
            job_pb.set_message(style(format!("Worker {} idle", worker_id + 1)).dim().to_string());
            pb.inc(1);
        });
    });

    // Cleanup worker bars
    for job_pb in active_bars.lock().unwrap().iter() {
        job_pb.finish_and_clear();
    }
    pb.finish_with_message("Done");

    // 4. Summary Panel
    let final_stats = stats.lock().unwrap();
    let (success, skipped, failed) = *final_stats;
    let total = success + skipped + failed;
    let deleted = if config.delete_originals { success } else { 0 };

    let success_pct = if total > 0 { (success as f32 / total as f32) * 100.0 } else { 0.0 };
    
    println!();
    let summary_lines = vec![
        format!("{} {:<18} {} ({:.1}%)", SUCCESS, "Successfully Merged:", style(success).green().bold(), success_pct),
        format!("{} {:<18} {}", SKIPPED, "Skipped (Exists):", style(skipped).yellow()),
        format!("{} {:<18} {}", FAILED, "Failures:", style(failed).red()),
        format!("{} {:<18} {}", TRASH, "Originals Deleted:", if deleted > 0 { style(deleted.to_string()).red().bold() } else { style("0".to_string()).dim() }),
        style("━".repeat(20)).dim().to_string(),
        format!("{} Total Time: {}", SPARKLE, style(HumanDuration(start_scan.elapsed())).cyan().bold()),
    ];

    draw_panel("PROCESSING COMPLETE", &summary_lines, |s| style(s).magenta().bold());

    println!();
    println!("{}", style("Press Enter to exit...").white().dim());
    let _ = std::io::stdin().read_line(&mut String::new());
}
