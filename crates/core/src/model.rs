use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const STORE_VERSION: u32 = 1;
pub const DEFAULT_PROFILE_ID: &str = "default";

fn default_version() -> u32 {
    STORE_VERSION
}

fn default_port() -> u16 {
    22
}

fn default_keepalive() -> u32 {
    30
}

fn default_scrollback() -> u32 {
    10_000
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub version: u32,
    pub folders: Vec<Folder>,
    pub sessions: Vec<Session>,
    pub profiles: Vec<Profile>,
    pub settings: Settings,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            version: default_version(),
            folders: Vec::new(),
            sessions: Vec::new(),
            profiles: vec![Profile::default()],
            settings: Settings::default(),
        }
    }
}

impl Store {
    pub fn normalized(mut self) -> Self {
        self.version = STORE_VERSION;
        if self.profiles.is_empty() {
            self.profiles.push(Profile::default());
        }
        if !self
            .profiles
            .iter()
            .any(|profile| profile.id == self.settings.default_profile)
        {
            self.settings.default_profile = self.profiles[0].id.clone();
        }
        for folder in &mut self.folders {
            folder.name = folder.name.trim().to_string();
        }
        self.folders.retain(|folder| !folder.name.is_empty());
        self
    }

    pub fn profile(&self, id: &str) -> &Profile {
        self.profiles
            .iter()
            .find(|profile| profile.id == id)
            .unwrap_or(&self.profiles[0])
    }

    pub fn default_profile(&self) -> &Profile {
        self.profile(&self.settings.default_profile)
    }

    pub fn session(&self, id: Uuid) -> Option<&Session> {
        self.sessions.iter().find(|session| session.id == id)
    }

    pub fn session_mut(&mut self, id: Uuid) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|session| session.id == id)
    }

    pub fn folder(&self, id: Uuid) -> Option<&Folder> {
        self.folders.iter().find(|folder| folder.id == id)
    }

    pub fn child_folders(&self, parent: Option<Uuid>) -> Vec<&Folder> {
        let mut folders: Vec<&Folder> = self
            .folders
            .iter()
            .filter(|folder| folder.parent == parent)
            .collect();
        folders.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.name.cmp(&b.name)));
        folders
    }

    pub fn child_sessions(&self, folder: Option<Uuid>) -> Vec<&Session> {
        let mut sessions: Vec<&Session> = self
            .sessions
            .iter()
            .filter(|session| session.folder == folder)
            .collect();
        sessions.sort_by_key(|session| session.name.to_lowercase());
        sessions
    }

    pub fn remove_folder_tree(&mut self, id: Uuid) {
        let mut pending = vec![id];
        let mut folders = Vec::new();
        while let Some(current) = pending.pop() {
            folders.push(current);
            pending.extend(
                self.folders
                    .iter()
                    .filter(|folder| folder.parent == Some(current))
                    .map(|folder| folder.id),
            );
        }
        self.folders.retain(|folder| !folders.contains(&folder.id));
        self.sessions
            .retain(|session| !session.folder.is_some_and(|f| folders.contains(&f)));
    }

    pub fn matching_sessions(&self, query: &str) -> Vec<&Session> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        self.sessions
            .iter()
            .filter(|session| {
                session.name.to_lowercase().contains(&query)
                    || session.host.to_lowercase().contains(&query)
                    || session.user.to_lowercase().contains(&query)
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub parent: Option<Uuid>,
    #[serde(default)]
    pub order: i32,
}

impl Folder {
    pub fn new(name: impl Into<String>, parent: Option<Uuid>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            parent,
            order: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub folder: Option<Uuid>,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub auth: AuthOptions,
    #[serde(default)]
    pub ssh: SshOptions,
    #[serde(default)]
    pub terminal: TerminalOptions,
    #[serde(default)]
    pub color: Option<String>,
}

impl Session {
    pub fn new(name: impl Into<String>, host: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            folder: None,
            host: host.into(),
            port: default_port(),
            user: String::new(),
            auth: AuthOptions::default(),
            ssh: SshOptions::default(),
            terminal: TerminalOptions::default(),
            color: None,
        }
    }

    pub fn target(&self) -> String {
        if self.user.is_empty() {
            self.host.clone()
        } else {
            format!("{}@{}", self.user, self.host)
        }
    }

    pub fn address(&self) -> String {
        if self.port == default_port() {
            self.target()
        } else {
            format!("{}:{}", self.target(), self.port)
        }
    }

    pub fn is_local(&self) -> bool {
        self.host.trim().is_empty()
    }

    pub fn subtitle(&self) -> String {
        if self.is_local() {
            return "Terminal local".to_string();
        }
        if self.user.is_empty() {
            format!("{}:{}", self.host, self.port)
        } else {
            format!("{}@{}:{}", self.user, self.host, self.port)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    #[default]
    Password,
    PublicKey,
    Agent,
    KeyboardInteractive,
}

impl AuthMethod {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Password => "Senha",
            Self::PublicKey => "Chave pública",
            Self::Agent => "Agente SSH",
            Self::KeyboardInteractive => "Teclado interativo",
        }
    }

    pub fn all() -> [Self; 4] {
        [
            Self::Password,
            Self::PublicKey,
            Self::Agent,
            Self::KeyboardInteractive,
        ]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthOptions {
    pub method: AuthMethod,
    pub key_path: Option<PathBuf>,
    pub save_password: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SshOptions {
    pub jump: Option<String>,
    pub compression: bool,
    pub agent_forward: bool,
    pub x11: bool,
    pub keepalive: u32,
    pub extra_args: Vec<String>,
}

impl Default for SshOptions {
    fn default() -> Self {
        Self {
            jump: None,
            compression: false,
            agent_forward: false,
            x11: false,
            keepalive: default_keepalive(),
            extra_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TerminalOptions {
    pub profile: String,
    pub font: Option<String>,
}

impl Default for TerminalOptions {
    fn default() -> Self {
        Self {
            profile: DEFAULT_PROFILE_ID.to_string(),
            font: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub font: String,
    pub palette: String,
    pub scrollback: u32,
    pub cursor: CursorShape,
    pub copy_on_select: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: DEFAULT_PROFILE_ID.to_string(),
            name: "Padrão".to_string(),
            font: "Monospace 11".to_string(),
            palette: "system".to_string(),
            scrollback: default_scrollback(),
            cursor: CursorShape::Block,
            copy_on_select: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CursorShape {
    #[default]
    Block,
    Ibeam,
    Underline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub confirm_close_running: bool,
    pub default_profile: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            confirm_close_running: default_true(),
            default_profile: DEFAULT_PROFILE_ID.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_store_has_profile() {
        let store = Store::default().normalized();
        assert_eq!(store.profiles.len(), 1);
        assert_eq!(store.settings.default_profile, DEFAULT_PROFILE_ID);
    }

    #[test]
    fn removing_folder_removes_descendants_and_sessions() {
        let mut store = Store::default();
        let parent = Folder::new("Work", None);
        let child = Folder::new("Lab", Some(parent.id));
        let parent_id = parent.id;
        let child_id = child.id;
        store.folders.push(parent);
        store.folders.push(child);

        let mut session = Session::new("m1scA-01", "10.0.0.21");
        session.folder = Some(child_id);
        store.sessions.push(session);
        let mut other = Session::new("local", "127.0.0.1");
        other.folder = None;
        store.sessions.push(other);

        store.remove_folder_tree(parent_id);
        assert!(store.folders.is_empty());
        assert_eq!(store.sessions.len(), 1);
        assert_eq!(store.sessions[0].name, "local");
    }

    #[test]
    fn session_address_omits_default_port() {
        let mut session = Session::new("m1", "host");
        session.user = "admin".into();
        assert_eq!(session.address(), "admin@host");
        session.port = 2222;
        assert_eq!(session.address(), "admin@host:2222");
    }

    #[test]
    fn children_are_sorted() {
        let mut store = Store::default();
        let folder = Folder::new("Work", None);
        let id = folder.id;
        store.folders.push(folder);
        for name in ["zeta", "Alpha", "beta"] {
            let mut session = Session::new(name, "host");
            session.folder = Some(id);
            store.sessions.push(session);
        }
        let names: Vec<String> = store
            .child_sessions(Some(id))
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert_eq!(names, vec!["Alpha", "beta", "zeta"]);
    }

    #[test]
    fn matching_sessions_filters_by_name_host_user() {
        let mut store = Store::default();
        let mut session = Session::new("m1scA-01", "10.0.0.21");
        session.user = "admin".into();
        store.sessions.push(session);
        assert_eq!(store.matching_sessions("m1sc").len(), 1);
        assert_eq!(store.matching_sessions("10.0").len(), 1);
        assert_eq!(store.matching_sessions("adm").len(), 1);
        assert_eq!(store.matching_sessions("nope").len(), 0);
    }
}
