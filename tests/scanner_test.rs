use std::collections::HashSet;
use std::path::{Path, PathBuf};

// Minimal reimplementation of scanner functions for testing
fn get_fingerprint(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| c.to_lowercase().to_string())
        .collect()
}

fn clean_stem_for_matching(stem: &str) -> String {
    // Build release tags set (mirrors scanner.rs)
    let release_tags: HashSet<&str> = [
        "480p", "720p", "1080p", "2160p", "4k",
        "bluray", "brrip", "webrip", "web-dl", "webdl", "web.dl",
        "hdtv", "hdrip", "dvdrip", "dvdscr", "cam", "hcam", "tc", "ts", "wp",
        "ppv", "remux",
        "x264", "x265", "h264", "h265", "hevc", "avc", "mpeg2", "mpeg-2",
        "xvid", "divx", "vc1", "vc-1", "av1",
        "ac3", "aac", "dts", "flac", "mp3", "eac3", "truehd", "dts-hd",
        "pcm", "dd5.1", "ddp5.1", "atmos",
        "yts", "yify", "dawgs", "rarbg", "fgt", "ion10", "evo", "amzn",
        "nf", "ntb", "ctrlhd", "kappa", "axxo", "sparks", "fum",
        "proper", "repack", "internal", "readnfo", "unrated",
        "extended", "directors.cut", "theatrical", "edition",
        "nordic",
        "fin", "eng", "swe", "ger", "fre", "spa", "nor", "dan", "rus",
        "ita", "jpn", "chi", "kor", "pol", "por",
        "fi", "en", "sv", "de", "fr", "es", "it", "ja", "ko", "pl", "pt",
        "ru", "no", "da",
        "hdr", "sdr", "dv", "dovi", "dolby", "vision", "uhd", "fhd",
        "hd", "sd", "pal", "ntsc",
    ].iter().cloned().collect();

    fn is_known_tag(token: &str, tags: &HashSet<&str>) -> bool {
        // Check full token first
        if tags.contains(token.to_lowercase().as_str()) {
            return true;
        }
        // Check sub-tokens split by common separators
        token.split(|c: char| c == '-' || c == '_')
            .any(|sub| !sub.is_empty() && tags.contains(sub.to_lowercase().as_str()))
    }

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
    let dot_tokens: Vec<&str> = no_brackets
        .split('.')
        .filter(|t| !t.is_empty())
        .collect();

    // Step 3: Remove known release tags, keep year and title words
    let clean_tokens: Vec<String> = dot_tokens.iter()
        .filter(|token| {
            let lower = token.to_lowercase();

            // Keep year numbers (4-digit, 1900-2099)
            if let Ok(num) = token.parse::<u32>() {
                if (1900..=2099).contains(&num) {
                    return true;
                }
            }

            // Drop if token or any sub-component matches a known tag
            !is_known_tag(token, &release_tags)
        })
        .map(|t| t.to_string())
        .collect();

    clean_tokens.join(".")
}

fn check_match(video_stem: &str, sub_stem: &str) -> bool {
    let clean_video = clean_stem_for_matching(video_stem);
    let clean_sub = clean_stem_for_matching(sub_stem);
    let video_fp = get_fingerprint(&clean_video);
    let sub_fp = get_fingerprint(&clean_sub);

    sub_fp.contains(&video_fp) || video_fp.contains(&sub_fp)
}

fn strip_lang_suffix(stem: &str) -> String {
    let lang_data: std::collections::HashMap<&str, (&str, &str)> = [
        ("fi", ("fin", "Finnish")), ("en", ("eng", "English")),
        ("sv", ("swe", "Swedish")), ("de", ("ger", "German")),
        ("fr", ("fre", "French")), ("es", ("spa", "Spanish")),
        ("no", ("nor", "Norwegian")), ("da", ("dan", "Danish")),
        ("ru", ("rus", "Russian")), ("it", ("ita", "Italian")),
        ("ja", ("jpn", "Japanese")), ("zh-cn", ("chi", "Chinese")),
        ("ko", ("kor", "Korean")), ("pl", ("pol", "Polish")),
        ("pt", ("por", "Portuguese")),
    ].iter().cloned().collect();

    let lower = stem.to_lowercase();
    for (_iso1, (iso3, _name)) in lang_data.iter() {
        for sep in &[".", "_", "-"] {
            let pat = format!("{}{}", sep, iso3);
            if lower.ends_with(&pat) {
                let idx = stem.len().saturating_sub(pat.len());
                return stem[..idx].to_string();
            }
        }
    }
    for (iso1, (iso3, _name)) in lang_data.iter() {
        for sep in &[".", "_", "-"] {
            let pat = format!("{}{}", sep, iso1);
            let three = format!("{}{}", sep, iso3);
            if lower.ends_with(&pat) && !lower.ends_with(&three) {
                let idx = stem.len().saturating_sub(pat.len());
                return stem[..idx].to_string();
            }
        }
    }
    stem.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // === clean_stem_for_matching tests ===

    #[test]
    fn test_clean_matching_yts_vs_dawgs() {
        // THE ACTUAL BUG CASE from Joni's input folder
        let video = "Riot.2015.1080p.BluRay.x264.[YTS.AG]";
        let sub = "Riot.2015.NORDIC.1080p.BluRay.x264.AC3-DAWGS-fin";

        assert_eq!(clean_stem_for_matching(video), "Riot.2015");
        assert_eq!(clean_stem_for_matching(sub), "Riot.2015");
        assert!(check_match(video, sub),
            "YTS release should match DAWGS subtitle! THIS IS THE BUG");
    }

    #[test]
    fn test_clean_matching_same_stem() {
        assert_eq!(clean_stem_for_matching("Riot.2015"), "Riot.2015");
    }

    #[test]
    fn test_clean_bracket_removal() {
        assert_eq!(clean_stem_for_matching("Riot.2015.[YTS.AG]"), "Riot.2015");
        assert_eq!(clean_stem_for_matching("Riot.2015.(YTS)"), "Riot.2015");
        assert_eq!(clean_stem_for_matching("Riot.2015.{YTS}"), "Riot.2015");
    }

    #[test]
    fn test_clean_strips_quality() {
        assert_eq!(clean_stem_for_matching("Riot.2015.1080p"), "Riot.2015");
        assert_eq!(clean_stem_for_matching("Riot.2015.2160p.4k"), "Riot.2015");
    }

    #[test]
    fn test_clean_strips_source() {
        assert_eq!(clean_stem_for_matching("Riot.2015.BluRay.WEBRip"), "Riot.2015");
    }

    #[test]
    fn test_clean_strips_codec() {
        assert_eq!(clean_stem_for_matching("Riot.2015.x264.HEVC"), "Riot.2015");
    }

    #[test]
    fn test_clean_strips_audio() {
        assert_eq!(clean_stem_for_matching("Riot.2015.AC3.DTS"), "Riot.2015");
    }

    #[test]
    fn test_clean_strips_group() {
        assert_eq!(clean_stem_for_matching("Riot.2015.YTS"), "Riot.2015");
        assert_eq!(clean_stem_for_matching("Riot.2015.DAWGS"), "Riot.2015");
    }

    #[test]
    fn test_clean_strips_region() {
        assert_eq!(clean_stem_for_matching("Riot.2015.NORDIC"), "Riot.2015");
    }

    #[test]
    fn test_clean_strips_language_codes() {
        assert_eq!(clean_stem_for_matching("Riot.2015.fin"), "Riot.2015");
        assert_eq!(clean_stem_for_matching("Riot.2015.eng"), "Riot.2015");
        assert_eq!(clean_stem_for_matching("Riot.2015.fi"), "Riot.2015");
    }

    #[test]
    fn test_clean_preserves_title_variations() {
        assert_ne!(
            clean_stem_for_matching("The.Dark.Knight.2008"),
            clean_stem_for_matching("The.Dark.Knight.Rises.2012")
        );
    }

    #[test]
    fn test_clean_without_year() {
        assert_eq!(clean_stem_for_matching("Riot.1080p.BluRay.x264.YTS"), "Riot");
    }

    #[test]
    fn test_clean_complex_movie_name() {
        let result = clean_stem_for_matching("The.Godfather.Part.II.1974.1080p.BluRay.x264");
        assert_eq!(result, "The.Godfather.Part.II.1974");
    }

    #[test]
    fn test_clean_webdl_hyphenated() {
        assert_eq!(clean_stem_for_matching("Riot.2015.1080p.WEB-DL"), "Riot.2015");
    }

    #[test]
    fn test_clean_x264_group() {
        // x264-ABC: "x264" is a codec tag inside hyphenated token
        assert_eq!(clean_stem_for_matching("Riot.2015.x264-ABC"), "Riot.2015");
    }

    // === check_match integration tests ===

    #[test]
    fn test_match_yts_vs_dawgs_actual_filenames() {
        assert!(check_match(
            "Riot.2015.1080p.BluRay.x264.[YTS.AG]",
            "Riot.2015.NORDIC.1080p.BluRay.x264.AC3-DAWGS-fin"
        ), "YTS video must match DAWGS subtitle!");
    }

    #[test]
    fn test_match_identical() {
        assert!(check_match("Riot.2015.1080p", "Riot.2015.1080p"));
    }

    #[test]
    fn test_match_sub_shorter() {
        assert!(check_match("Riot.2015.1080p.BluRay.x264.YTS", "Riot.2015.fin"));
    }

    #[test]
    fn test_match_different_release_groups() {
        assert!(check_match(
            "Movie.Name.2024.2160p.WEBRip.x264.AAC-EVO",
            "Movie.Name.2024.NORDIC.1080p.BluRay.x264.AC3-DAWGS-eng"
        ));
    }

    #[test]
    fn test_no_match_different_movies() {
        assert!(!check_match("The.Dark.Knight.2008", "Inception.2010"),
            "Different movies should NOT match!");
    }

    #[test]
    fn test_no_match_same_series_different_years() {
        assert!(!check_match("The.Dark.Knight.2008", "The.Dark.Knight.Rises.2012"),
            "Different movies in same series should NOT match!");
    }

    #[test]
    fn test_match_subtitle_with_hyphens() {
        assert!(check_match(
            "Riot.2015.1080p.WEB-DL",
            "Riot.2015.NORDIC.WEBRiP.x264-ABC"
        ));
    }

    #[test]
    fn test_upper_vs_lower() {
        assert!(check_match("RIOT.2015.1080P.BLURAY", "riot.2015.1080p"));
    }

    // === Existing strip_lang_suffix tests ===

    #[test]
    fn test_strip_lang_suffix() {
        assert_eq!(strip_lang_suffix("Riot.2015.fin"), "Riot.2015");
        assert_eq!(strip_lang_suffix("Riot.2015.eng"), "Riot.2015");
        assert_eq!(strip_lang_suffix("Riot.2015.fi"), "Riot.2015");
        assert_eq!(strip_lang_suffix("Riot.2015.en"), "Riot.2015");
        assert_eq!(strip_lang_suffix("Riot.2015"), "Riot.2015");
    }
}
