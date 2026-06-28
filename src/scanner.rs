use std::path::{Path, PathBuf};
use std::collections::HashSet;
use regex::Regex;
use once_cell::sync::Lazy;
use crate::config::{Config, LANG_DATA};

static SXXEXX_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)s(\d+)e(\d+)").unwrap());

/// Common release tags stripped before fingerprint matching.
/// These are quality, source, codec, audio, group, region, and language tags
/// that differ between release groups (e.g. YTS vs DAWGS) but don't affect
/// the actual title of the media.
static RELEASE_TAGS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    // Quality
    for &t in &["480p", "720p", "1080p", "2160p", "4k"] { s.insert(t); }
    // Source
    for &t in &["bluray", "brrip", "webrip", "web-dl", "webdl", "web.dl",
               "hdtv", "hdrip", "dvdrip", "dvdscr", "cam", "hcam", "tc", "ts", "wp",
               "ppv", "remux"] { s.insert(t); }
    // Codec
    for &t in &["x264", "x265", "h264", "h265", "hevc", "avc", "mpeg2", "mpeg-2",
               "xvid", "divx", "vc1", "vc-1", "av1"] { s.insert(t); }
    // Audio
    for &t in &["ac3", "aac", "dts", "flac", "mp3", "eac3", "truehd", "dts-hd",
               "pcm", "dd5.1", "ddp5.1", "atmos"] { s.insert(t); }
    // Groups / Scene / Flags
    for &t in &["yts", "yify", "dawgs", "rarbg", "fgt", "ion10", "evo", "amzn",
               "nf", "ntb", "ctrlhd", "kappa", "axxo", "sparks", "fum",
               "proper", "repack", "internal", "readnfo", "unrated",
               "extended", "directors.cut", "theatrical", "edition"] { s.insert(t); }
    // Region tags
    for &t in &["nordic"] { s.insert(t); }
    // 3-letter language codes (from LANG_DATA values)
    for (_, (iso3, _)) in LANG_DATA.iter() {
        s.insert(*iso3);
    }
    // 2-letter language codes (from LANG_DATA keys)
    for (iso1, _) in LANG_DATA.iter() {
        s.insert(*iso1);
    }
    // Other common media tags
    for &t in &["hdr", "sdr", "dv", "dovi", "dolby", "vision", "uhd", "fhd",
               "hd", "sd", "pal", "ntsc"] { s.insert(t); }
    s
});

/// Normalizes text for fingerprinting: lowercase alphanumeric only.
fn get_fingerprint(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| c.to_lowercase().to_string())
        .collect()
}

/// Strips common language-code suffixes (e.g., `.fin`, `_eng`, `.fi`) from
/// a filename stem *for matching purposes only* — the original stem is
/// kept intact for language detection.
fn strip_lang_suffix(stem: &str) -> String {
    let lower = stem.to_lowercase();

    for (_iso1, (iso3, _name)) in LANG_DATA.iter() {
        for sep in &[".", "_", "-"] {
            let pat = format!("{}{}", sep, iso3);
            if lower.ends_with(&pat) {
                let idx = stem.len().saturating_sub(pat.len());
                return stem[..idx].to_string();
            }
        }
    }

    for (iso1, (_iso3, _name)) in LANG_DATA.iter() {
        for sep in &[".", "_", "-"] {
            let pat = format!("{}{}", sep, iso1);
            let three = format!("{}{}", sep, _iso3);
            if lower.ends_with(&pat) && !lower.ends_with(&three) {
                let idx = stem.len().saturating_sub(pat.len());
                return stem[..idx].to_string();
            }
        }
    }

    stem.to_string()
}

/// Strips all known release metadata tags from a filename stem so that
/// videos and subtitles from different release groups (YTS vs DAWGS etc.)
/// produce the same "clean name" for matching.
///
/// ## How it works
/// 1. Removes everything inside brackets `[...]`, `(...)`, `{...}`
/// 2. Splits by `.`, `-`, `_`, or space into tokens
/// 3. Drops tokens that match known release tags (quality, codec,
///    source, audio, group, region, language)
/// 4. Preserves the year and title words
/// 5. Rejoins with dots
///
/// ## Example
/// ```ignore
/// clean_stem_for_matching("Riot.2015.1080p.BluRay.x264.[YTS.AG]")
///   → "Riot.2015"
/// clean_stem_for_matching("Riot.2015.NORDIC.1080p.BluRay.x264.AC3-DAWGS-fin")
///   → "Riot.2015"
/// ```
fn clean_stem_for_matching(stem: &str) -> String {
    // Step 1: Remove everything inside brackets
    let no_brackets: String = {
        let mut result = String::with_capacity(stem.len());
        let mut depth = 0i32;
        for c in stem.chars() {
            match c {
                '[' | '(' | '{' => depth += 1,
                ']' | ')' | '}' => depth = depth.saturating_sub(1),
                _ if depth == 0 => result.push(c),
                _ => {}
            }
        }
        result
    };

    // Step 2: Split by dots only → tokens
    let tokens: Vec<&str> = no_brackets
        .split('.')
        .filter(|t| !t.is_empty())
        .collect();

    // Helper: check if a token (or any hyphen/underscore-split sub-token) is a known tag
    let is_known_tag = |token: &str| -> bool {
        let lower = token.to_lowercase();
        // Check full token first
        if RELEASE_TAGS.contains(lower.as_str()) {
            return true;
        }
        // Check sub-tokens split by common separators
        token.split(|c: char| c == '-' || c == '_')
            .any(|sub| !sub.is_empty() && RELEASE_TAGS.contains(sub.to_lowercase().as_str()))
    };

    // Step 3: Remove known release tags, keep year and title words
    let clean_tokens: Vec<String> = tokens.iter()
        .filter(|token| {
            // Keep year numbers (4-digit, 1900-2099)
            if let Ok(num) = token.parse::<u32>() {
                if (1900..=2099).contains(&num) {
                    return true;
                }
            }

            // Drop if token or any sub-component matches a known tag
            !is_known_tag(token)
        })
        .map(|t| t.to_string())
        .collect();

    clean_tokens.join(".")
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

    // Compute fingerprint from CLEANED stem (strips release tags so different
    // release groups like YTS and DAWGS can still match)
    let clean_video_stem = clean_stem_for_matching(video_stem);
    let video_fingerprint = get_fingerprint(&clean_video_stem);
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

            // Compute fingerprint from CLEANED stem for matching
            let clean_item_stem = clean_stem_for_matching(item_stem);
            let item_fingerprint = get_fingerprint(&clean_item_stem);

            // Match if one cleaned fingerprint is contained in the other,
            // or if they share a common SxxExx pattern
            let is_match = if item_fingerprint.contains(&video_fingerprint)
                || video_fingerprint.contains(&item_fingerprint)
            {
                true
            } else {
                // Fallback: try stripping just the language suffix
                let clean_stem = strip_lang_suffix(item_stem);
                if clean_stem != item_stem {
                    let clean_fp = get_fingerprint(&clean_stem);
                    clean_fp.contains(&video_fingerprint)
                        || video_fingerprint.contains(&clean_fp)
                } else {
                    false
                }
            } || {
                // Also try SxxExx matching
                if let (Some(v_cap), Some(i_cap)) =
                    (SXXEXX_RE.captures(video_stem), SXXEXX_RE.captures(name))
                {
                    v_cap[1] == i_cap[1] && v_cap[2] == i_cap[2]
                } else {
                    false
                }
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
