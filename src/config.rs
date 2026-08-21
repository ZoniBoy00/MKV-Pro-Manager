use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub root_folder: PathBuf,
    pub output_root: PathBuf,
    pub mkvmerge_path: PathBuf,
    pub dry_run: bool,
    pub delete_originals: bool,
    pub default_sub_lang: String,
    pub ext_video: Vec<String>,
    pub ext_sub: Vec<String>,
    pub ext_audio: Vec<String>,
    pub concurrent_jobs: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root_folder: PathBuf::from(r"C:\MKV-Pro-Manager\input"),
            output_root: PathBuf::from("MKV_output"),
            mkvmerge_path: PathBuf::from(r"C:\Program Files\MKVToolNix\mkvmerge.exe"),
            dry_run: false,
            delete_originals: false,
            default_sub_lang: "fin".to_string(),
            ext_video: vec![".mp4".into(), ".mkv".into(), ".avi".into(), ".mov".into()],
            ext_sub: vec![".srt".into(), ".ass".into(), ".ssa".into(), ".vtt".into()],
            ext_audio: vec![
                ".aac".into(),
                ".mp3".into(),
                ".m4a".into(),
                ".flac".into(),
                ".wav".into(),
            ],
            concurrent_jobs: 2,
        }
    }
}

impl Config {
    /// Validate values that could otherwise make processing unsafe or panic.
    pub fn validate(&self) -> Result<(), String> {
        if self.concurrent_jobs == 0 || self.concurrent_jobs > 64 {
            return Err("concurrent_jobs must be between 1 and 64".to_string());
        }
        if self.default_sub_lang.trim().is_empty() {
            return Err("default_sub_lang must not be empty".to_string());
        }
        let default_sub_lang = self.default_sub_lang.to_lowercase();
        if !LANG_DATA
            .values()
            .any(|(iso3, _)| *iso3 == default_sub_lang)
        {
            return Err(format!(
                "unsupported default_sub_lang: {}",
                self.default_sub_lang
            ));
        }
        for (name, extensions) in [
            ("video", &self.ext_video),
            ("subtitle", &self.ext_sub),
            ("audio", &self.ext_audio),
        ] {
            if extensions
                .iter()
                .any(|ext| !ext.starts_with('.') || ext.len() == 1)
            {
                return Err(format!("{} extensions must start with a dot", name));
            }
        }
        Ok(())
    }
}

pub static LANG_DATA: Lazy<HashMap<&'static str, (&'static str, &'static str)>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("fi", ("fin", "Finnish"));
    m.insert("en", ("eng", "English"));
    m.insert("sv", ("swe", "Swedish"));
    m.insert("de", ("ger", "German"));
    m.insert("fr", ("fre", "French"));
    m.insert("es", ("spa", "Spanish"));
    m.insert("no", ("nor", "Norwegian"));
    m.insert("da", ("dan", "Danish"));
    m.insert("ru", ("rus", "Russian"));
    m.insert("it", ("ita", "Italian"));
    m.insert("ja", ("jpn", "Japanese"));
    m.insert("zh-cn", ("chi", "Chinese"));
    m.insert("ko", ("kor", "Korean"));
    m.insert("pl", ("pol", "Polish"));
    m.insert("pt", ("por", "Portuguese"));
    m
});

pub fn get_config_path() -> PathBuf {
    PathBuf::from("config.toml")
}

pub fn load_config_interactive() -> Config {
    let config_path = get_config_path();

    // Attempt to load existing
    if config_path.exists() {
        match fs::read_to_string(&config_path) {
            Ok(content) => {
                match toml::from_str::<Config>(&content) {
                    Ok(cfg) => {
                        if let Err(e) = cfg.validate() {
                            println!(
                                "{} Invalid configuration: {}",
                                style("❌").red(),
                                style(e).red()
                            );
                            std::process::exit(1);
                        }
                        return cfg;
                    }
                    Err(e) => {
                        println!(
                            "\n{} {} {}",
                            style("❌").red(),
                            style("Error parsing config.toml:").red().bold(),
                            style(e).red()
                        );
                        println!("{}", style("Please check your config.toml or delete it to run the wizard again.").dim());
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                println!(
                    "\n{} {} {}",
                    style("❌").red(),
                    style("Error reading config.toml:").red().bold(),
                    style(e).red()
                );
                std::process::exit(1);
            }
        }
    }

    // Wizard
    println!();
    println!(
        "{}",
        style("⚡ Welcome to MKV Pro Manager Setup").bold().cyan()
    );
    println!("{}", style("Let's configure your environment.").dim());
    println!();

    let theme = ColorfulTheme::default();
    let default = Config::default();

    let root_folder: String = Input::with_theme(&theme)
        .with_prompt("Source Video Directory")
        .default(default.root_folder.to_string_lossy().to_string())
        .interact_text()
        .unwrap();

    let output_root: String = Input::with_theme(&theme)
        .with_prompt("Output Directory")
        .default(default.output_root.to_string_lossy().to_string())
        .interact_text()
        .unwrap();

    let mkvmerge_path: String = Input::with_theme(&theme)
        .with_prompt("Path to mkvmerge.exe")
        .default(default.mkvmerge_path.to_string_lossy().to_string())
        .interact_text()
        .unwrap();

    let default_sub_lang: String = Input::with_theme(&theme)
        .with_prompt("Default Subtitle Language (3-letter ISO, e.g., fin, eng)")
        .default(default.default_sub_lang)
        .interact_text()
        .unwrap();

    let delete_originals = Confirm::with_theme(&theme)
        .with_prompt("Delete original files after successful merge?")
        .default(default.delete_originals)
        .interact()
        .unwrap();

    let concurrent_jobs: usize = Input::with_theme(&theme)
        .with_prompt("Concurrent Jobs (Parallel Processing)")
        .default(default.concurrent_jobs)
        .interact()
        .unwrap();

    let new_config = Config {
        root_folder: PathBuf::from(root_folder),
        output_root: PathBuf::from(output_root),
        mkvmerge_path: PathBuf::from(mkvmerge_path),
        default_sub_lang: default_sub_lang.to_lowercase(),
        delete_originals,
        concurrent_jobs,
        ..default
    };

    if let Err(e) = new_config.validate() {
        println!(
            "{} Invalid configuration: {}",
            style("❌").red(),
            style(e).red()
        );
        std::process::exit(1);
    }

    // Save
    if let Ok(toml_str) = toml::to_string_pretty(&new_config) {
        let _ = fs::write(&config_path, toml_str);
        println!("{}", style("✔ Configuration saved to config.toml").green());
    }

    new_config
}

#[cfg(test)]
mod tests {
    use super::{Config, LANG_DATA};

    #[test]
    fn default_config_is_valid() {
        assert!(Config::default().validate().is_ok());
    }

    #[test]
    fn zero_workers_are_rejected() {
        let config = Config {
            concurrent_jobs: 0,
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn unknown_subtitle_language_is_rejected() {
        let config = Config {
            default_sub_lang: "zzz".to_string(),
            ..Config::default()
        };
        assert!(config.validate().is_err());
        assert!(!LANG_DATA.is_empty());
    }
}
