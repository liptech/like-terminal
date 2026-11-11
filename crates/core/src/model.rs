use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const STORE_VERSION: u32 = 2;
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

fn default_bind_address() -> String {
    "127.0.0.1".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub version: u32,
    pub folders: Vec<Folder>,
    pub sessions: Vec<Session>,
    pub profiles: Vec<Profile>,
    pub settings: Settings,
    pub highlight_rules: Vec<HighlightRule>,
    pub highlight_ips: bool,
    pub snippets: Vec<Snippet>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            version: default_version(),
            folders: Vec::new(),
            sessions: Vec::new(),
            profiles: vec![Profile::default()],
            settings: Settings::default(),
            highlight_rules: default_highlight_rules(),
            highlight_ips: true,
            snippets: Vec::new(),
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

    pub fn snippet(&self, id: Uuid) -> Option<&Snippet> {
        self.snippets.iter().find(|snippet| snippet.id == id)
    }

    pub fn highlight_config(&self) -> HighlightConfig {
        let rules: Vec<String> = self
            .highlight_rules
            .iter()
            .filter(|rule| rule.problem().is_none())
            .map(HighlightRule::spec)
            .collect();
        HighlightConfig {
            spec: (!rules.is_empty()).then(|| rules.join(";")),
            ips: self.highlight_ips,
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialParity {
    #[default]
    None,
    Even,
    Odd,
}

impl SerialParity {
    pub fn letter(self) -> &'static str {
        match self {
            Self::None => "N",
            Self::Even => "E",
            Self::Odd => "O",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Even => "even",
            Self::Odd => "odd",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialFlow {
    #[default]
    None,
    Software,
    Hardware,
}

impl SerialFlow {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Software => "software",
            Self::Hardware => "hardware",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerialOptions {
    #[serde(default)]
    pub port: String,
    #[serde(default = "default_baud")]
    pub baud: u32,
    #[serde(default = "default_data_bits")]
    pub data_bits: u8,
    #[serde(default)]
    pub parity: SerialParity,
    #[serde(default = "default_stop_bits")]
    pub stop_bits: u8,
    #[serde(default)]
    pub flow: SerialFlow,
}

pub const SERIAL_BAUDS: &[u32] = &[
    300, 600, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
];

fn default_baud() -> u32 {
    9600
}

fn default_data_bits() -> u8 {
    8
}

fn default_stop_bits() -> u8 {
    1
}

impl Default for SerialOptions {
    fn default() -> Self {
        Self {
            port: String::new(),
            baud: default_baud(),
            data_bits: default_data_bits(),
            parity: SerialParity::None,
            stop_bits: default_stop_bits(),
            flow: SerialFlow::None,
        }
    }
}

impl SerialOptions {
    pub fn problem(&self) -> Option<&'static str> {
        let port = self.port.trim();
        if port.is_empty() {
            return Some("Informe a porta serial.");
        }
        if !port.starts_with('/') || port.chars().any(char::is_whitespace) {
            return Some("Use o caminho completo do dispositivo, como /dev/ttyUSB0.");
        }
        if !SERIAL_BAUDS.contains(&self.baud) {
            return Some("Selecione uma velocidade válida.");
        }
        if !(5..=8).contains(&self.data_bits) {
            return Some("Os bits de dados devem estar entre 5 e 8.");
        }
        if !(1..=2).contains(&self.stop_bits) {
            return Some("Os bits de parada devem ser 1 ou 2.");
        }
        None
    }

    pub fn summary(&self) -> String {
        format!(
            "{} · {} {}{}{}",
            self.port.trim(),
            self.baud,
            self.data_bits,
            self.parity.letter(),
            self.stop_bits
        )
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
    #[serde(default)]
    pub log_enabled: bool,
    #[serde(default)]
    pub log_dir: Option<String>,
    #[serde(default)]
    pub serial: Option<SerialOptions>,
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
            log_enabled: false,
            log_dir: None,
            serial: None,
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

    pub fn log_directory(&self) -> std::path::PathBuf {
        match self.log_dir.as_deref().filter(|dir| !dir.trim().is_empty()) {
            Some(dir) => std::path::PathBuf::from(dir),
            None => dirs::state_dir()
                .or_else(dirs::data_local_dir)
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("like-terminal")
                .join("logs"),
        }
    }

    pub fn log_path(&self) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let slug: String = self
            .name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let slug = slug.trim_matches('-');
        let slug = if slug.is_empty() { "sessao" } else { slug };
        self.log_directory().join(format!("{slug}-{stamp}.log"))
    }

    pub fn is_local(&self) -> bool {
        self.serial.is_none() && self.host.trim().is_empty()
    }

    pub fn is_serial(&self) -> bool {
        self.serial.is_some()
    }

    pub fn subtitle(&self) -> String {
        if let Some(serial) = &self.serial {
            return serial.summary();
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForwardKind {
    #[default]
    Local,
    Remote,
    Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardRule {
    pub kind: ForwardKind,
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    pub bind_port: u16,
    #[serde(default)]
    pub target_host: String,
    #[serde(default = "default_port")]
    pub target_port: u16,
}

impl Default for ForwardRule {
    fn default() -> Self {
        Self {
            kind: ForwardKind::Local,
            bind_address: default_bind_address(),
            bind_port: 0,
            target_host: String::new(),
            target_port: default_port(),
        }
    }
}

impl ForwardRule {
    pub fn problem(&self) -> Option<&'static str> {
        if self.bind_port == 0 {
            return Some("Informe a porta.");
        }
        if self.kind != ForwardKind::Dynamic && self.target_host.trim().is_empty() {
            return Some("Informe o host de destino.");
        }
        if self.kind != ForwardKind::Dynamic && self.target_port == 0 {
            return Some("Informe a porta de destino.");
        }
        if self
            .target_host
            .chars()
            .any(|c| c.is_whitespace() || c == '/' || c == '@')
        {
            return Some("Host de destino inválido.");
        }
        if self.bind_address.is_empty() {
            return Some("Informe o endereço de escuta.");
        }
        None
    }

    pub fn summary(&self) -> String {
        match self.kind {
            ForwardKind::Dynamic => format!("{}:{} (SOCKS)", self.bind_address, self.bind_port),
            _ => format!(
                "{}:{} → {}:{}",
                self.bind_address, self.bind_port, self.target_host, self.target_port
            ),
        }
    }
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
    pub forwards: Vec<ForwardRule>,
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
            forwards: Vec::new(),
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
pub struct Snippet {
    pub id: Uuid,
    pub name: String,
    pub text: String,
    #[serde(default = "default_true")]
    pub send_enter: bool,
}

impl Snippet {
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            text: text.into(),
            send_enter: true,
        }
    }

    pub fn problem(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            return Some("Informe um nome para o snippet.");
        }
        if self.text.trim().is_empty() {
            return Some("Informe o texto do snippet.");
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub font: String,
    pub palette: String,
    pub scrollback: u32,
    pub cursor: CursorShape,
    pub copy_on_select: bool,
    #[serde(default)]
    pub syntax_highlight: bool,
    #[serde(default)]
    pub highlight_output: bool,
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
            syntax_highlight: false,
            highlight_output: false,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HighlightRule {
    pub keyword: String,
    pub color: String,
    pub bold: bool,
    pub case_insensitive: bool,
}

impl Default for HighlightRule {
    fn default() -> Self {
        Self {
            keyword: String::new(),
            color: "#ff5f5f".to_string(),
            bold: false,
            case_insensitive: true,
        }
    }
}

impl HighlightRule {
    pub fn problem(&self) -> Option<&'static str> {
        let keyword = self.keyword.trim();
        if keyword.is_empty() {
            return Some("Informe a palavra-chave.");
        }
        if keyword.contains([';', '=']) {
            return Some("A palavra não pode conter ';' nem '='.");
        }
        if !is_hex_color(&self.color) {
            return Some("Cor inválida: use #rrggbb.");
        }
        None
    }

    pub fn spec(&self) -> String {
        let mut spec = format!("{}={}", self.keyword.trim(), self.color);
        if self.bold {
            spec.push_str(":bold");
        }
        if self.case_insensitive {
            spec.push_str(":nocase");
        }
        spec
    }
}

fn is_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit())
}

fn default_highlight_rules() -> Vec<HighlightRule> {
    [
        ("Error", "#ff5f5f", true),
        ("Failed", "#ff5f5f", true),
        ("Denied", "#ff5f5f", true),
        ("Warning", "#f1fa8c", false),
        ("Info", "#8be9fd", false),
        ("Success", "#50fa7b", false),
    ]
    .into_iter()
    .map(|(keyword, color, bold)| HighlightRule {
        keyword: keyword.to_string(),
        color: color.to_string(),
        bold,
        case_insensitive: true,
    })
    .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HighlightConfig {
    pub spec: Option<String>,
    pub ips: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub confirm_close_running: bool,
    pub default_profile: String,
    #[serde(default = "default_true")]
    pub restore_tabs: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            confirm_close_running: default_true(),
            default_profile: DEFAULT_PROFILE_ID.to_string(),
            restore_tabs: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_rule_spec_includes_flags() {
        let rule = HighlightRule {
            keyword: " Error ".to_string(),
            color: "#ff0000".to_string(),
            bold: true,
            case_insensitive: true,
        };
        assert_eq!(rule.spec(), "Error=#ff0000:bold:nocase");
        assert_eq!(rule.problem(), None);
    }

    #[test]
    fn highlight_rule_rejects_invalid_values() {
        let mut rule = HighlightRule::default();
        assert!(rule.problem().is_some());

        rule.keyword = "a=b".to_string();
        assert!(rule.problem().is_some());

        rule.keyword = "Error".to_string();
        rule.color = "red".to_string();
        assert!(rule.problem().is_some());

        rule.color = "#00ff00".to_string();
        assert_eq!(rule.problem(), None);
    }

    #[test]
    fn legacy_store_gets_highlight_defaults() {
        let json = r#"{"version":1,"sessions":[],"folders":[],"profiles":[],"settings":{"default_profile":"default","confirm_close_running":true}}"#;
        let store: Store = serde_json::from_str(json).expect("store antigo");
        assert!(!store.highlight_rules.is_empty());
        assert!(store.highlight_ips);
        assert_eq!(store.highlight_rules[0].keyword, "Error");
    }

    #[test]
    fn highlight_config_comes_from_enabled_rules() {
        let mut store = Store::default();
        let config = store.highlight_config();
        let spec = config.spec.expect("spec");
        assert!(spec.contains("Error=#ff5f5f:bold:nocase"));
        assert!(config.ips);

        store.highlight_rules.clear();
        store.highlight_ips = false;
        let config = store.highlight_config();
        assert_eq!(config.spec, None);
        assert!(!config.ips);
    }

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

    #[test]
    fn log_path_is_slugged_under_configured_directory() {
        let mut session = Session::new("Switch SFP 01", "10.0.0.1");
        session.log_dir = Some("/tmp/lt-logs".to_string());
        let path = session.log_path();
        assert_eq!(path.parent().unwrap(), std::path::Path::new("/tmp/lt-logs"));
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("switch-sfp-01-"), "{name}");
        assert!(name.ends_with(".log"), "{name}");
    }

    #[test]
    fn log_path_defaults_to_state_directory() {
        let session = Session::new("Teste", "10.0.0.1");
        let path = session.log_path();
        assert!(path.components().any(|part| part.as_os_str() == "logs"));
    }

    #[test]
    fn legacy_session_gets_log_defaults() {
        let json = r#"{"id":"11111111-1111-4111-8111-111111111111","name":"x","host":""}"#;
        let session: Session = serde_json::from_str(json).unwrap();
        assert!(!session.log_enabled);
        assert!(session.log_dir.is_none());
    }

    #[test]
    fn forward_rule_round_trips() {
        let rule = ForwardRule {
            kind: ForwardKind::Remote,
            bind_address: "0.0.0.0".to_string(),
            bind_port: 8080,
            target_host: "10.0.0.5".to_string(),
            target_port: 80,
        };
        let json = serde_json::to_string(&rule).unwrap();
        let decoded: ForwardRule = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, rule);
        assert!(json.contains("\"remote\""));
    }

    #[test]
    fn legacy_ssh_options_get_empty_forwards() {
        let json = r#"{"jump":null,"compression":false,"agent_forward":false,"x11":false,"keepalive":30,"extra_args":[]}"#;
        let options: SshOptions = serde_json::from_str(json).unwrap();
        assert!(options.forwards.is_empty());
    }

    #[test]
    fn forward_rule_problem_accepts_valid_rule() {
        let rule = ForwardRule {
            kind: ForwardKind::Local,
            bind_address: "127.0.0.1".to_string(),
            bind_port: 8080,
            target_host: "10.0.0.5".to_string(),
            target_port: 80,
        };
        assert_eq!(rule.problem(), None);

        let dynamic = ForwardRule {
            kind: ForwardKind::Dynamic,
            bind_address: "127.0.0.1".to_string(),
            bind_port: 1080,
            target_host: String::new(),
            target_port: 0,
        };
        assert_eq!(dynamic.problem(), None);
    }

    #[test]
    fn forward_rule_problem_rejects_port_zero_and_empty_host() {
        let mut rule = ForwardRule {
            kind: ForwardKind::Local,
            bind_address: "127.0.0.1".to_string(),
            bind_port: 0,
            target_host: "10.0.0.5".to_string(),
            target_port: 80,
        };
        assert_eq!(rule.problem(), Some("Informe a porta."));

        rule.bind_port = 8080;
        rule.target_host = "  ".to_string();
        assert_eq!(rule.problem(), Some("Informe o host de destino."));
    }

    #[test]
    fn forward_rule_summary_for_local_and_dynamic() {
        let local = ForwardRule {
            kind: ForwardKind::Local,
            bind_address: "127.0.0.1".to_string(),
            bind_port: 8080,
            target_host: "10.0.0.5".to_string(),
            target_port: 80,
        };
        assert_eq!(local.summary(), "127.0.0.1:8080 → 10.0.0.5:80");

        let dynamic = ForwardRule {
            kind: ForwardKind::Dynamic,
            bind_address: "127.0.0.1".to_string(),
            bind_port: 1080,
            target_host: String::new(),
            target_port: 0,
        };
        assert_eq!(dynamic.summary(), "127.0.0.1:1080 (SOCKS)");
    }

    #[test]
    fn serial_options_round_trip() {
        let serial = SerialOptions {
            port: "/dev/ttyUSB0".to_string(),
            baud: 115200,
            data_bits: 7,
            parity: SerialParity::Even,
            stop_bits: 2,
            flow: SerialFlow::Hardware,
        };
        let json = serde_json::to_string(&serial).unwrap();
        let parsed: SerialOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, serial);
        assert_eq!(parsed.summary(), "/dev/ttyUSB0 · 115200 7E2");
        assert_eq!(parsed.problem(), None);
    }

    #[test]
    fn serial_options_defaults_to_9600_8n1() {
        let serial = SerialOptions::default();
        assert_eq!(serial.baud, 9600);
        assert_eq!(serial.data_bits, 8);
        assert_eq!(serial.parity, SerialParity::None);
        assert_eq!(serial.stop_bits, 1);
        assert_eq!(serial.flow, SerialFlow::None);
        assert_eq!(serial.problem(), Some("Informe a porta serial."));

        let parsed: SerialOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, SerialOptions::default());
    }

    #[test]
    fn serial_options_validate_port_and_bits() {
        let mut serial = SerialOptions {
            port: "/dev/ttyUSB0".to_string(),
            ..SerialOptions::default()
        };
        assert_eq!(serial.problem(), None);

        serial.port = "ttyUSB0".to_string();
        assert!(serial.problem().is_some());

        serial.port = "/dev/tty USB".to_string();
        assert!(serial.problem().is_some());

        serial.port = "/dev/ttyUSB0".to_string();
        serial.baud = 12345;
        assert!(serial.problem().is_some());

        serial.baud = 9600;
        serial.data_bits = 9;
        assert!(serial.problem().is_some());

        serial.data_bits = 8;
        serial.stop_bits = 3;
        assert!(serial.problem().is_some());
    }

    #[test]
    fn serial_session_is_not_local_and_has_summary() {
        let mut session = Session::new("Console", "");
        assert!(session.is_local());
        assert!(!session.is_serial());

        session.serial = Some(SerialOptions {
            port: "/dev/ttyS0".to_string(),
            ..SerialOptions::default()
        });
        assert!(session.is_serial());
        assert!(!session.is_local());
        assert_eq!(session.subtitle(), "/dev/ttyS0 · 9600 8N1");
    }
}
