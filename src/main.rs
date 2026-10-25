use console::{style, Emoji, Term};
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
use mkv_pro_manager::config::{fail, find_mkvmerge, load_config, parse_args, USAGE};
use mkv_pro_manager::processor::{ProcessStatus, Processor, CANCELLED};
use rayon::prelude::*;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

// --- THEME & CONSTANTS ---
static SPARKLE: Emoji<'_, '_> = Emoji("✨ ", "* ");
static ROCKET: Emoji<'_, '_> = Emoji("🚀 ", "> ");
static FOLDER: Emoji<'_, '_> = Emoji("📂 ", "");
static GEAR: Emoji<'_, '_> = Emoji("⚙️  ", "");
static SUCCESS: Emoji<'_, '_> = Emoji("✅ ", "+");
static SKIPPED: Emoji<'_, '_> = Emoji("⏭️  ", "~");
static FAILED: Emoji<'_, '_> = Emoji("❌ ", "x");
static TRASH: Emoji<'_, '_> = Emoji("🗑️  ", "");
static PREVIEW: Emoji<'_, '_> = Emoji("👀 ", "? ");

/// Appends failures and notes to a log file next to the config.
struct Log(Mutex<Option<File>>);

impl Log {
    fn open(path: &Path) -> Self {
        Log(Mutex::new(
            OpenOptions::new().create(true).append(true).open(path).ok(),
        ))
    }

    fn write(&self, level: &str, file: &str, message: &str) {
        if let Some(log) = self.0.lock().unwrap().as_mut() {
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let _ = writeln!(log, "{secs} [{level}] {file}: {message}");
        }
    }
}

/// Waits for Enter unless running non-interactively.
fn pause(yes: bool) {
    if !yes {
        println!("{}", style("Press Enter to exit...").white().dim());
        let _ = std::io::stdin().read_line(&mut String::new());
    }
}

/// Prints above the progress bars; indicatif hides these lines when stdout is
/// not a terminal, so fall back to plain printing there.
fn say(multi: &MultiProgress, line: String) {
    if Term::stdout().is_term() {
        let _ = multi.println(line);
    } else {
        println!("{line}");
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// Draws a stylish box with a title and content lines
fn draw_panel(
    title: &str,
    content: &[String],
    color_func: fn(&str) -> console::StyledObject<&str>,
) {
    let term = Term::stdout();
    let (_, width) = term.size();
    let width = (width as usize).clamp(24, 80); // Cap width for readability

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
        let processed_line =
            console::pad_str(line, inner_width, console::Alignment::Left, Some("..."));
        println!("{}", color_func(&format!("│ {} │", processed_line)));
    }

    // Bottom
    println!("{}", color_func(&format!("╰{}╯", horiz)));
}

fn main() {
    let cli = match parse_args(std::env::args().skip(1)) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    if cli.help {
        println!("{USAGE}");
        return;
    }
    if cli.version {
        println!("mkv_pro_manager {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // Prevent immediate close on panic
    let yes = cli.yes;
    std::panic::set_hook(Box::new(move |info| {
        let msg = match info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            },
        };
        println!(
            "\n{} {} {}",
            style("CRITICAL ERROR:").red().bold(),
            style(msg).red(),
            style(info.location().unwrap()).dim()
        );
        if !yes {
            println!("{}", style("\nPress Enter to close...").dim());
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        std::process::exit(1);
    }));

    let term = Term::stdout();
    term.clear_screen().ok();

    // 1. Config & Setup
    let (mut config, config_path) = load_config(&cli);
    let log = Arc::new(Log::open(&config_path.with_file_name("mkv_pro_manager.log")));

    let mkvmerge = find_mkvmerge(&config.mkvmerge_path);
    if let Some(found) = &mkvmerge {
        config.mkvmerge_path = found.clone();
    }
    let config = Arc::new(config);

    // Ctrl-C: stop starting new files; running mkvmerge children receive the
    // same console event and exit, and their .part files are cleaned up.
    let _ = ctrlc::set_handler(|| {
        CANCELLED.store(true, Ordering::SeqCst);
        println!("\nCancelling...");
    });

    // UI Header
    let version = env!("CARGO_PKG_VERSION");
    let title = format!("MKV PRO MANAGER V{}", version);

    let info_lines = vec![
        format!(
            "{} Source: {}",
            FOLDER,
            style(config.root_folder.display()).cyan()
        ),
        format!(
            "{} Output: {}",
            FOLDER,
            style(config.output_root.display()).cyan()
        ),
        format!(
            "{} Custom: {}",
            GEAR,
            if mkvmerge.is_none() {
                style("Missing MKVMerge!").red()
            } else {
                style("Ready").green()
            }
        ),
        format!(
            "{} Mode:   {}",
            ROCKET,
            if config.dry_run {
                style("DRY RUN").yellow().bold()
            } else {
                style("PRODUCTION").green().bold()
            }
        ),
    ];

    draw_panel(&title, &info_lines, |s| style(s).magenta().bold());

    if mkvmerge.is_none() {
        println!(
            "\n{} {}",
            FAILED,
            style("Critical Error: mkvmerge.exe not found.").red()
        );
        println!(
            "{}",
            style("Install MKVToolNix or set mkvmerge_path in config.toml.").dim()
        );
        pause(cli.yes);
        std::process::exit(1);
    }

    if !config.root_folder.is_dir() {
        fail(format!(
            "Source folder not found: {}",
            config.root_folder.display()
        ));
    }

    // 2. Scan
    println!(
        "\n{} {}",
        style("SCANNING LIBRARY...").bold(),
        style("Please wait").dim()
    );
    let start_scan = Instant::now();

    // Never rescan our own output when it lives inside the source folder.
    let output_abs = std::path::absolute(&config.output_root).unwrap_or_default();
    let mut video_files: Vec<PathBuf> = WalkDir::new(&config.root_folder)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            !(e.file_type().is_dir()
                && std::path::absolute(e.path()).is_ok_and(|p| same_path(&p, &output_abs)))
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|ext| {
                    config.ext_video.contains(&format!(".{}", ext.to_ascii_lowercase()))
                })
        })
        .map(|e| e.into_path())
        .collect();
    video_files.sort();

    let scan_time = start_scan.elapsed();

    if video_files.is_empty() {
        println!(
            "\n{} Check your source directory.",
            style("NO FILES FOUND").yellow().bold()
        );
        pause(cli.yes);
        return;
    }

    // Scan Result
    let scan_msg = format!(
        "Found {} files in {}",
        video_files.len(),
        HumanDuration(scan_time)
    );
    println!(
        "{} {}\n",
        style("SCAN COMPLETE").green().bold(),
        style(scan_msg).dim()
    );

    if !config.dry_run && !cli.yes {
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
    let worker_bars: Vec<ProgressBar> = (0..config.concurrent_jobs)
        .map(|i| {
            let job_pb = multiprogress.add(ProgressBar::new_spinner());
            job_pb.set_style(ProgressStyle::with_template("  {spinner:.dim} {msg}").unwrap());
            job_pb.set_message(style(format!("Worker {} waiting...", i + 1)).dim().to_string());
            job_pb.enable_steady_tick(std::time::Duration::from_millis(150));
            job_pb
        })
        .collect();

    // Stats: success, dry-run, skipped, failed, originals deleted
    let stats = Mutex::new([0usize; 5]);
    let processor = Processor::new((*config).clone());

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.concurrent_jobs)
        .build()
        .unwrap();

    pool.install(|| {
        video_files.par_iter().for_each(|video| {
            // The thread that actually runs this file owns the matching bar.
            let worker_id = rayon::current_thread_index().unwrap_or(0) % worker_bars.len();
            let job_pb = &worker_bars[worker_id];

            let name = video.file_name().unwrap_or_default().to_string_lossy();
            let display_name = if name.chars().count() > 35 {
                format!("{}...", name.chars().take(32).collect::<String>())
            } else {
                name.to_string()
            };

            job_pb.set_style(ProgressStyle::with_template("  {spinner:.yellow} {msg}").unwrap());
            job_pb.set_message(format!("Processing: {}", style(&display_name).cyan()));

            let counts = |subs: usize, audios: usize| {
                let parts: Vec<String> = [(subs, "subs"), (audios, "audios")]
                    .into_iter()
                    .filter(|(n, _)| *n > 0)
                    .map(|(n, what)| format!("{n} {what}"))
                    .collect();
                if parts.is_empty() {
                    "no extra assets".to_string()
                } else {
                    parts.join(", ")
                }
            };

            match processor.process_file(video) {
                ProcessStatus::Success { subs, audios, deleted, note } => {
                    {
                        let mut s = stats.lock().unwrap();
                        s[0] += 1;
                        s[4] += usize::from(deleted);
                    }
                    let mut info = format!("Merged ({})", counts(subs, audios));
                    if let Some(note) = &note {
                        info.push_str(&format!(" - {note}"));
                        log.write("NOTE", &name, note);
                    }
                    say(&multiprogress, format!(
                        "{} {} -> {}",
                        SUCCESS,
                        display_name,
                        style(info).green()
                    ));
                }
                ProcessStatus::DryRun { output, subs, audios } => {
                    stats.lock().unwrap()[1] += 1;
                    say(&multiprogress, format!(
                        "{} {} -> {} ({})",
                        PREVIEW,
                        display_name,
                        style(output.display()).yellow(),
                        counts(subs, audios)
                    ));
                }
                ProcessStatus::Skipped => {
                    stats.lock().unwrap()[2] += 1;
                    say(&multiprogress, format!(
                        "{} {} -> {}",
                        SKIPPED,
                        display_name,
                        style("Already exists").yellow()
                    ));
                }
                ProcessStatus::Failed(e) => {
                    stats.lock().unwrap()[3] += 1;
                    log.write("FAIL", &video.display().to_string(), &e);
                    say(&multiprogress, format!(
                        "{} {} -> {}",
                        FAILED,
                        display_name,
                        style(e).red()
                    ));
                }
            }

            job_pb.set_style(ProgressStyle::with_template("  {spinner:.dim} {msg}").unwrap());
            job_pb.set_message(
                style(format!("Worker {} idle", worker_id + 1))
                    .dim()
                    .to_string(),
            );
            pb.inc(1);
        });
    });

    // Cleanup worker bars
    for job_pb in &worker_bars {
        job_pb.finish_and_clear();
    }
    pb.finish_with_message("Done");

    // 4. Summary Panel
    let [success, dry, skipped, failed, deleted] = *stats.lock().unwrap();
    let total = success + dry + skipped + failed;
    let done = success + dry;

    let success_pct = if total > 0 {
        (done as f32 / total as f32) * 100.0
    } else {
        0.0
    };

    println!();
    let mut summary_lines = vec![format!(
        "{} {:<18} {} ({:.1}%)",
        SUCCESS,
        if config.dry_run { "Would Merge:" } else { "Successfully Merged:" },
        style(done).green().bold(),
        success_pct
    )];
    summary_lines.extend([
        format!(
            "{} {:<18} {}",
            SKIPPED,
            "Skipped (Exists):",
            style(skipped).yellow()
        ),
        format!("{} {:<18} {}", FAILED, "Failures:", style(failed).red()),
        format!(
            "{} {:<18} {}",
            TRASH,
            "Originals Deleted:",
            if deleted > 0 {
                style(deleted.to_string()).red().bold()
            } else {
                style("0".to_string()).dim()
            }
        ),
        style("━".repeat(20)).dim().to_string(),
        format!(
            "{} Total Time: {}",
            SPARKLE,
            style(HumanDuration(start_scan.elapsed())).cyan().bold()
        ),
    ]);

    draw_panel("PROCESSING COMPLETE", &summary_lines, |s| {
        style(s).magenta().bold()
    });

    println!();
    pause(cli.yes);
    if failed > 0 {
        std::process::exit(1);
    }
}
