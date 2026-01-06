use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use whatlang::detect;
use crate::config::LANG_DATA;

pub struct LangDetectResult {
    pub iso: String,
    pub name: String,
}

pub fn detect_subtitle_language(path: &Path) -> LangDetectResult {
    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Priority 1: Check filename tags
    for (iso1, (iso3, name)) in LANG_DATA.iter() {
        let patterns = [
            format!(".{}.", iso1),
            format!("_{}.", iso1),
            format!(".{}.", iso3),
            format!("_{}.", iso3),
        ];

        for pat in patterns.iter() {
            if filename.contains(pat) {
                return LangDetectResult {
                    iso: iso3.to_string(),
                    name: name.to_string(),
                };
            }
        }
    }

    // Priority 2: Content analysis
    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        let mut sample_text = String::new();
        let mut total_chars = 0;

        for line in reader.lines() {
            if let Ok(l) = line {
                let clean_line: String = l.chars().filter(|c| c.is_alphabetic()).collect();
                if clean_line.len() > 5 {
                    sample_text.push_str(&l);
                    sample_text.push(' ');
                    total_chars += l.len();
                }
            }
            if total_chars > 1500 {
                break;
            }
        }

        if !sample_text.trim().is_empty() {
            if let Some(info) = detect(&sample_text) {
                let code = info.lang().code(); 
                
                if let Some((iso3, name)) = LANG_DATA.get(code) {
                    return LangDetectResult {
                        iso: iso3.to_string(),
                        name: name.to_string(),
                    };
                }

                for (_, (iso3, name)) in LANG_DATA.iter() {
                    if *iso3 == code {
                        return LangDetectResult {
                            iso: iso3.to_string(),
                            name: name.to_string(),
                        };
                    }
                }
            }
        }
    }

    LangDetectResult {
        iso: "und".to_string(),
        name: "Undefined".to_string(),
    }
}
