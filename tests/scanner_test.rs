use std::path::{Path, PathBuf};

fn get_fingerprint(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| c.to_lowercase().to_string())
        .collect()
}

fn strip_lang_suffix(stem: &str) -> String {
    // Minimal reimplementation of the scanner function for testing
    use std::collections::HashMap;
    let lang_data: HashMap<&str, (&str, &str)> = [
        ("fi", ("fin", "Finnish")),
        ("en", ("eng", "English")),
        ("sv", ("swe", "Swedish")),
        ("de", ("ger", "German")),
        ("fr", ("fre", "French")),
        ("es", ("spa", "Spanish")),
        ("no", ("nor", "Norwegian")),
        ("da", ("dan", "Danish")),
        ("ru", ("rus", "Russian")),
        ("it", ("ita", "Italian")),
        ("ja", ("jpn", "Japanese")),
        ("zh-cn", ("chi", "Chinese")),
        ("ko", ("kor", "Korean")),
        ("pl", ("pol", "Polish")),
        ("pt", ("por", "Portuguese")),
    ].iter().cloned().collect();

    let lower = stem.to_lowercase();

    // Check 3-letter codes first
    for (_iso1, (iso3, _name)) in lang_data.iter() {
        for sep in &[".", "_", "-"] {
            let pat = format!("{}{}", sep, iso3);
            if lower.ends_with(&pat) {
                let idx = stem.len().saturating_sub(pat.len());
                return stem[..idx].to_string();
            }
        }
    }

    // Then 2-letter codes, avoiding false match on 3-letter suffix
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

fn check_match(video_stem: &str, sub_stem: &str) -> bool {
    let video_fp = get_fingerprint(video_stem);
    let item_fp = get_fingerprint(sub_stem);

    // Primary: substring match
    if item_fp.contains(&video_fp) || video_fp.contains(&item_fp) {
        return true;
    }

    // Fallback: strip lang codes and retry
    let clean_stem = strip_lang_suffix(sub_stem);
    if clean_stem != sub_stem {
        let clean_fp = get_fingerprint(&clean_stem);
        if clean_fp.contains(&video_fp) || video_fp.contains(&clean_fp) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- EXISTING tests that already pass ---

    #[test]
    fn test_fingerprint_basic() {
        let fp = get_fingerprint("Riot.2015.1080p.BluRay.x264.YTS");
        assert_eq!(fp, "riot20151080pblurayx264yts");
    }

    #[test]
    fn test_fingerprint_with_lang() {
        let fp = get_fingerprint("Riot.2015.1080p.BluRay.x264.YTS.fin");
        assert_eq!(fp, "riot20151080pblurayx264ytsfin");
    }

    #[test]
    fn test_fingerprint_contains() {
        let video_fp = get_fingerprint("Riot.2015.1080p.BluRay.x264.YTS");
        let sub_fp = get_fingerprint("Riot.2015.1080p.BluRay.x264.YTS");
        assert!(sub_fp.contains(&video_fp));
        assert!(video_fp.contains(&sub_fp));
    }

    #[test]
    fn test_fingerprint_contains_with_lang() {
        // Video longer, sub has lang suffix
        assert!(check_match(
            "Riot.2015.1080p.BluRay.x264.YTS",
            "Riot.2015.1080p.BluRay.x264.YTS.fin"
        ));
    }

    #[test]
    fn test_fingerprint_shorter_sub() {
        assert!(check_match(
            "Riot.2015.1080p.BluRay.x264.YTS",
            "Riot.2015"
        ));
    }

    #[test]
    fn test_identical_stems() {
        assert!(check_match("Riot.2015", "Riot.2015"));
    }

    #[test]
    fn test_case_sensitivity() {
        assert!(check_match("RIOT.2015.1080P", "riot.2015.1080p"));
    }

    // --- BUG REPRODUCTION: cases that FAILED before the fix ---

    #[test]
    fn test_lang_code_suffix_mismatch() {
        // Video: Riot.2015.1080p.BluRay.x264.YTS.mp4
        // Sub:   Riot.2015.fin.srt
        // The "fin" doesn't appear contiguously in the video fingerprint
        assert!(check_match(
            "Riot.2015.1080p.BluRay.x264.YTS",
            "Riot.2015.fin"
        ), "FAILED: sub with .fin suffix should match! THIS IS THE BUG");
    }

    #[test]
    fn test_eng_suffix() {
        // Same bug with English
        assert!(check_match(
            "Movie.Name.2024.1080p.WEBRip.x264",
            "Movie.Name.2024.eng"
        ), "FAILED: .eng suffix should match!");
    }

    #[test]
    fn test_swe_suffix() {
        assert!(check_match(
            "Show.Name.S01E01.1080p.WEB-DL",
            "Show.Name.S01E01.swe"
        ), "FAILED: .swe suffix should match!");
    }

    #[test]
    fn test_2letter_code_fi() {
        assert!(check_match(
            "Riot.2015.1080p.BluRay.x264.YTS",
            "Riot.2015.fi"
        ), "FAILED: .fi suffix should match!");
    }

    #[test]
    fn test_2letter_code_en() {
        assert!(check_match(
            "Movie.Name.2024.2160p.WEBRip",
            "Movie.Name.2024.en"
        ), "FAILED: .en suffix should match!");
    }

    #[test]
    fn test_lang_code_middle_may_not_match() {
        // Sub: Riot.fin.2015.1080p.srt — lang code in middle (unusual pattern)
        // strip_lang_suffix only strips from the END, so this case is NOT matched.
        // This is a rare edge case and acceptable behavior.
        // The user should name subtitle files with lang code at the end.
        assert!(!check_match(
            "Riot.2015.1080p.BluRay.x264.YTS",
            "Riot.fin.2015.1080p"
        ));
    }

    #[test]
    fn test_underscore_lang_code() {
        assert!(check_match(
            "Riot.2015.1080p.BluRay.x264.YTS",
            "Riot.2015_fin"
        ), "FAILED: _fin suffix should match!");
    }

    #[test]
    fn test_hyphen_lang_code() {
        assert!(check_match(
            "Riot.2015.1080p.BluRay.x264.YTS",
            "Riot.2015-fin"
        ), "FAILED: -fin suffix should match!");
    }

    #[test]
    fn test_no_false_positive_completely_different() {
        // Different movies should NOT match
        assert!(!check_match("The.Dark.Knight.2008", "Inception.2010"),
            "Completely different movies should not match!");
    }

    #[test]
    fn test_no_false_positive_same_series() {
        // Same series different movies should NOT match
        assert!(!check_match("The.Dark.Knight.2008", "The.Dark.Knight.Rises.2012"),
            "Different movies in same series should not match!");
    }

    #[test]
    fn test_uppercase_lang() {
        assert!(check_match(
            "Riot.2015.1080p.BluRay.x264.YTS",
            "Riot.2015.FIN"
        ), "FAILED: .FIN (uppercase) should match!");
    }

    // --- All variations the filter should handle ---
    #[test]
    fn test_lang_code_variations() {
        let video_stem = "Riot.2015.1080p.BluRay.x264.YTS";

        let patterns = vec![
            "Riot.2015.1080p.BluRay.x264.YTS.fin",  // same name + lang suffix
            "Riot.2015.1080p.BluRay.x264.YTS.eng",  // same name + eng suffix
            "Riot.2015.1080p.BluRay.x264.YTS.FIN",  // uppercase lang
            "Riot.2015.1080p.BluRay.x264.YTS.fi",   // 2-letter code
            "Riot.2015",                              // just base name
            "Riot.2015.fin",                          // base + lang (THE BUG CASE)
            "Riot.2015.eng",                          // base + eng
            "Riot.2015.FIN",                          // base + uppercase lang
        ];

        for pattern in &patterns {
            assert!(
                check_match(video_stem, pattern),
                "Pattern '{}' failed to match!",
                pattern
            );
        }
    }
}
