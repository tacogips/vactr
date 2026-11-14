use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DefaultTemplate {
    Example { id: String },
    Snapshot { name: String, text: String },
}

#[derive(Serialize, Deserialize)]
struct StoredTemplate {
    version: u8,
    template: DefaultTemplate,
}

pub const FILE_NAME: &str = "default-template.json";
pub const MAX_TEXT_BYTES: usize = 1_048_576;
const TEMP_FILE_NAME: &str = "default-template.json.tmp";

pub fn validate(template: &DefaultTemplate) -> Result<(), String> {
    match template {
        DefaultTemplate::Example { id } => {
            let mut chars = id.chars();
            let valid_first = chars
                .next()
                .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
            if !valid_first
                || id.chars().count() > 96
                || !chars
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '/' || c == '-')
            {
                return Err("invalid example id".to_owned());
            }
        }
        DefaultTemplate::Snapshot { name, text } => {
            let name_len = name.chars().count();
            if !(1..=120).contains(&name_len) || name.chars().any(char::is_control) {
                return Err("invalid template name".to_owned());
            }
            if text.len() > MAX_TEXT_BYTES {
                return Err("template text is too large".to_owned());
            }
        }
    }
    Ok(())
}

pub fn read_template(dir: &Path) -> Option<DefaultTemplate> {
    let content = fs::read_to_string(dir.join(FILE_NAME)).ok()?;
    let stored: StoredTemplate = serde_json::from_str(&content).ok()?;
    if stored.version != 1 || validate(&stored.template).is_err() {
        return None;
    }
    Some(stored.template)
}

pub fn write_template(dir: &Path, template: &DefaultTemplate) -> Result<(), String> {
    validate(template)?;
    fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let content = serde_json::to_vec(&StoredTemplate {
        version: 1,
        template: template.clone(),
    })
    .map_err(|error| error.to_string())?;
    let temporary = dir.join(TEMP_FILE_NAME);
    let target = dir.join(FILE_NAME);

    let write_result = fs::write(&temporary, content)
        .map_err(|error| error.to_string())
        .and_then(|()| fs::rename(&temporary, &target).map_err(|error| error.to_string()));
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

pub fn remove_template(dir: &Path) -> Result<(), String> {
    match fs::remove_file(dir.join(FILE_NAME)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub fn default_template_get(app: tauri::AppHandle) -> Result<Option<DefaultTemplate>, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    Ok(read_template(&dir))
}

#[tauri::command]
pub fn default_template_set(
    app: tauri::AppHandle,
    template: DefaultTemplate,
) -> Result<(), String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    write_template(&dir, &template)
}

#[tauri::command]
pub fn default_template_reset(app: tauri::AppHandle) -> Result<(), String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    remove_template(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("vactr-template-{}-{id}", std::process::id()));
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
    fn example_persists_across_fresh_read() {
        let dir = TestDir::new();
        let template = DefaultTemplate::Example {
            id: "acid-techno".to_owned(),
        };
        write_template(dir.path(), &template).expect("write example template");
        assert_eq!(read_template(dir.path()), Some(template));
    }

    #[test]
    fn snapshot_persists_and_temporary_file_is_removed() {
        let dir = TestDir::new();
        let template = DefaultTemplate::Snapshot {
            name: "My song".to_owned(),
            text: "use-bpm 128".to_owned(),
        };
        write_template(dir.path(), &template).expect("write snapshot template");
        assert_eq!(read_template(dir.path()), Some(template));
        assert!(!dir.path().join(TEMP_FILE_NAME).exists());
    }

    #[test]
    fn missing_corrupt_and_foreign_version_values_are_ignored_without_deletion() {
        let dir = TestDir::new();
        assert_eq!(read_template(dir.path()), None);

        let path = dir.path().join(FILE_NAME);
        fs::write(&path, "not json").expect("write corrupt file");
        assert_eq!(read_template(dir.path()), None);
        assert!(path.exists());

        fs::write(
            &path,
            r#"{"version":2,"template":{"kind":"example","id":"x"}}"#,
        )
        .expect("write future version");
        assert_eq!(read_template(dir.path()), None);
        assert!(path.exists());
    }

    #[test]
    fn invalid_example_ids_are_rejected_without_writing() {
        let dir = TestDir::new();
        for id in ["../x".to_owned(), "A".to_owned(), "a".repeat(97)] {
            assert!(write_template(dir.path(), &DefaultTemplate::Example { id }).is_err());
            assert!(!dir.path().join(FILE_NAME).exists());
        }
    }

    #[test]
    fn invalid_snapshot_name_and_oversized_text_are_rejected() {
        let dir = TestDir::new();
        for name in [String::new(), "n".repeat(121), "bad\nname".to_owned()] {
            assert!(write_template(
                dir.path(),
                &DefaultTemplate::Snapshot {
                    name,
                    text: String::new(),
                }
            )
            .is_err());
        }
        assert!(write_template(
            dir.path(),
            &DefaultTemplate::Snapshot {
                name: "valid".to_owned(),
                text: "x".repeat(MAX_TEXT_BYTES + 1),
            }
        )
        .is_err());
        assert!(!dir.path().join(FILE_NAME).exists());
    }

    #[test]
    fn removing_template_is_idempotent() {
        let dir = TestDir::new();
        write_template(
            dir.path(),
            &DefaultTemplate::Example {
                id: "showcase".to_owned(),
            },
        )
        .expect("write template");
        remove_template(dir.path()).expect("remove template");
        assert!(!dir.path().join(FILE_NAME).exists());
        remove_template(dir.path()).expect("remove missing template");
    }

    #[test]
    fn snapshot_tag_deserializes() {
        let template: DefaultTemplate =
            serde_json::from_str(r#"{"kind":"snapshot","name":"n","text":"t"}"#)
                .expect("deserialize snapshot");
        assert_eq!(
            template,
            DefaultTemplate::Snapshot {
                name: "n".to_owned(),
                text: "t".to_owned(),
            }
        );
    }
}
