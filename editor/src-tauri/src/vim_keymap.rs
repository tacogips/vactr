use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use tauri::Manager;

pub const FILE_NAME: &str = "vim-keymap.json";
pub const MAX_BYTES: usize = 262_144;
const TEMP_FILE_NAME: &str = "vim-keymap.json.tmp";

/// Checks the storage-level shape of a version 1 keymap document.
pub fn validate(text: &str) -> Result<(), String> {
    if text.len() > MAX_BYTES {
        return Err("keymap is too large".to_owned());
    }

    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "keymap is not a JSON object".to_owned())?;
    let Some(object) = value.as_object() else {
        return Err("keymap is not a JSON object".to_owned());
    };
    if object.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err("keymap version must be 1".to_owned());
    }
    Ok(())
}

/// Reads a valid keymap document without deleting invalid or unreadable files.
pub fn read_keymap(dir: &Path) -> Option<String> {
    let content = fs::read_to_string(dir.join(FILE_NAME)).ok()?;
    validate(&content).ok()?;
    Some(content)
}

/// Atomically stores the exact JSON text after checking its storage-level shape.
pub fn write_keymap(dir: &Path, text: &str) -> Result<(), String> {
    validate(text)?;
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;

    let temporary = dir.join(TEMP_FILE_NAME);
    let target = dir.join(FILE_NAME);
    let write_result = fs::write(&temporary, text)
        .map_err(|error| error.to_string())
        .and_then(|()| fs::rename(&temporary, &target).map_err(|error| error.to_string()));
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

/// Removes the stored keymap; removing an already absent file succeeds.
pub fn remove_keymap(dir: &Path) -> Result<(), String> {
    match fs::remove_file(dir.join(FILE_NAME)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub fn vim_keymap_get(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    Ok(read_keymap(&dir))
}

#[tauri::command]
pub fn vim_keymap_set(app: tauri::AppHandle, text: String) -> Result<(), String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    write_keymap(&dir, &text)
}

#[tauri::command]
pub fn vim_keymap_reset(app: tauri::AppHandle) -> Result<(), String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    remove_keymap(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new() -> Self {
            let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("vactr-vim-keymap-{}-{id}", std::process::id()));
            fs::create_dir_all(&dir).expect("create test directory");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn valid_document_is_stored_exactly_and_temp_file_is_removed() {
        let dir = TestDir::new();
        let text = "{\"version\":1,\"maps\":{}}\n";
        write_keymap(dir.path(), text).expect("write keymap");
        assert_eq!(read_keymap(dir.path()).as_deref(), Some(text));
        assert!(!dir.path().join(TEMP_FILE_NAME).exists());
    }

    #[test]
    fn oversized_document_is_rejected_without_creating_files() {
        let dir = TestDir::new();
        let text = format!(
            "{{\"version\":1,\"padding\":\"{}\"}}",
            "x".repeat(MAX_BYTES)
        );
        assert_eq!(validate(&text), Err("keymap is too large".to_owned()));
        assert_eq!(
            write_keymap(dir.path(), &text),
            Err("keymap is too large".to_owned())
        );
        assert!(!dir.path().join(FILE_NAME).exists());
        assert!(!dir.path().join(TEMP_FILE_NAME).exists());
    }

    #[test]
    fn non_object_json_roots_are_rejected() {
        for text in ["[]", "\"x\"", "null", "not json"] {
            assert_eq!(
                validate(text),
                Err("keymap is not a JSON object".to_owned())
            );
        }
    }

    #[test]
    fn missing_and_non_integer_versions_are_rejected() {
        for text in [
            "{}",
            "{\"version\":2}",
            "{\"version\":\"1\"}",
            "{\"version\":1.0}",
        ] {
            assert_eq!(validate(text), Err("keymap version must be 1".to_owned()));
        }
    }

    #[test]
    fn invalid_document_is_not_written() {
        let dir = TestDir::new();
        assert!(write_keymap(dir.path(), "[]").is_err());
        assert!(!dir.path().join(FILE_NAME).exists());
    }

    #[test]
    fn invalid_persisted_document_is_ignored_without_deletion() {
        let dir = TestDir::new();
        let path = dir.path().join(FILE_NAME);
        fs::write(&path, "{\"version\":2}").expect("write invalid document");
        assert_eq!(read_keymap(dir.path()), None);
        assert!(path.exists());
    }

    #[test]
    fn missing_file_reads_as_none() {
        let dir = TestDir::new();
        assert_eq!(read_keymap(dir.path()), None);
    }

    #[test]
    fn removing_keymap_is_idempotent() {
        let dir = TestDir::new();
        write_keymap(dir.path(), "{\"version\":1}").expect("write keymap");
        remove_keymap(dir.path()).expect("remove keymap");
        assert!(!dir.path().join(FILE_NAME).exists());
        remove_keymap(dir.path()).expect("remove missing keymap");
    }
}
