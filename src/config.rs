use crate::lang::find_language;
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// Every field is optional in `config.toml`; missing ones fall back to `Default`.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
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

fn exts(list: &[&str]) -> Vec<String> {
    list.iter().map(|e| e.to_string()).collect()
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
            ext_video: exts(&[".mp4", ".mkv", ".avi", ".mov", ".ts", ".m2ts", ".wmv", ".webm"]),
            ext_sub: exts(&[".srt", ".ass", ".ssa", ".vtt", ".sup"]),
            ext_audio: exts(&[".aac", ".mp3", ".m4a", ".flac", ".wav", ".ac3", ".eac3", ".dts", ".mka"]),
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
        if find_language(&self.default_sub_lang).is_none() {
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

    /// Canonical form: ISO 639-2 language code and lowercase extensions.
    pub fn normalize(&mut self) {
        if let Some(language) = find_language(&self.default_sub_lang) {
            self.default_sub_lang = language.iso3.to_string();
        }
        for list in [&mut self.ext_video, &mut self.ext_sub, &mut self.ext_audio] {
            for ext in list.iter_mut() {
                *ext = ext.to_ascii_lowercase();
            }
        }
    }
}

/// Prints a red error and exits.
pub fn fail(message: impl Display) -> ! {
    println!("\n{} {}", style("❌").red(), style(message).red());
    std::process::exit(1);
}

/// Command line options. Everything else lives in `config.toml`.
#[derive(Debug, Default, PartialEq)]
pub struct CliOptions {
    pub config: Option<PathBuf>,
    pub dry_run: bool,
    /// Non-interactive: no pauses and no "Press Enter".
    pub yes: bool,
    pub help: bool,
    pub version: bool,
}

pub const USAGE: &str = "\
Usage: mkv_pro_manager [OPTIONS]

Options:
      --config <PATH>  Use this config.toml instead of the default one
      --dry-run        Show what would be done without writing any files
  -y, --yes            Do not pause or wait for Enter (for scripts)
  -h, --help           Show this help
  -V, --version        Show the version";

pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<CliOptions, String> {
    let mut options = CliOptions::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dry-run" => options.dry_run = true,
            "-y" | "--yes" => options.yes = true,
            "-h" | "--help" => options.help = true,
            "-V" | "--version" => options.version = true,
            "--config" => {
                let path = args.next().ok_or("--config needs a path")?;
                options.config = Some(PathBuf::from(path));
            }
            other => match other.strip_prefix("--config=") {
                Some(path) => options.config = Some(PathBuf::from(path)),
                None => return Err(format!("unknown option: {other}")),
            },
        }
    }
    Ok(options)
}

/// Config location: `--config`, else `config.toml` in the working directory
/// (as before), else next to the executable.
pub fn get_config_path(cli: &CliOptions) -> PathBuf {
    if let Some(path) = &cli.config {
        return path.clone();
    }
    let local = PathBuf::from("config.toml");
    if local.exists() {
        return local;
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("config.toml")))
        .unwrap_or(local)
}

/// Looks for mkvmerge: the configured path, then `PATH`, then the usual
/// MKVToolNix install folders.
pub fn find_mkvmerge(configured: &Path) -> Option<PathBuf> {
    if configured.is_file() {
        return Some(configured.to_path_buf());
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join("mkvmerge.exe"));
            candidates.push(dir.join("mkvmerge"));
        }
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(dir) = std::env::var_os(var) {
            candidates.push(Path::new(&dir).join("MKVToolNix").join("mkvmerge.exe"));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Relative folders in a config file are relative to that file, so a
/// double-clicked exe and a shortcut with another working directory agree.
fn resolve_relative(config: &mut Config, config_path: &Path) {
    let Some(base) = config_path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return;
    };
    for path in [&mut config.root_folder, &mut config.output_root] {
        if path.is_relative() {
            *path = base.join(&path);
        }
    }
}

fn ask<T>(result: dialoguer::Result<T>) -> T {
    result.unwrap_or_else(|e| {
        fail(format!(
            "Setup needs an interactive terminal ({e}). Create config.toml by hand or use --config."
        ))
    })
}

/// Loads `config.toml`, running the setup wizard on first start.
/// Returns the config together with the path it lives at.
pub fn load_config(cli: &CliOptions) -> (Config, PathBuf) {
    let config_path = get_config_path(cli);

    let mut config = if config_path.exists() {
        match fs::read_to_string(&config_path) {
            Ok(content) => match toml::from_str::<Config>(&content) {
                Ok(cfg) => cfg,
                Err(e) => {
                    println!("{}", style("Please check your config.toml or delete it to run the wizard again.").dim());
                    fail(format!("Error parsing {}: {e}", config_path.display()));
                }
            },
            Err(e) => fail(format!("Error reading {}: {e}", config_path.display())),
        }
    } else if cli.config.is_some() || !std::io::stdin().is_terminal() {
        fail(format!(
            "{} not found. Create it, or run without --config in a terminal for the setup wizard.",
            config_path.display()
        ));
    } else {
        run_wizard(&config_path)
    };

    config.normalize();
    if let Err(e) = config.validate() {
        fail(format!("Invalid configuration: {e}"));
    }
    if cli.dry_run {
        config.dry_run = true;
    }
    resolve_relative(&mut config, &config_path);
    (config, config_path)
}

fn run_wizard(config_path: &Path) -> Config {
    println!();
    println!(
        "{}",
        style("⚡ Welcome to MKV Pro Manager Setup").bold().cyan()
    );
    println!("{}", style("Let's configure your environment.").dim());
    println!();

    let theme = ColorfulTheme::default();
    let default = Config::default();
    let detected_mkvmerge =
        find_mkvmerge(&default.mkvmerge_path).unwrap_or_else(|| default.mkvmerge_path.clone());

    let root_folder: String = ask(Input::with_theme(&theme)
        .with_prompt("Source Video Directory")
        .default(default.root_folder.to_string_lossy().to_string())
        .interact_text());

    let output_root: String = ask(Input::with_theme(&theme)
        .with_prompt("Output Directory")
        .default(default.output_root.to_string_lossy().to_string())
        .interact_text());

    let mkvmerge_path: String = ask(Input::with_theme(&theme)
        .with_prompt("Path to mkvmerge.exe")
        .default(detected_mkvmerge.to_string_lossy().to_string())
        .interact_text());

    let default_sub_lang: String = ask(Input::with_theme(&theme)
        .with_prompt("Default Subtitle Language (3-letter ISO, e.g., fin, eng)")
        .default(default.default_sub_lang.clone())
        .validate_with(|input: &String| -> Result<(), String> {
            find_language(input)
                .map(|_| ())
                .ok_or_else(|| format!("unsupported language: {input}"))
        })
        .interact_text());

    let dry_run = ask(Confirm::with_theme(&theme)
        .with_prompt("Dry run first? (show what would happen, change nothing)")
        .default(default.dry_run)
        .interact());

    let delete_originals = ask(Confirm::with_theme(&theme)
        .with_prompt("Delete original files after successful merge?")
        .default(default.delete_originals)
        .interact());

    let concurrent_jobs: usize = ask(Input::with_theme(&theme)
        .with_prompt("Concurrent Jobs (Parallel Processing)")
        .default(default.concurrent_jobs)
        .interact());

    let new_config = Config {
        root_folder: PathBuf::from(root_folder),
        output_root: PathBuf::from(output_root),
        mkvmerge_path: PathBuf::from(mkvmerge_path),
        default_sub_lang,
        dry_run,
        delete_originals,
        concurrent_jobs,
        ..default
    };

    let mut checked = new_config.clone();
    checked.normalize();
    if let Err(e) = checked.validate() {
        fail(format!("Invalid configuration: {e}"));
    }

    match toml::to_string_pretty(&checked) {
        Ok(toml_str) => match fs::write(config_path, toml_str) {
            Ok(()) => println!(
                "{}",
                style(format!("✔ Configuration saved to {}", config_path.display())).green()
            ),
            Err(e) => println!(
                "{} could not save {}: {e}",
                style("!").yellow(),
                config_path.display()
            ),
        },
        Err(e) => println!("{} could not serialize the configuration: {e}", style("!").yellow()),
    }
    new_config
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

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
    }

    #[test]
    fn readme_example_config_parses() {
        let toml = r#"
root_folder = "C:\\Downloads\\Incoming"
output_root = "D:\\Media\\Library"
mkvmerge_path = "C:\\Program Files\\MKVToolNix\\mkvmerge.exe"
concurrent_jobs = 4 # How many files to merge at once
default_sub_lang = "fin" # Preferred subtitle language (ISO 639-3)
delete_originals = false # Delete source files after success?
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.concurrent_jobs, 4);
        assert!(!config.dry_run);
        assert!(!config.ext_video.is_empty());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn empty_config_falls_back_to_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.concurrent_jobs, Config::default().concurrent_jobs);
    }

    #[test]
    fn normalize_canonicalizes_language_and_extensions() {
        let mut config = Config {
            default_sub_lang: "FI".to_string(),
            ext_video: vec![".MKV".to_string()],
            ..Config::default()
        };
        config.normalize();
        assert_eq!(config.default_sub_lang, "fin");
        assert_eq!(config.ext_video, [".mkv"]);
    }

    #[test]
    fn parses_command_line_flags() {
        let options = parse_args(args(&["--dry-run", "-y", "--config", "x.toml"])).unwrap();
        assert!(options.dry_run && options.yes);
        assert_eq!(options.config, Some(PathBuf::from("x.toml")));
        let options = parse_args(args(&["--config=y.toml"])).unwrap();
        assert_eq!(options.config, Some(PathBuf::from("y.toml")));
        assert!(parse_args(args(&["--nope"])).is_err());
        assert!(parse_args(args(&["--config"])).is_err());
    }

    #[test]
    fn relative_folders_resolve_against_the_config_file() {
        let mut config = Config {
            output_root: PathBuf::from("out"),
            root_folder: PathBuf::from(r"C:\abs\in"),
            ..Config::default()
        };
        resolve_relative(&mut config, Path::new(r"C:\tools\config.toml"));
        assert_eq!(config.output_root, Path::new(r"C:\tools").join("out"));
        assert_eq!(config.root_folder, PathBuf::from(r"C:\abs\in"));

        let mut config = Config::default();
        resolve_relative(&mut config, Path::new("config.toml"));
        assert_eq!(config.output_root, PathBuf::from("MKV_output"));
    }
}
