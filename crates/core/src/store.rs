use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::model::{Store, STORE_VERSION};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("não foi possível determinar o diretório de configuração")]
    NoConfigDir,
    #[error("falha ao ler {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("falha ao interpretar {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("falha ao gravar {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    #[error("falha ao serializar: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub state: PathBuf,
}

impl Paths {
    pub fn discover() -> Result<Self, StoreError> {
        let config_dir = dirs::config_dir().ok_or(StoreError::NoConfigDir)?;
        let state_dir = dirs::state_dir()
            .or_else(dirs::data_local_dir)
            .ok_or(StoreError::NoConfigDir)?;
        Ok(Self {
            config: config_dir.join("like-terminal").join("sessions.json"),
            state: state_dir.join("like-terminal").join("state.json"),
        })
    }

    pub fn with_base(base: &Path) -> Self {
        Self {
            config: base.join("config").join("sessions.json"),
            state: base.join("state").join("state.json"),
        }
    }
}

pub fn load(path: &Path) -> Result<Store, StoreError> {
    if !path.exists() {
        return Ok(Store::default().normalized());
    }
    let data = fs::read_to_string(path).map_err(|source| StoreError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let store: Store = serde_json::from_str(&data).map_err(|source| StoreError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    let mut store = store.normalized();
    store.version = STORE_VERSION;
    Ok(store)
}

pub fn save(path: &Path, store: &Store) -> Result<(), StoreError> {
    write_json_atomic(path, store)
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), StoreError> {
    let parent = path.parent().ok_or(StoreError::NoConfigDir)?;
    fs::create_dir_all(parent).map_err(|source| StoreError::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    let json = serde_json::to_string_pretty(value)?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("store"),
        std::process::id()
    ));
    fs::write(&tmp, json).map_err(|source| StoreError::Write {
        path: tmp.clone(),
        source,
    })?;
    fs::rename(&tmp, path).map_err(|source| StoreError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Folder, Session};

    fn temp_base(name: &str) -> PathBuf {
        let base =
            std::env::temp_dir().join(format!("like-terminal-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        base
    }

    #[test]
    fn missing_file_loads_defaults() {
        let base = temp_base("missing");
        let paths = Paths::with_base(&base);
        let store = load(&paths.config).unwrap();
        assert_eq!(store.profiles.len(), 1);
        assert!(store.sessions.is_empty());
    }

    #[test]
    fn round_trip_keeps_data() {
        let base = temp_base("roundtrip");
        let paths = Paths::with_base(&base);

        let mut store = Store::default();
        let folder = Folder::new("Work", None);
        let folder_id = folder.id;
        store.folders.push(folder);
        let mut session = Session::new("m1scA-01", "10.0.0.21");
        session.folder = Some(folder_id);
        session.user = "admin".into();
        session.auth.save_password = true;
        let session_id = session.id;
        store.sessions.push(session);

        save(&paths.config, &store).unwrap();
        let loaded = load(&paths.config).unwrap();

        assert_eq!(loaded.folders.len(), 1);
        assert_eq!(loaded.sessions.len(), 1);
        assert_eq!(loaded.sessions[0].id, session_id);
        assert!(loaded.sessions[0].auth.save_password);
        assert_eq!(loaded.sessions[0].folder, Some(folder_id));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn save_is_atomic_and_leaves_no_temp_file() {
        let base = temp_base("atomic");
        let paths = Paths::with_base(&base);
        let store = Store::default();
        save(&paths.config, &store).unwrap();
        save(&paths.config, &store).unwrap();

        assert!(paths.config.exists());
        let leftovers: Vec<_> = fs::read_dir(paths.config.parent().unwrap())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn corrupted_file_is_reported() {
        let base = temp_base("corrupt");
        let paths = Paths::with_base(&base);
        fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
        fs::write(&paths.config, "{ not json").unwrap();
        assert!(matches!(load(&paths.config), Err(StoreError::Parse { .. })));
        let _ = fs::remove_dir_all(&base);
    }
}
