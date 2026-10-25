use chardetng::EncodingDetector;
use encoding_rs::{
    Encoding, EUC_KR, GBK, SHIFT_JIS, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252,
};
use std::path::Path;
use whatlang::detect;

/// Subtitle formats that are plain text (and therefore have a charset).
/// Binary formats (`.sup`, `.sub`/`.idx` VobSub) must never get `--sub-charset`.
const TEXT_SUB_EXTS: &[&str] = &["srt", "ass", "ssa", "vtt"];

/// Subtitles larger than this are not read for content analysis.
const MAX_SUB_BYTES: u64 = 8 * 1024 * 1024;

/// Amount of dialogue text fed to the language detector.
const SAMPLE_CHARS: usize = 1500;

#[derive(Debug)]
pub struct Language {
    pub iso1: &'static str,
    pub iso3: &'static str,
    /// Extra codes/aliases accepted in filenames and config (lowercase).
    pub alt: &'static [&'static str],
    pub name: &'static str,
    /// Code returned by `whatlang` (ISO 639-3, not always equal to `iso3`).
    pub whatlang: &'static str,
    /// Legacy codepage used by old subtitle files in this language.
    pub legacy: &'static Encoding,
}

pub static LANGUAGES: [Language; 15] = [
    Language { iso1: "fi", iso3: "fin", alt: &["suomi"], name: "Finnish", whatlang: "fin", legacy: WINDOWS_1252 },
    Language { iso1: "en", iso3: "eng", alt: &[], name: "English", whatlang: "eng", legacy: WINDOWS_1252 },
    Language { iso1: "sv", iso3: "swe", alt: &["svenska"], name: "Swedish", whatlang: "swe", legacy: WINDOWS_1252 },
    Language { iso1: "de", iso3: "ger", alt: &["deu"], name: "German", whatlang: "deu", legacy: WINDOWS_1252 },
    Language { iso1: "fr", iso3: "fre", alt: &["fra"], name: "French", whatlang: "fra", legacy: WINDOWS_1252 },
    Language { iso1: "es", iso3: "spa", alt: &[], name: "Spanish", whatlang: "spa", legacy: WINDOWS_1252 },
    Language { iso1: "no", iso3: "nor", alt: &["nob", "nno", "nb"], name: "Norwegian", whatlang: "nob", legacy: WINDOWS_1252 },
    Language { iso1: "da", iso3: "dan", alt: &[], name: "Danish", whatlang: "dan", legacy: WINDOWS_1252 },
    Language { iso1: "ru", iso3: "rus", alt: &[], name: "Russian", whatlang: "rus", legacy: WINDOWS_1251 },
    Language { iso1: "it", iso3: "ita", alt: &[], name: "Italian", whatlang: "ita", legacy: WINDOWS_1252 },
    Language { iso1: "ja", iso3: "jpn", alt: &[], name: "Japanese", whatlang: "jpn", legacy: SHIFT_JIS },
    Language { iso1: "zh", iso3: "chi", alt: &["zho", "cmn", "chs", "cht", "zh-cn"], name: "Chinese", whatlang: "cmn", legacy: GBK },
    Language { iso1: "ko", iso3: "kor", alt: &[], name: "Korean", whatlang: "kor", legacy: EUC_KR },
    Language { iso1: "pl", iso3: "pol", alt: &[], name: "Polish", whatlang: "pol", legacy: WINDOWS_1250 },
    Language { iso1: "pt", iso3: "por", alt: &[], name: "Portuguese", whatlang: "por", legacy: WINDOWS_1252 },
];

/// Finds a language by ISO 639-1/-2/-3 code, alias or English name (case-insensitive).
pub fn find_language(code: &str) -> Option<&'static Language> {
    let code = code.trim().to_lowercase();
    if code.is_empty() {
        return None;
    }
    LANGUAGES.iter().find(|l| {
        l.iso1 == code
            || l.iso3 == code
            || l.alt.contains(&code.as_str())
            || l.name.eq_ignore_ascii_case(&code)
    })
}

fn language_from_whatlang(code: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.whatlang == code)
}

/// Tokens that describe a subtitle variant rather than the title.
pub fn is_flag_token(token: &str) -> bool {
    matches!(token, "forced" | "sdh" | "cc" | "hi")
}

/// Splits a name into lowercase alphanumeric tokens.
fn split_tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NameTags {
    pub language: Option<&'static Language>,
    pub forced: bool,
    pub sdh: bool,
}

/// Reads language and variant flags from a file *stem* (`Movie.2015.fin.forced`).
///
/// Whole tokens are compared, so `Dead` or `Nobody` never read as `de`/`no`.
/// A two-letter code only counts when it is the last meaningful token, and
/// the first token is only considered when it is the only non-numeric one
/// (`English.srt`, `2_English.srt`).
pub fn tags_from_name(stem: &str) -> NameTags {
    let tokens = split_tokens(stem);
    let mut tags = NameTags::default();

    let non_numeric = tokens
        .iter()
        .filter(|t| !t.chars().all(|c| c.is_ascii_digit()))
        .count();
    let last_meaningful = tokens.iter().rposition(|t| !is_flag_token(t));

    for (i, token) in tokens.iter().enumerate() {
        if i > 0 && is_flag_token(token) {
            match token.as_str() {
                "forced" => tags.forced = true,
                _ => tags.sdh = true,
            }
        }
    }

    for i in (0..tokens.len()).rev() {
        let token = &tokens[i];
        if (i > 0 && is_flag_token(token)) || (i == 0 && non_numeric != 1) {
            continue;
        }
        if token.len() == 2 && Some(i) != last_meaningful {
            continue;
        }
        if let Some(language) = find_language(token) {
            tags.language = Some(language);
            break;
        }
    }
    tags
}

pub struct SubtitleInfo {
    pub iso3: &'static str,
    pub name: String,
    pub forced: bool,
    pub sdh: bool,
    /// Charset to pass to `--sub-charset`; `None` for binary formats and BOM files.
    pub charset: Option<&'static str>,
}

/// Decodes subtitle bytes. Returns the text and the charset mkvmerge should be told about.
///
/// UTF-8 is reported explicitly (mkvmerge otherwise falls back to the system
/// locale); BOM files are left to mkvmerge; anything else is a legacy codepage.
pub fn decode_subtitle(bytes: &[u8], hint: Option<&Language>) -> (String, Option<&'static str>) {
    if let Some((encoding, bom_len)) = Encoding::for_bom(bytes) {
        let (text, _) = encoding.decode_without_bom_handling(&bytes[bom_len..]);
        return (text.into_owned(), None);
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return (text.to_owned(), Some("UTF-8"));
    }
    let encoding = legacy_encoding(bytes, hint);
    let (text, _) = encoding.decode_without_bom_handling(bytes);
    (text.into_owned(), Some(encoding.name()))
}

fn legacy_encoding(bytes: &[u8], hint: Option<&Language>) -> &'static Encoding {
    if let Some(language) = hint {
        let (_, had_errors) = language.legacy.decode_without_bom_handling(bytes);
        if !had_errors {
            return language.legacy;
        }
    }
    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    detector.guess(None, false)
}

/// Picks dialogue lines from SRT/VTT/ASS text for language detection.
fn dialogue_sample(text: &str) -> String {
    let mut sample = String::new();
    let mut chars = 0;

    for line in text.lines() {
        let line = line.trim();
        let dialogue = match line.strip_prefix("Dialogue:") {
            Some(rest) => rest.splitn(10, ',').nth(9).unwrap_or(""),
            None => line,
        };
        if dialogue.contains("-->") || dialogue.starts_with("WEBVTT") {
            continue;
        }

        let mut clean = String::with_capacity(dialogue.len());
        let mut depth = 0u32;
        for c in dialogue.replace("\\N", " ").chars() {
            match c {
                '<' | '{' => depth += 1,
                '>' | '}' => depth = depth.saturating_sub(1),
                _ if depth == 0 => clean.push(c),
                _ => {}
            }
        }

        if clean.chars().filter(|c| c.is_alphabetic()).count() > 5 {
            chars += clean.chars().count();
            sample.push_str(&clean);
            sample.push(' ');
        }
        if chars > SAMPLE_CHARS {
            break;
        }
    }
    sample
}

fn detect_language_from_text(text: &str) -> Option<&'static Language> {
    let sample = dialogue_sample(text);
    if sample.trim().is_empty() {
        return None;
    }
    detect(&sample).and_then(|info| language_from_whatlang(info.lang().code()))
}

/// Works out language, flags and charset of a subtitle file.
pub fn analyze_subtitle(path: &Path) -> SubtitleInfo {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let tags = tags_from_name(stem);

    let mut charset = None;
    let mut language = tags.language;

    if TEXT_SUB_EXTS.contains(&ext.as_str()) {
        let readable = std::fs::metadata(path).is_ok_and(|m| m.len() <= MAX_SUB_BYTES);
        if let (true, Ok(bytes)) = (readable, std::fs::read(path)) {
            let (text, detected_charset) = decode_subtitle(&bytes, tags.language);
            charset = detected_charset;
            if language.is_none() {
                language = detect_language_from_text(&text);
            }
        }
    }

    let (iso3, base_name) = match language {
        Some(l) => (l.iso3, l.name),
        None => ("und", "Undefined"),
    };
    let mut name = base_name.to_string();
    if tags.forced {
        name.push_str(" (Forced)");
    }
    if tags.sdh {
        name.push_str(" (SDH)");
    }

    SubtitleInfo {
        iso3,
        name,
        forced: tags.forced,
        sdh: tags.sdh,
        charset,
    }
}

/// Language of an external audio file, from its name only.
pub fn audio_language(path: &Path) -> (&'static str, &'static str) {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    match tags_from_name(stem).language {
        Some(l) => (l.iso3, l.name),
        None => ("und", "Undefined"),
    }
}

/// Index of the subtitle that should get the default flag: the first regular
/// (non-forced, non-SDH) track in the preferred language.
pub fn choose_default(infos: &[SubtitleInfo], default_iso3: &str) -> Option<usize> {
    infos
        .iter()
        .position(|i| i.iso3 == default_iso3 && !i.forced && !i.sdh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iso(stem: &str) -> Option<&'static str> {
        tags_from_name(stem).language.map(|l| l.iso3)
    }

    #[test]
    fn finds_languages_by_any_code_or_name() {
        for code in ["fi", "FIN", "Finnish", "suomi"] {
            assert_eq!(find_language(code).unwrap().iso3, "fin");
        }
        assert_eq!(find_language("deu").unwrap().iso3, "ger");
        assert_eq!(find_language("zh-cn").unwrap().iso3, "chi");
        assert!(find_language("zzz").is_none());
        assert!(find_language("").is_none());
    }

    #[test]
    fn whatlang_codes_map_to_every_language() {
        for language in LANGUAGES.iter() {
            let found = language_from_whatlang(language.whatlang).unwrap();
            assert_eq!(found.iso3, language.iso3);
            assert!(whatlang::Lang::from_code(language.whatlang).is_some());
        }
    }

    #[test]
    fn name_tags_use_whole_tokens() {
        assert_eq!(iso("Movie.2015.fin"), Some("fin"));
        assert_eq!(iso("Movie.2015.fi"), Some("fin"));
        assert_eq!(iso("Movie_eng"), Some("eng"));
        assert_eq!(iso("Movie-sv"), Some("swe"));
        assert_eq!(iso("2_English"), Some("eng"));
        assert_eq!(iso("English"), Some("eng"));
    }

    #[test]
    fn name_tags_ignore_words_that_merely_contain_a_code() {
        assert_eq!(iso("The.Dead.Zone.2005"), None);
        assert_eq!(iso("Nobody.2021"), None);
        assert_eq!(iso("The.Player.1992"), None);
        assert_eq!(iso("No.Country.For.Old.Men"), None);
        assert_eq!(iso("Dan.in.Real.Life"), None);
    }

    #[test]
    fn flags_are_detected_and_do_not_hide_the_language() {
        let tags = tags_from_name("Movie.2015.fin.forced");
        assert_eq!(tags.language.unwrap().iso3, "fin");
        assert!(tags.forced && !tags.sdh);
        let tags = tags_from_name("Movie.eng.SDH");
        assert_eq!(tags.language.unwrap().iso3, "eng");
        assert!(tags.sdh);
    }

    #[test]
    fn utf8_is_reported_explicitly() {
        let (text, charset) = decode_subtitle("Hyvää päivää".as_bytes(), None);
        assert_eq!(text, "Hyvää päivää");
        assert_eq!(charset, Some("UTF-8"));
    }

    #[test]
    fn bom_files_are_left_to_mkvmerge() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("Hyvää".as_bytes());
        let (text, charset) = decode_subtitle(&bytes, None);
        assert_eq!(text, "Hyvää");
        assert_eq!(charset, None);
    }

    #[test]
    fn windows_1252_finnish_is_decoded_correctly() {
        // "Hyvää yötä" in windows-1252: ä = 0xE4, ö = 0xF6
        let bytes = [b'H', b'y', b'v', 0xE4, 0xE4, b' ', b'y', 0xF6, b't', 0xE4];
        let fi = find_language("fi");
        let (text, charset) = decode_subtitle(&bytes, fi);
        assert_eq!(text, "Hyvää yötä");
        assert_eq!(charset, Some("windows-1252"));
    }

    #[test]
    fn legacy_codepage_follows_the_language_hint() {
        // "Привет" in windows-1251
        let bytes = [0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2];
        let (text, charset) = decode_subtitle(&bytes, find_language("ru"));
        assert_eq!(text, "Привет");
        assert_eq!(charset, Some("windows-1251"));
    }

    #[test]
    fn content_detection_covers_languages_with_different_codes() {
        let cases = [
            ("ger", "Ich habe gestern einen langen Spaziergang durch den Wald gemacht und dabei viele Vögel gesehen."),
            ("fre", "Hier, j'ai fait une longue promenade dans la forêt et j'ai vu beaucoup d'oiseaux magnifiques."),
            ("nor", "I går gikk jeg en lang tur gjennom skogen og så mange vakre fugler som sang i trærne."),
            ("chi", "昨天我在森林里散了很长时间的步，看到了许多美丽的小鸟在树上唱歌，心情非常愉快。"),
            ("fin", "Eilen kävelin pitkään metsässä ja näin monta kaunista lintua, jotka lauloivat puissa."),
        ];
        for (expected, text) in cases {
            let language = detect_language_from_text(text)
                .unwrap_or_else(|| panic!("no language detected for {expected}"));
            assert_eq!(language.iso3, expected);
        }
    }

    #[test]
    fn dialogue_sample_skips_timings_and_markup() {
        let srt = "1\n00:00:01,000 --> 00:00:03,000\n<i>Hello there, friend</i>\n";
        assert_eq!(dialogue_sample(srt).trim(), "Hello there, friend");
        let ass = "Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,{\\an8}Hello there\\Nfriend";
        assert_eq!(dialogue_sample(ass).trim(), "Hello there friend");
    }

    #[test]
    fn only_a_regular_track_becomes_default() {
        let info = |iso3, forced, sdh| SubtitleInfo {
            iso3,
            name: String::new(),
            forced,
            sdh,
            charset: None,
        };
        let infos = [
            info("fin", true, false),
            info("eng", false, false),
            info("fin", false, true),
            info("fin", false, false),
            info("fin", false, false),
        ];
        assert_eq!(choose_default(&infos, "fin"), Some(3));
        assert_eq!(choose_default(&infos, "swe"), None);
    }
}
