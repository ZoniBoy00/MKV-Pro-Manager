use crate::config::Config;
use crate::lang::{analyze_subtitle, audio_language, choose_default, find_language, SubtitleInfo};
use crate::naming::{is_year, parse_episode, strip_brackets, tokens};
use crate::scanner::{DirIndex, FoundAssets};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Set by the Ctrl-C handler; no new file is started once it is true.
pub static CANCELLED: AtomicBool = AtomicBool::new(false);

static SEASON_ONLY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(?:season|s)[ ._-]?(\d{1,2})\b").unwrap());

/// Track entries in `mkvmerge -J` output (locale independent).
static TRACK_ENTRY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#""type"\s*:\s*"(?:video|audio|subtitles|buttons)""#).unwrap()
});

pub enum ProcessStatus {
    Success {
        subs: usize,
        audios: usize,
        deleted: bool,
        /// Something worth telling the user (warnings, originals kept).
        note: Option<String>,
    },
    /// Dry run: what would have been created.
    DryRun {
        output: PathBuf,
        subs: usize,
        audios: usize,
    },
    Skipped,
    Failed(String),
}

#[derive(Debug, PartialEq)]
pub struct MediaInfo {
    pub title: String,
    pub is_series: bool,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub year: Option<String>,
}

impl MediaInfo {
    fn season_folder(&self) -> String {
        format!("Season {:02}", self.season.unwrap_or(1))
    }
}

fn clean_title(input: &str) -> String {
    input
        .replace(['.', '_', '-'], " ")
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn infer_title_from_folder(path: &Path) -> String {
    let components: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect();

    let len = components.len();
    if len < 3 {
        return "Unknown".to_string();
    }

    let parent = &components[len - 2];
    if SEASON_ONLY.is_match(parent) {
        clean_title(&components[len - 3])
    } else {
        clean_title(parent)
    }
}

/// Works out title, year and season/episode from a video path.
pub fn media_info(path: &Path) -> MediaInfo {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let cleaned = strip_brackets(stem);

    if let Some(ep) = parse_episode(&cleaned) {
        let head = tokens(&cleaned[..ep.start]);
        let letters: usize = head
            .iter()
            .map(|t| t.chars().filter(|c| c.is_alphanumeric()).count())
            .sum();
        let title = if letters < 2 {
            infer_title_from_folder(path)
        } else {
            clean_title(&head.join(" "))
        };
        return MediaInfo {
            title,
            is_series: true,
            season: Some(ep.season),
            episode: Some(ep.episode),
            year: None,
        };
    }

    let words = tokens(&cleaned);

    // A year only counts after at least one title word (`2012` alone is a title).
    if let Some(i) = words.iter().rposition(|t| is_year(t)).filter(|&i| i > 0) {
        return MediaInfo {
            title: clean_title(&words[..i].join(" ")),
            is_series: false,
            season: None,
            episode: None,
            year: Some(words[i].clone()),
        };
    }

    if let Some(parent_name) = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) {
        if let Some(season) = SEASON_ONLY
            .captures(parent_name)
            .and_then(|c| c[1].parse::<u32>().ok())
        {
            return MediaInfo {
                title: infer_title_from_folder(path),
                is_series: true,
                season: Some(season),
                episode: None,
                year: None,
            };
        }
    }

    let title = clean_title(&words.join(" "));
    MediaInfo {
        title: if title.is_empty() { clean_title(stem) } else { title },
        is_series: false,
        season: None,
        episode: None,
        year: None,
    }
}

fn sanitize_filename(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    // Windows silently drops trailing dots/spaces and rejects device names.
    let trimmed = replaced.trim_end_matches(['.', ' ']).trim_start();
    let base = trimmed.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.ends_with(|c: char| c.is_ascii_digit() && c != '0'));
    if trimmed.is_empty() {
        "_".to_string()
    } else if reserved {
        format!("_{trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// `file.mkv` -> `file.mkv.part`
fn part_path(output: &Path) -> PathBuf {
    let mut name = output.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    output.with_file_name(name)
}

/// Counts the tracks mkvmerge sees in a finished file.
fn count_tracks(mkvmerge: &Path, file: &Path) -> Option<usize> {
    let output = Command::new(mkvmerge).arg("-J").arg(file).output().ok()?;
    if !matches!(output.status.code(), Some(0) | Some(1)) {
        return None;
    }
    Some(TRACK_ENTRY.find_iter(&String::from_utf8_lossy(&output.stdout)).count())
}

pub struct Processor {
    config: Config,
    default_iso3: &'static str,
    /// Output files being written right now, so two inputs mapping to the
    /// same name cannot write the same file at once.
    in_flight: Mutex<HashSet<PathBuf>>,
    /// One directory scan per video folder instead of one per video.
    indexes: Mutex<HashMap<PathBuf, Arc<DirIndex>>>,
}

impl Processor {
    pub fn new(config: Config) -> Self {
        let default_iso3 = find_language(&config.default_sub_lang).map_or("fin", |l| l.iso3);
        Self {
            config,
            default_iso3,
            in_flight: Mutex::new(HashSet::new()),
            indexes: Mutex::new(HashMap::new()),
        }
    }

    fn assets_for(&self, video_path: &Path) -> FoundAssets {
        let Some(parent) = video_path.parent() else {
            return FoundAssets::default();
        };
        let cached = self.indexes.lock().unwrap().get(parent).cloned();
        let index = match cached {
            Some(index) => index,
            None => {
                let built = Arc::new(DirIndex::build(parent, &self.config));
                self.indexes
                    .lock()
                    .unwrap()
                    .entry(parent.to_path_buf())
                    .or_insert(built)
                    .clone()
            }
        };
        index.assets_for(video_path)
    }

    /// Where the merged file for `video_path` goes.
    pub fn output_path(&self, video_path: &Path) -> PathBuf {
        let info = media_info(video_path);
        let title = sanitize_filename(&info.title);
        let stem = video_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");

        let (dir, file_stem) = if info.is_series {
            let dir = self
                .config
                .output_root
                .join("TV Shows")
                .join(&title)
                .join(info.season_folder());
            let name = match info.episode {
                Some(episode) => format!(
                    "{} S{:02}E{:02}",
                    info.title,
                    info.season.unwrap_or(1),
                    episode
                ),
                None => clean_title(&tokens(stem).join(" ")),
            };
            (dir, name)
        } else {
            let folder = match &info.year {
                Some(y) => format!("{} ({})", info.title, y),
                None => info.title.clone(),
            };
            (
                self.config.output_root.join("Movies").join(sanitize_filename(&folder)),
                info.title.clone(),
            )
        };
        dir.join(format!("{}.mkv", sanitize_filename(&file_stem)))
    }

    pub fn process_file(&self, video_path: &Path) -> ProcessStatus {
        if CANCELLED.load(Ordering::SeqCst) {
            return ProcessStatus::Failed("Cancelled".to_string());
        }

        let output_file = self.output_path(video_path);
        if output_file.exists() {
            return ProcessStatus::Skipped;
        }
        if !self.in_flight.lock().unwrap().insert(output_file.clone()) {
            return ProcessStatus::Skipped;
        }

        let status = self.merge(video_path, &output_file);
        self.in_flight.lock().unwrap().remove(&output_file);
        status
    }

    fn merge(&self, video_path: &Path, output_file: &Path) -> ProcessStatus {
        let assets = self.assets_for(video_path);
        let (subs, audios) = (assets.subtitles.len(), assets.audios.len());

        if self.config.dry_run {
            return ProcessStatus::DryRun {
                output: output_file.to_path_buf(),
                subs,
                audios,
            };
        }

        if let Some(dir) = output_file.parent() {
            if let Err(e) = fs::create_dir_all(dir) {
                return ProcessStatus::Failed(format!("Dir Create Error: {}", e));
            }
        }

        // Written under a temporary name, so an interrupted run never leaves
        // a half-written file that later passes for a finished one.
        let part = part_path(output_file);
        let mut cmd = Command::new(&self.config.mkvmerge_path);
        cmd.arg("-o").arg(&part).arg(video_path);

        let infos: Vec<SubtitleInfo> = assets.subtitles.iter().map(|s| analyze_subtitle(s)).collect();
        let default_sub = choose_default(&infos, self.default_iso3);
        for (i, (sub, info)) in assets.subtitles.iter().zip(&infos).enumerate() {
            cmd.arg("--language")
                .arg(format!("0:{}", info.iso3))
                .arg("--track-name")
                .arg(format!("0:{}", info.name))
                .arg("--default-track")
                .arg(format!("0:{}", if default_sub == Some(i) { 1 } else { 0 }));
            if info.forced {
                cmd.arg("--forced-display-flag").arg("0:1");
            }
            if info.sdh {
                cmd.arg("--hearing-impaired-flag").arg("0:1");
            }
            if let Some(charset) = info.charset {
                cmd.arg("--sub-charset").arg(format!("0:{}", charset));
            }
            cmd.arg(sub);
        }

        for audio in &assets.audios {
            let (iso, name) = audio_language(audio);
            cmd.arg("--language")
                .arg(format!("0:{}", iso))
                .arg("--track-name")
                .arg(format!("0:{}", name))
                .arg(audio);
        }

        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) => return ProcessStatus::Failed(e.to_string()),
        };

        // mkvmerge: 0 = ok, 1 = finished with warnings, 2 = error.
        let code = output.status.code();
        if !matches!(code, Some(0) | Some(1)) {
            let _ = fs::remove_file(&part);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            // mkvmerge reports errors on stdout; keep its "Error:" lines, not the banner.
            let errors: Vec<&str> = stdout
                .lines()
                .filter(|l| l.starts_with("Error:"))
                .collect();
            let message = if !errors.is_empty() {
                errors.join(" ")
            } else if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            return ProcessStatus::Failed(message);
        }
        let mut notes = Vec::new();
        if code == Some(1) {
            notes.push("mkvmerge reported warnings".to_string());
        }

        if let Err(e) = fs::rename(&part, output_file) {
            let _ = fs::remove_file(&part);
            return ProcessStatus::Failed(format!("Could not finalize output: {}", e));
        }

        let mut deleted = false;
        if self.config.delete_originals {
            // Originals are only removed once the new file demonstrably holds
            // the video plus every track that was merged in.
            let expected = 1 + subs + audios;
            match count_tracks(&self.config.mkvmerge_path, output_file) {
                Some(found) if found >= expected => {
                    let mut leftovers = 0;
                    let originals = std::iter::once(video_path)
                        .chain(assets.subtitles.iter().map(PathBuf::as_path))
                        .chain(assets.audios.iter().map(PathBuf::as_path));
                    for path in originals {
                        if fs::remove_file(path).is_err() {
                            leftovers += 1;
                        }
                    }
                    deleted = leftovers == 0;
                    if leftovers > 0 {
                        notes.push(format!("{leftovers} original file(s) could not be deleted"));
                    }
                }
                Some(found) => {
                    let _ = fs::remove_file(output_file);
                    return ProcessStatus::Failed(format!(
                        "Verification failed: output has {found} tracks, expected at least {expected}. Originals kept."
                    ));
                }
                None => notes.push("could not verify output, originals kept".to_string()),
            }
        }

        ProcessStatus::Success {
            subs,
            audios,
            deleted,
            note: (!notes.is_empty()).then(|| notes.join("; ")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(path: &str) -> MediaInfo {
        media_info(Path::new(path))
    }

    fn movie(title: &str, year: Option<&str>) -> MediaInfo {
        MediaInfo {
            title: title.to_string(),
            is_series: false,
            season: None,
            episode: None,
            year: year.map(String::from),
        }
    }

    #[test]
    fn resolution_is_not_a_year() {
        assert_eq!(info("Movie.Name.1080p.BluRay.x264-YTS.mkv"), movie("Movie Name", None));
        assert_eq!(info("Movie.Name.2160p.mkv"), movie("Movie Name", None));
    }

    #[test]
    fn year_and_title_are_split() {
        assert_eq!(info("Inception.2010.1080p.mkv"), movie("Inception", Some("2010")));
        assert_eq!(
            info("Blade.Runner.2049.2017.1080p.mkv"),
            movie("Blade Runner 2049", Some("2017"))
        );
        assert_eq!(info("1917.2019.mkv"), movie("1917", Some("2019")));
        assert_eq!(info("2012.mkv"), movie("2012", None));
    }

    #[test]
    fn resolution_in_wxh_form_does_not_make_a_series() {
        assert!(!info("Movie.1920x1080.mkv").is_series);
    }

    #[test]
    fn episodes_are_parsed_and_padded() {
        let i = info("Breaking.Bad.S1E5.720p.mkv");
        assert!(i.is_series);
        assert_eq!((i.title.as_str(), i.season, i.episode), ("Breaking Bad", Some(1), Some(5)));
        let i = info("Show Name 2x07.mkv");
        assert_eq!((i.title.as_str(), i.season, i.episode), ("Show Name", Some(2), Some(7)));
    }

    #[test]
    fn season_folder_gives_series_without_episode_tag() {
        let i = info(r"C:\Media\Some Show\Season 2\episode.mkv");
        assert!(i.is_series);
        assert_eq!((i.title.as_str(), i.season, i.episode), ("Some Show", Some(2), None));
    }

    fn processor(root: &str) -> Processor {
        Processor::new(Config {
            output_root: PathBuf::from(root),
            ..Config::default()
        })
    }

    #[test]
    fn output_paths_follow_the_library_layout() {
        let p = processor("out");
        assert_eq!(
            p.output_path(Path::new("Breaking.Bad.S1E5.720p.mkv")),
            Path::new("out").join("TV Shows").join("Breaking Bad").join("Season 01").join("Breaking Bad S01E05.mkv")
        );
        assert_eq!(
            p.output_path(Path::new("Inception.2010.1080p.mkv")),
            Path::new("out").join("Movies").join("Inception (2010)").join("Inception.mkv")
        );
    }

    #[test]
    fn filenames_are_made_safe_for_windows() {
        assert_eq!(sanitize_filename("What: If?"), "What_ If_");
        assert_eq!(sanitize_filename("Title..."), "Title");
        assert_eq!(sanitize_filename("CON"), "_CON");
        assert_eq!(sanitize_filename("aux.txt"), "_aux.txt");
        assert_eq!(sanitize_filename("COM1"), "_COM1");
        assert_eq!(sanitize_filename("COM0"), "COM0");
        assert_eq!(sanitize_filename("..."), "_");
    }

    #[test]
    fn partial_output_gets_its_own_name() {
        assert_eq!(part_path(Path::new("a/b.mkv")), Path::new("a/b.mkv.part"));
    }

    #[test]
    fn same_output_name_is_claimed_only_once() {
        let p = processor("out");
        let output = p.output_path(Path::new("Inception.2010.1080p.mkv"));
        assert!(p.in_flight.lock().unwrap().insert(output.clone()));
        // A second input for the same movie finds the name taken.
        assert!(matches!(
            p.process_file(Path::new("Inception.2010.720p.mkv")),
            ProcessStatus::Skipped
        ));
    }
}
