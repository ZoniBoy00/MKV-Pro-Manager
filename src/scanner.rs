use std::path::{Path, PathBuf};
use crate::config::Config;

/// Normalizes text for fingerprinting: lowercase alphanumeric only.
fn get_fingerprint(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
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

    let entries = match std::fs::read_dir(parent) {
        Ok(e) => e,
        Err(_) => return assets,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path == *video_path || !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else { continue };
        let ext = format!(".{}", ext.to_ascii_lowercase());

        if !config.ext_sub.contains(&ext) && !config.ext_audio.contains(&ext) {
            continue;
        }

        let item_fingerprint = get_fingerprint(name);
        if item_fingerprint.contains(&video_fingerprint) {
            if config.ext_sub.contains(&ext) {
                assets.subtitles.push(path);
            } else if config.ext_audio.contains(&ext) {
                assets.audios.push(path);
            }
        }
    }

    assets
}
