use std::path::{Path, PathBuf};
use regex::Regex;
use once_cell::sync::Lazy;
use crate::config::Config;

static SXXEXX_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)s(\d+)e(\d+)").unwrap());

/// Normalizes text for fingerprinting: lowercase alphanumeric only.
fn get_fingerprint(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| c.to_lowercase().to_string())
        .collect()
}

#[derive(Debug, Default)]
pub struct FoundAssets {
    pub subtitles: Vec<PathBuf>,
    pub audios: Vec<PathBuf>,
}

/// Scans the directory of the given video file for matching subtitles and audio tracks.
pub fn find_matching_assets(video_path: &Path, config: &Config) -> FoundAssets {
    let mut assets = FoundAssets::default();

    let Some(parent) = video_path.parent() else { return assets; };
    let Some(video_stem) = video_path.file_stem().and_then(|s| s.to_str()) else { return assets; };

    let video_fingerprint = get_fingerprint(video_stem);
    if video_fingerprint.is_empty() { return assets; }

    // Directories to search: current folder and potential 'Subs' or 'Subtitles' subfolders
    let mut search_dirs = vec![parent.to_path_buf()];
    
    let subs_subfolder = parent.join("Subs");
    if subs_subfolder.is_dir() { search_dirs.push(subs_subfolder); }
    
    let subtitles_subfolder = parent.join("Subtitles");
    if subtitles_subfolder.is_dir() { search_dirs.push(subtitles_subfolder); }

    for dir in search_dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path == *video_path || !path.is_file() {
                continue;
            }

            let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
            let Some(item_stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else { continue };
            let ext = format!(".{}", ext.to_ascii_lowercase());

            if !config.ext_sub.contains(&ext) && !config.ext_audio.contains(&ext) {
                continue;
            }

            let item_fingerprint = get_fingerprint(item_stem);
            
            // Match if one fingerprint is contained in the other, or if they share a common SxxExx
            // This handles cases where video has more tags than sub, or vice versa.
            let is_match = if item_fingerprint.contains(&video_fingerprint) || video_fingerprint.contains(&item_fingerprint) {
                true
            } else if let (Some(v_cap), Some(i_cap)) = (SXXEXX_RE.captures(video_stem), SXXEXX_RE.captures(name)) {
                v_cap[1] == i_cap[1] && v_cap[2] == i_cap[2]
            } else {
                false
            };

            if is_match {
                if config.ext_sub.contains(&ext) {
                    assets.subtitles.push(path);
                } else if config.ext_audio.contains(&ext) {
                    assets.audios.push(path);
                }
            }
        }
    }

    assets
}
