//! Integration test: every bundled example tuning file must load and validate.

use std::path::PathBuf;

use tuner_core::tuning_file::TuningFile;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
}

#[test]
fn example_files_load_and_validate() {
    let dir = examples_dir();
    let mut count = 0;

    for entry in std::fs::read_dir(&dir).expect("examples directory should exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("ptun") {
            continue;
        }
        let file = TuningFile::load_from_path(&path)
            .unwrap_or_else(|e| panic!("{} failed to load: {e}", path.display()));

        assert_eq!(
            file.tuning_curve_cents.len(),
            88,
            "{} must have 88 curve values",
            path.display()
        );
        assert!(
            file.tuning_curve_cents[48].abs() < 1e-9,
            "{} should be anchored at A4",
            path.display()
        );
        assert!(
            !file.inharmonicity_by_key().is_empty(),
            "{} should carry inharmonicity data",
            path.display()
        );
        count += 1;
    }

    assert_eq!(count, 3, "expected three example tuning files in {dir:?}");
}
