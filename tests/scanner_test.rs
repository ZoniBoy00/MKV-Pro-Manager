use mkv_pro_manager::scanner::{clean_stem_for_matching, fingerprints_match};

#[test]
fn cleans_release_metadata_from_real_scanner_code() {
    assert_eq!(
        clean_stem_for_matching("Riot.2015.1080p.BluRay.x264-YTS"),
        "Riot.2015"
    );
}

#[test]
fn matches_different_release_groups() {
    assert!(fingerprints_match(
        "Riot.2015.1080p.BluRay.x264-YTS",
        "Riot.2015.720p.WEB-DL.DAWGS-fin"
    ));
}

#[test]
fn does_not_match_unrelated_or_tiny_names() {
    assert!(!fingerprints_match("It.mkv", "Italy.eng.srt"));
    assert!(!fingerprints_match(
        "Movie.2020.mkv",
        "Other.Movie.2020.srt"
    ));
}
