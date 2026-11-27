use std::fs;
use std::path::Path;

use pinwheel_d3_cover::model::word::parse_certificate;
use pinwheel_d3_cover::upper::enumerate::enumerate_boxes_pruned;
use pinwheel_d3_cover::upper::mincover::find_min_cover;
use pinwheel_d3_cover::upper::verify::verify_certificate;

fn check_certificate(rel_path: &str, expected_words: usize) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel_path);
    let content = fs::read_to_string(&path).expect("failed to read certificate file");
    let schedules = parse_certificate(&content).expect("failed to parse certificate");
    assert_eq!(schedules.len(), expected_words);

    let report = verify_certificate(&schedules).expect("certificate verification failed");
    assert_eq!(report.word_count, expected_words);
    assert!(
        report.redundant_words().is_empty(),
        "unexpected redundant words in {rel_path}: {:?}",
        report.redundant_words()
    );
}

#[test]
fn verify_12word_certificate() {
    check_certificate("proofs/D3_12words.txt", 12);
}

#[test]
fn verify_11word_certificate() {
    check_certificate("proofs/D3_11words.txt", 11);
}

/// Sum-pruned search over lengths <= 24 (~10-20 s in release mode on 10 cores).
/// Run with `cargo test --release -- --ignored`.
#[test]
#[ignore = "takes ~10-20 s in release mode"]
fn rediscover_11_words_from_pruned_len_24() {
    let words = enumerate_boxes_pruned(24, 19, 20);
    let res = find_min_cover(&words).expect("min cover search failed");
    assert_eq!(res.pool.items.len(), 233);
    assert_eq!(res.chosen.len(), 11);
    // Minimum covers are not unique, so check validity rather than equality with proofs/.
    let report = verify_certificate(&res.chosen).expect("rediscovered cover failed verification");
    assert!(report.redundant_words().is_empty());
}
