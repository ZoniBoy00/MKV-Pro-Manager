use crate::config::Config;
use crate::naming::{is_generic_name, item_keys, keys_match, match_key, MatchKey};
use std::fs;
use std::path::{Path, PathBuf};

pub use crate::naming::{
    clean_stem_for_matching, fingerprints_match, get_fingerprint,
};

#[derive(Debug, Default)]
pub struct FoundAssets {
    pub subtitles: Vec<PathBuf>,
    pub audios: Vec<PathBuf>,
}

#[derive(Clone, Copy, PartialEq)]
enum AssetKind {
    Subtitle,
    Audio,
}

enum Origin {
    /// Next to the video.
    Adjacent,
    /// Directly inside `Subs/` or `Subtitles/`.
    SubsRoot,
    /// Inside `Subs/<name>/` (the folder name identifies the video).
    NamedFolder(MatchKey),
}

struct Candidate {
    path: PathBuf,
    kind: AssetKind,
    keys: Vec<MatchKey>,
    generic: bool,
    origin: Origin,
}

/// Everything in one video directory that could be a subtitle or audio track.
/// Built once per directory so a season folder is not rescanned per episode.
pub struct DirIndex {
    video_count: usize,
    candidates: Vec<Candidate>,
}

fn extension_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_ascii_lowercase()))
}

fn is_subs_folder(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "subs" | "subtitles")
}

fn candidate(path: PathBuf, config: &Config, origin: Origin) -> Option<Candidate> {
    let ext = extension_of(&path)?;
    let kind = if config.ext_sub.contains(&ext) {
        AssetKind::Subtitle
    } else if config.ext_audio.contains(&ext) {
        AssetKind::Audio
    } else {
        return None;
    };
    let stem = path.file_stem()?.to_str()?.to_string();
    Some(Candidate {
        keys: item_keys(&stem),
        generic: is_generic_name(&stem),
        path,
        kind,
        origin,
    })
}

impl DirIndex {
    pub fn build(dir: &Path, config: &Config) -> Self {
        let mut index = DirIndex {
            video_count: 0,
            candidates: Vec::new(),
        };
        let Ok(entries) = fs::read_dir(dir) else {
            return index;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if extension_of(&path).is_some_and(|e| config.ext_video.contains(&e)) {
                    index.video_count += 1;
                } else if let Some(c) = candidate(path, config, Origin::Adjacent) {
                    index.candidates.push(c);
                }
            } else if path.is_dir()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(is_subs_folder)
            {
                index.add_subs_folder(&path, config);
            }
        }

        index.candidates.sort_by(|a, b| a.path.cmp(&b.path));
        index
    }

    fn add_subs_folder(&mut self, folder: &Path, config: &Config) {
        let Ok(entries) = fs::read_dir(folder) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(c) = candidate(path, config, Origin::SubsRoot) {
                    self.candidates.push(c);
                }
            } else if path.is_dir() {
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                let folder_key = match_key(name);
                let Ok(inner) = fs::read_dir(&path) else {
                    continue;
                };
                for file in inner.flatten() {
                    let file = file.path();
                    if file.is_file() {
                        if let Some(c) =
                            candidate(file, config, Origin::NamedFolder(folder_key.clone()))
                        {
                            self.candidates.push(c);
                        }
                    }
                }
            }
        }
    }

    /// Subtitles and audio tracks that belong to `video_path`.
    ///
    /// A file belongs to the video when its cleaned name matches, when it sits
    /// in a `Subs/<video name>/` folder, or when it is a generic
    /// `English.srt`-style file and the video is the only one in the folder.
    pub fn assets_for(&self, video_path: &Path) -> FoundAssets {
        let mut assets = FoundAssets::default();
        let Some(stem) = video_path.file_stem().and_then(|s| s.to_str()) else {
            return assets;
        };
        let video_key = match_key(stem);
        if video_key.title.is_empty() {
            return assets;
        }

        for c in &self.candidates {
            if c.path == video_path {
                continue;
            }
            let by_name = c.keys.iter().any(|k| keys_match(&video_key, k));
            let by_folder = match &c.origin {
                Origin::NamedFolder(folder_key) => keys_match(&video_key, folder_key),
                _ => false,
            };
            let by_generic = c.generic && self.video_count == 1;

            if by_name || by_folder || by_generic {
                match c.kind {
                    AssetKind::Subtitle => assets.subtitles.push(c.path.clone()),
                    AssetKind::Audio => assets.audios.push(c.path.clone()),
                }
            }
        }
        assets
    }
}

/// Scans the directory of the given video file for matching subtitles and audio tracks.
pub fn find_matching_assets(video_path: &Path, config: &Config) -> FoundAssets {
    match video_path.parent() {
        Some(parent) => DirIndex::build(parent, config).assets_for(video_path),
        None => FoundAssets::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Temporary directory tree that is removed on drop.
    struct Tree(PathBuf);

    impl Tree {
        fn new(label: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let dir = std::env::temp_dir().join(format!("mkvpm-{label}-{nanos}"));
            fs::create_dir_all(&dir).unwrap();
            Tree(dir)
        }

        fn file(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"x").unwrap();
            path
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn names(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn similar_titles_do_not_steal_each_others_subtitles() {
        let tree = Tree::new("alien");
        let alien = tree.file("Alien.1979.mkv");
        tree.file("Aliens.1986.eng.srt");
        tree.file("Alien.1979.fin.srt");
        let assets = find_matching_assets(&alien, &Config::default());
        assert_eq!(names(&assets.subtitles), ["Alien.1979.fin.srt"]);
    }

    #[test]
    fn each_episode_gets_only_its_own_files() {
        let tree = Tree::new("episodes");
        let e1 = tree.file("Show.S01E01.1080p.mkv");
        let e2 = tree.file("Show.S01E02.1080p.mkv");
        tree.file("Show.S01E01.fin.srt");
        tree.file("Show.S01E02.fin.srt");
        tree.file("Show.S01E02.eng.ac3");
        let index = DirIndex::build(e1.parent().unwrap(), &Config::default());
        assert_eq!(names(&index.assets_for(&e1).subtitles), ["Show.S01E01.fin.srt"]);
        let second = index.assets_for(&e2);
        assert_eq!(names(&second.subtitles), ["Show.S01E02.fin.srt"]);
    }

    #[test]
    fn finds_subs_in_a_folder_named_after_the_video() {
        let tree = Tree::new("subsfolder");
        let video = tree.file("Show.S01E01.1080p.mkv");
        tree.file("Show.S01E02.1080p.mkv");
        tree.file("Subs/Show.S01E01.1080p/2_English.srt");
        tree.file("Subs/Show.S01E02.1080p/2_English.srt");
        let assets = find_matching_assets(&video, &Config::default());
        assert_eq!(assets.subtitles.len(), 1);
        assert!(assets.subtitles[0].to_string_lossy().contains("S01E01"));
    }

    #[test]
    fn generic_subs_need_a_single_video() {
        let single = Tree::new("single");
        let movie = single.file("Movie.2015.mkv");
        single.file("Subs/2_English.srt");
        single.file("Subs/3_Finnish.srt");
        let assets = find_matching_assets(&movie, &Config::default());
        assert_eq!(assets.subtitles.len(), 2);

        let many = Tree::new("many");
        let movie = many.file("Movie.2015.mkv");
        many.file("Other.2016.mkv");
        many.file("Subs/2_English.srt");
        assert!(find_matching_assets(&movie, &Config::default())
            .subtitles
            .is_empty());
    }
}
