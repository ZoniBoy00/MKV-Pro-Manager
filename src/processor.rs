use std::path::Path;
use std::process::Command;
use std::fs;
use regex::Regex;
use crate::config::Config;
use crate::scanner::find_matching_assets;
use crate::lang::detect_subtitle_language;

pub struct Processor {
    config: Config,
    regex_series_standard: Regex,
    regex_series_x: Regex,
    regex_year: Regex,
    regex_season_only: Regex,
}

#[derive(Debug)]
struct MediaInfo {
    title: String,
    season_folder: String, 
    is_series: bool,
    year: Option<String>,
}

pub enum ProcessStatus {
    Success,
    Skipped,
    Failed(String),
}

impl Processor {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            regex_series_standard: Regex::new(r"(?i)^(.*?)[\. \-_]+s(\d+)[\. \-_]*e\d+").unwrap(),
            regex_series_x: Regex::new(r"(?i)^(.*?)[\. \-_]+(\d+)x\d+").unwrap(),
            regex_year: Regex::new(r"(?i)^(.*?)[\. \-_]+(\d{4})").unwrap(),
            regex_season_only: Regex::new(r"(?i)(?:season|s)[\. \-_]?(\d{1,2})").unwrap(),
        }
    }

    fn clean_title(&self, input: &str) -> String {
        input.replace(['.', '_'], " ")
            .split_whitespace()
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn parse_media_info(&self, path: &Path) -> MediaInfo {
        let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        
        if let Some(caps) = self.regex_series_standard.captures(filename) {
            let raw = caps.get(1).map_or("", |m| m.as_str());
            let season = caps.get(2).map_or("1", |m| m.as_str());
            
            let title = if raw.len() < 2 {
                self.infer_title_from_folder(path)
            } else {
                self.clean_title(raw)
            };

            return MediaInfo {
                title,
                season_folder: format!("Season {:0>2}", season),
                is_series: true,
                year: None,
            };
        }

        if let Some(caps) = self.regex_series_x.captures(filename) {
            let raw = caps.get(1).map_or("", |m| m.as_str());
            let season = caps.get(2).map_or("1", |m| m.as_str());
            
            return MediaInfo {
                title: self.clean_title(raw),
                season_folder: format!("Season {:0>2}", season),
                is_series: true,
                year: None,
            };
        }

        if let Some(caps) = self.regex_year.captures(filename) {
            let raw = caps.get(1).map_or("", |m| m.as_str());
            let year = caps.get(2).map_or("", |m| m.as_str());
            
            let title = if raw.is_empty() {
                filename.replace(year, "").trim().to_string()
            } else {
                self.clean_title(raw)
            };

            return MediaInfo {
                title,
                season_folder: String::new(),
                is_series: false,
                year: Some(year.to_string()),
            };
        }

        if let Some(parent) = path.parent() {
            let parent_name = parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Some(caps) = self.regex_season_only.captures(parent_name) {
                let season = caps.get(1).map_or("1", |m| m.as_str());
                return MediaInfo {
                    title: self.infer_title_from_folder(path),
                    season_folder: format!("Season {:0>2}", season),
                    is_series: true,
                    year: None,
                };
            }
        }

        MediaInfo {
            title: self.clean_title(filename),
            season_folder: String::new(),
            is_series: false,
            year: None,
        }
    }

    fn infer_title_from_folder(&self, path: &Path) -> String {
        let components: Vec<_> = path.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect();
        
        let len = components.len();
        if len < 3 { return "Unknown".to_string(); }
        
        let parent = &components[len - 2];
        if self.regex_season_only.is_match(parent) {
            self.clean_title(&components[len - 3])
        } else {
            self.clean_title(parent)
        }
    }

    pub fn process_file(&self, video_path: &Path) -> ProcessStatus {
        let meta = self.parse_media_info(video_path);
        
        let target_dir = if meta.is_series {
            self.config.output_root
                .join("TV Shows")
                .join(&meta.title)
                .join(&meta.season_folder)
        } else {
            let folder_name = if let Some(y) = meta.year {
                format!("{} ({})", meta.title, y)
            } else {
                meta.title.clone()
            };
            self.config.output_root
                .join("Movies")
                .join(folder_name)
        };

        let stem = video_path.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
        let output_file = target_dir.join(format!("{}.mkv", stem));

        if output_file.exists() {
            return ProcessStatus::Skipped;
        }

        let assets = find_matching_assets(video_path, &self.config);

        if self.config.dry_run {
            return ProcessStatus::Success; 
        }

        if let Err(e) = fs::create_dir_all(&target_dir) {
            return ProcessStatus::Failed(format!("Dir Create Error: {}", e));
        }

        let mut cmd = Command::new(&self.config.mkvmerge_path);
        cmd.arg("-o").arg(&output_file).arg(video_path);

        for sub in &assets.subtitles {
            let lang = detect_subtitle_language(sub);
            let is_default = if lang.iso == self.config.default_sub_lang { "1" } else { "0" };
            
            cmd.arg("--language").arg(format!("0:{}", lang.iso))
               .arg("--track-name").arg(format!("0:{}", lang.name))
               .arg("--default-track").arg(format!("0:{}", is_default))
               .arg(sub);
        }

        for audio in &assets.audios {
            cmd.arg("--language").arg("0:eng") 
               .arg("--track-name").arg("0:English")
               .arg(audio);
        }

        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                     if self.config.delete_originals {
                         let _ = fs::remove_file(video_path);
                         for s in &assets.subtitles { let _ = fs::remove_file(s); }
                         for a in &assets.audios { let _ = fs::remove_file(a); }
                     }
                     ProcessStatus::Success
                } else {
                    let err = String::from_utf8_lossy(&output.stderr);
                    ProcessStatus::Failed(err.trim().to_string())
                }
            }
            Err(e) => ProcessStatus::Failed(e.to_string()),
        }
    }
}
