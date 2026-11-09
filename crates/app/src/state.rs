use std::path::Path;

use lt_core::PaneLayout;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiState {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
    pub sidebar: bool,
    pub open_sessions: Vec<Uuid>,
    pub tabs: Vec<PaneLayout>,
    pub broadcast: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            width: 1180,
            height: 720,
            maximized: false,
            sidebar: true,
            open_sessions: Vec::new(),
            tabs: Vec::new(),
            broadcast: false,
        }
    }
}

pub fn load(path: &Path) -> UiState {
    lt_core::store::read_json(path).unwrap_or_default()
}

pub fn save(path: &Path, state: &UiState) {
    if let Err(error) = lt_core::store::write_json_atomic(path, state) {
        tracing::warn!(%error, "falha ao salvar o estado da janela");
    }
}
