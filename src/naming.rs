//! Filename parsing shared by the scanner (asset matching) and the processor
//! (output naming): release-tag stripping, year/episode detection and the
//! token-based matching rules.

use crate::lang::{find_language, is_flag_token};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;

/// Quality, source, codec, audio, group and edition tags that differ between
/// releases (e.g. YTS vs DAWGS) but are not part of the title.
const RELEASE_TAG_LIST: &[&str] = &[
    // Quality
    "480p", "720p", "1080p", "2160p", "4k",
    // Source
    "bluray", "brrip", "webrip", "web-dl", "webdl", "web.dl", "hdtv", "hdrip", "dvdrip",
    "dvdscr", "cam", "hcam", "tc", "ts", "wp", "ppv", "remux",
    // Codec
    "x264", "x265", "h264", "h265", "hevc", "avc", "mpeg2", "mpeg-2", "xvid", "divx", "vc1",
    "vc-1", "av1",
    // Audio
    "ac3", "aac", "dts", "flac", "mp3", "eac3", "truehd", "dts-hd", "pcm", "dd5.1", "ddp5.1",
    "atmos",
    // Groups / scene / flags
    "yts", "yify", "dawgs", "rarbg", "fgt", "ion10", "evo", "amzn", "nf", "ntb", "ctrlhd",
    "kappa", "axxo", "sparks", "fum", "proper", "repack", "internal", "readnfo", "unrated",
    "extended", "directors.cut", "theatrical", "edition",
    // Region and other media tags
    "nordic", "hdr", "sdr", "dv", "dovi", "dolby", "vision", "uhd", "fhd", "hd", "sd", "pal",
    "ntsc",
];

static RELEASE_TAGS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| RELEASE_TAG_LIST.iter().copied().collect());

/// Tags that come in many spellings: `1080p`, `10bit`, `DDP51`, `AAC20`.
static PATTERN_TAG: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:\d{3,4}[pi]|\d{1,2}bit|(?:dd|ddp|aac|ac3|eac3|dts|truehd|flac|atmos)\d{0,3})$")
        .unwrap()
});

/// `DDP5.1` -> `DDP51`, `H.264` -> `H264`, so the dot does not split the tag.
static DOTTED_TAG: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(dd|ddp|aac|ac3|eac3|dts|truehd|atmos)[ ._-]?([257])\.([01])\b|\b([hx])\.(26[45])\b")
        .unwrap()
});

static EPISODE_SXXEXX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:^|[^a-z0-9])(s(\d{1,3})[ ._-]?e(\d{1,4}))").unwrap());

static EPISODE_NXNN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:^|[^a-z0-9])((\d{1,2})x(\d{2,3}))(?:[^a-z0-9]|$)").unwrap());

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Episode {
    pub season: u32,
    pub episode: u32,
    /// Byte offset of the `S01E02` / `1x02` tag in the string that was parsed.
    pub start: usize,
}

/// Finds the first `S01E02` or `1x02` style tag.
pub fn parse_episode(text: &str) -> Option<Episode> {
    let from = |re: &Regex| {
        re.captures(text).and_then(|caps| {
            Some(Episode {
                season: caps[2].parse().ok()?,
                episode: caps[3].parse().ok()?,
                start: caps.get(1)?.start(),
            })
        })
    };
    [from(&EPISODE_SXXEXX), from(&EPISODE_NXNN)]
        .into_iter()
        .flatten()
        .min_by_key(|e| e.start)
}

/// A plausible release year (the old `\d{4}` also matched `1080` in `1080p`).
pub fn is_year(token: &str) -> bool {
    token.len() == 4 && token.parse::<u32>().is_ok_and(|y| (1900..=2099).contains(&y))
}

/// Normalizes text for fingerprinting: lowercase alphanumeric only.
pub fn get_fingerprint(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Removes everything inside `[...]`, `(...)` and `{...}`.
pub fn strip_brackets(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut depth = 0i32;
    for c in text.chars() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => result.push(c),
            _ => {}
        }
    }
    result
}

fn is_tag(lowercase_token: &str) -> bool {
    RELEASE_TAGS.contains(lowercase_token) || PATTERN_TAG.is_match(lowercase_token)
}

/// Splits a stem into title/year tokens with release tags removed.
///
/// Brackets are dropped first. A hyphenated token such as `Movie-1080p` loses
/// only its tag parts; a trailing `x264-GROUP` token is a scene group suffix
/// and is dropped whole.
pub fn tokens(stem: &str) -> Vec<String> {
    let stripped = strip_brackets(stem);
    let text = DOTTED_TAG.replace_all(&stripped, |caps: &regex::Captures| {
        match caps.get(1) {
            Some(prefix) => format!("{}{}{}", prefix.as_str(), &caps[2], &caps[3]),
            None => format!("{}{}", &caps[4], &caps[5]),
        }
    });
    let raw: Vec<&str> = text
        .split(['.', ' ', '_'])
        .filter(|t| !t.is_empty())
        .collect();

    let last = raw.len().saturating_sub(1);
    let mut result = Vec::with_capacity(raw.len());
    for (i, token) in raw.iter().enumerate() {
        if is_tag(&token.to_lowercase()) {
            continue;
        }
        if !token.contains('-') {
            result.push(token.to_string());
            continue;
        }
        let parts: Vec<&str> = token.split('-').filter(|p| !p.is_empty()).collect();
        if i == last && parts.first().is_some_and(|p| is_tag(&p.to_lowercase())) {
            continue;
        }
        let kept: Vec<&str> = parts
            .into_iter()
            .filter(|p| !is_tag(&p.to_lowercase()))
            .collect();
        if !kept.is_empty() {
            result.push(kept.join("-"));
        }
    }
    result
}

/// Strips release metadata so videos and subtitles from different release
/// groups produce the same name.
///
/// ```ignore
/// clean_stem_for_matching("Riot.2015.1080p.BluRay.x264.[YTS.AG]") // "Riot.2015"
/// ```
pub fn clean_stem_for_matching(stem: &str) -> String {
    tokens(stem).join(".")
}

fn fingerprint_tokens(tokens: Vec<String>) -> Vec<String> {
    tokens
        .iter()
        .map(|t| get_fingerprint(t))
        .filter(|t| !t.is_empty())
        .collect()
}

/// What identifies a piece of media: title tokens and, for series, the episode.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchKey {
    pub title: Vec<String>,
    pub episode: Option<(u32, u32)>,
}

pub fn match_key(stem: &str) -> MatchKey {
    let cleaned = strip_brackets(stem);
    let episode = parse_episode(&cleaned);
    let head = match &episode {
        Some(e) => &cleaned[..e.start],
        None => &cleaned[..],
    };
    MatchKey {
        title: fingerprint_tokens(tokens(head)),
        episode: episode.map(|e| (e.season, e.episode)),
    }
}

fn strip_trailing_language(mut title: Vec<String>, aggressive: bool) -> Vec<String> {
    while title.len() > 1 {
        let last = title.last().expect("non-empty");
        let is_language = find_language(last).is_some()
            && (aggressive || last.len() > 2 || title.len() > 2);
        if is_flag_token(last) || is_language {
            title.pop();
        } else {
            break;
        }
    }
    title
}

/// Keys for a subtitle/audio file. The trailing language/flag tokens are
/// stripped; a two-letter code is also tried as a real title word
/// (`Dr.No`) alongside the aggressive reading (`Heat.fi`).
pub fn item_keys(stem: &str) -> Vec<MatchKey> {
    let base = match_key(stem);
    let mut keys = Vec::with_capacity(2);
    for aggressive in [true, false] {
        let key = MatchKey {
            title: strip_trailing_language(base.title.clone(), aggressive),
            episode: base.episode,
        };
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

/// True when a name only says "which language" (`English`, `2_English`, `fi`).
pub fn is_generic_name(stem: &str) -> bool {
    let tokens = tokens(stem);
    !tokens.is_empty()
        && tokens.iter().all(|t| {
            let t = t.to_lowercase();
            t.chars().all(|c| c.is_ascii_digit()) || is_flag_token(&t) || find_language(&t).is_some()
        })
}

fn is_prefix(shorter: &[String], longer: &[String]) -> bool {
    longer.len() >= shorter.len() && longer[..shorter.len()] == *shorter
}

fn titles_compatible_movie(a: &[String], b: &[String]) -> bool {
    let (shorter, longer) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    !shorter.is_empty()
        && is_prefix(shorter, longer)
        && longer[shorter.len()..].iter().all(|t| is_year(t))
}

fn titles_compatible_episode(a: &[String], b: &[String]) -> bool {
    let (shorter, longer) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    shorter.is_empty() || is_prefix(shorter, longer)
}

/// Do two keys describe the same media?
///
/// Movies need equal titles (an extra year is tolerated); episodes need the
/// same season and episode plus a compatible title. `Alien` never matches `Aliens`.
pub fn keys_match(video: &MatchKey, item: &MatchKey) -> bool {
    match (video.episode, item.episode) {
        (Some(a), Some(b)) => a == b && titles_compatible_episode(&video.title, &item.title),
        (None, None) => titles_compatible_movie(&video.title, &item.title),
        _ => false,
    }
}

/// Stem-level convenience used by tests and callers without keys.
pub fn fingerprints_match(left: &str, right: &str) -> bool {
    let left = match_key(left);
    right_matches(&left, right)
}

fn right_matches(left: &MatchKey, right: &str) -> bool {
    item_keys(right).iter().any(|k| keys_match(left, k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_release_metadata() {
        assert_eq!(
            clean_stem_for_matching("Riot.2015.1080p.BluRay.x264.[YTS.AG]"),
            "Riot.2015"
        );
        assert_eq!(
            clean_stem_for_matching("Riot.2015.NORDIC.1080p.BluRay.x264.AC3-DAWGS"),
            "Riot.2015"
        );
        assert_eq!(
            clean_stem_for_matching("Movie.2020.2160p.WEB-DL.DDP5.1.H.264-FLUX"),
            "Movie.2020"
        );
        assert_eq!(clean_stem_for_matching("Movie-1080p-BluRay"), "Movie");
        assert_eq!(clean_stem_for_matching("Spider-Man.2002.720p"), "Spider-Man.2002");
    }

    #[test]
    fn year_is_not_confused_with_resolution() {
        assert!(is_year("1999") && is_year("2049"));
        assert!(!is_year("1080") && !is_year("2160") && !is_year("3000") && !is_year("999"));
    }

    #[test]
    fn parses_episode_tags() {
        let e = parse_episode("Show.Name.S01E05.720p").unwrap();
        assert_eq!((e.season, e.episode, e.start), (1, 5, 10));
        let e = parse_episode("Show 2x07").unwrap();
        assert_eq!((e.season, e.episode), (2, 7));
        assert_eq!(parse_episode("Show.S3.E4").map(|e| (e.season, e.episode)), Some((3, 4)));
        assert!(parse_episode("Movie.1920x1080").is_none());
        assert!(parse_episode("Movie.x264.2015").is_none());
    }

    #[test]
    fn matches_different_release_groups() {
        assert!(fingerprints_match(
            "Riot.2015.1080p.BluRay.x264-YTS",
            "Riot.2015.720p.WEB-DL.DAWGS-fin"
        ));
        assert!(fingerprints_match("Riot.2015.1080p", "Riot.fin"));
        assert!(fingerprints_match("Heat", "Heat.fi"));
        assert!(fingerprints_match("Dr.No.1962", "Dr.No.1962"));
        assert!(fingerprints_match("Dr.No", "Dr.No"));
    }

    #[test]
    fn does_not_match_similar_names() {
        assert!(!fingerprints_match("Alien", "Aliens"));
        assert!(!fingerprints_match("Alien.mkv", "Aliens.eng.srt"));
        assert!(!fingerprints_match("It.2017", "It.Follows.2014"));
        assert!(!fingerprints_match("It.mkv", "Italy.eng.srt"));
        assert!(!fingerprints_match("Movie.2020.mkv", "Other.Movie.2020.srt"));
        assert!(!fingerprints_match("Riot.2015", "Riot.2016"));
    }

    #[test]
    fn episodes_need_same_episode_and_compatible_title() {
        assert!(fingerprints_match("Show.S01E02.1080p", "Show.S01E02.fin"));
        assert!(fingerprints_match("Show.S01E02", "S01E02.fin"));
        assert!(!fingerprints_match("Show.S01E02", "Show.S01E03.fin"));
        assert!(!fingerprints_match("Show.S01E02", "Other.Show.S01E02.fin"));
        assert!(!fingerprints_match("Show.S01E02", "Show.2015.fin"));
    }

    #[test]
    fn generic_names_are_recognised() {
        for name in ["English", "2_English", "fi", "3_Finnish.forced"] {
            assert!(is_generic_name(name), "{name}");
        }
        assert!(!is_generic_name("Movie.English"));
        assert!(!is_generic_name(""));
    }
}
