//! On-device storage for the tuning library.
//!
//! The library directory is configured once at start-up. On Android
//! `android_main` points it at the app's private data directory; on the host it
//! falls back to `./tunings` so the same code can be exercised in tests.

use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use tuner_core::tuning_file::{TuningFile, FILE_EXTENSION};

fn dir_cell() -> &'static RwLock<Option<PathBuf>> {
    static DIR: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
    DIR.get_or_init(|| RwLock::new(None))
}

/// Set the directory used to store tuning files.
pub fn set_library_dir(path: impl Into<PathBuf>) {
    if let Ok(mut dir) = dir_cell().write() {
        *dir = Some(path.into());
    }
}

/// Directory currently used to store tuning files.
pub fn library_dir() -> PathBuf {
    dir_cell()
        .read()
        .ok()
        .and_then(|d| d.clone())
        .unwrap_or_else(|| PathBuf::from("tunings"))
}

/// Turn a user supplied tuning name into a safe file stem.
///
/// Everything other than alphanumerics, spaces, `-` and `_` is dropped, which
/// also prevents path traversal (`..`, `/`) from user input.
pub fn sanitize_file_stem(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "tuning".to_string()
    } else {
        trimmed.chars().take(64).collect()
    }
}

/// Full path a tuning with this name would be written to.
pub fn path_for(name: &str) -> PathBuf {
    library_dir().join(format!("{}.{}", sanitize_file_stem(name), FILE_EXTENSION))
}

/// Save a tuning file into the library directory, returning the path written.
pub fn save(file: &TuningFile) -> Result<PathBuf, String> {
    let dir = library_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;
    let path = path_for(&file.name);
    file.save_to_path(&path)?;
    Ok(path)
}

/// Delete a stored tuning by name. Missing files are not an error.
pub fn delete(name: &str) -> Result<(), String> {
    let path = path_for(name);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Failed to delete {}: {e}", path.display())),
    }
}

/// Load every valid tuning file from the library directory.
///
/// Returns the loaded tunings plus a list of per-file error messages so the UI
/// can report corrupt files without failing the whole load.
pub fn load_all() -> (Vec<TuningFile>, Vec<String>) {
    let dir = library_dir();
    let mut files = Vec::new();
    let mut errors = Vec::new();

    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (files, errors),
        Err(e) => {
            errors.push(format!("Failed to list {}: {e}", dir.display()));
            return (files, errors);
        }
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| has_tuning_extension(p))
        .collect();
    paths.sort();

    for path in paths {
        match TuningFile::load_from_path(&path) {
            Ok(file) => files.push(file),
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }

    (files, errors)
}

fn has_tuning_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case(FILE_EXTENSION))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The library directory is global, so storage tests run in one test fn.
    #[test]
    fn test_library_roundtrip_and_sanitizing() {
        assert_eq!(sanitize_file_stem("Steinway B"), "Steinway B");
        assert_eq!(sanitize_file_stem("../../etc/passwd"), "______etc_passwd");
        assert_eq!(sanitize_file_stem("   "), "tuning");
        assert_eq!(sanitize_file_stem(""), "tuning");
        assert!(!sanitize_file_stem(&"x".repeat(200)).is_empty());
        assert_eq!(sanitize_file_stem(&"x".repeat(200)).len(), 64);

        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("storage-test");
        let _ = std::fs::remove_dir_all(&dir);
        set_library_dir(&dir);
        assert_eq!(library_dir(), dir);

        // Empty (missing) directory loads cleanly.
        let (files, errors) = load_all();
        assert!(files.is_empty() && errors.is_empty());

        let mut file = TuningFile::new("Test Grand");
        file.a4_reference_hz = 441.0;
        let path = save(&file).unwrap();
        assert!(path.ends_with("Test Grand.ptun"));

        // A corrupt file is reported but does not break loading of valid ones.
        std::fs::write(dir.join("broken.ptun"), "{ not json").unwrap();
        std::fs::write(dir.join("ignored.txt"), "not a tuning").unwrap();
        let (files, errors) = load_all();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "Test Grand");
        assert_eq!(errors.len(), 1);

        // Path traversal in the name cannot escape the library directory.
        let mut evil = TuningFile::new("../escape");
        evil.notes = "nope".into();
        let evil_path = save(&evil).unwrap();
        assert_eq!(evil_path.parent(), Some(dir.as_path()));

        delete("Test Grand").unwrap();
        delete("Test Grand").unwrap(); // deleting twice is fine
        let (files, _) = load_all();
        assert!(files.iter().all(|f| f.name != "Test Grand"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
