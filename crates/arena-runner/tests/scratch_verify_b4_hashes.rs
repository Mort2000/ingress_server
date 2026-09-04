//! TEMPORARY verification harness (B1/B3 precedent; deleted before the
//! report commit): re-hashes every entry of a frozen B4 run archive's
//! hashes.txt against the on-disk artifacts, independently of the writer
//! code path. Run with:
//!   RUN_DIR=<run-dir> cargo test --release -p arena-runner --test scratch_verify_b4_hashes -- --ignored --nocapture

#[test]
#[ignore]
fn rehash_every_artifact_of_a_b4_run() {
    let dir = std::env::var("RUN_DIR").expect("pass RUN_DIR=<path>");
    let hashes =
        std::fs::read_to_string(std::path::Path::new(&dir).join("hashes.txt")).expect("hashes.txt");
    let mut checked = 0usize;
    for line in hashes.lines() {
        let (tagged, relative) = line.split_once("  ").expect("format");
        let hash = tagged.strip_prefix("blake3:").expect("blake3: prefix");
        let bytes = std::fs::read(std::path::Path::new(&dir).join(relative)).expect(relative);
        assert_eq!(
            arena_replay::canon::hex(blake3::hash(&bytes).as_bytes()),
            hash,
            "{relative} does not match its archived hash"
        );
        checked += 1;
    }
    println!("verified {checked} artifact hashes of {dir}");
    assert!(checked > 5, "a real archive has many artifacts");
}
