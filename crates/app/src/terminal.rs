use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;
use lt_core::{AuthMethod, HighlightConfig, Profile, Session};
use lt_ssh::{AskpassKind, AskpassSpec};
use vte4::prelude::*;

const PCRE2_MULTILINE: u32 = 0x0000_0400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    Connecting,
    Connected,
    Exited,
}

pub struct TerminalView {
    pub root: gtk::Box,
    pub id: uuid::Uuid,
    pub session: Session,
    pub terminal: vte4::Terminal,
    state: Rc<Cell<ConnState>>,
    search_tag: Cell<i32>,
    pid: Rc<Cell<Option<glib::Pid>>>,
    log_active: Cell<bool>,
}

impl TerminalView {
    pub fn new(
        session: &Session,
        profile: &Profile,
        highlight: &HighlightConfig,
        exe: PathBuf,
        on_state: Rc<dyn Fn(ConnState)>,
    ) -> Result<Rc<Self>, lt_ssh::BuildError> {
        let terminal = vte4::Terminal::new();
        terminal.set_hexpand(true);
        terminal.set_vexpand(true);
        apply_profile(&terminal, profile);

        let askpass = askpass_for(session, exe);
        let mut command = lt_ssh::build(session, askpass.as_ref())?;
        if session.is_local() && profile.syntax_highlight {
            if let Some(fish) = find_in_path("fish") {
                command.program = fish;
                command.args.clear();
            }
        }
        if let Some(relay) = relay_path() {
            let mut args = Vec::new();
            if profile.highlight_output {
                if let Some(spec) = &highlight.spec {
                    args.push("--rules".to_string());
                    args.push(spec.clone());
                }
                if highlight.ips {
                    args.push("--ips".to_string());
                }
            }
            args.push("--log".to_string());
            args.push(session.log_path().display().to_string());
            if !session.log_enabled {
                args.push("--log-paused".to_string());
            }
            args.push("--".to_string());
            args.push(command.program);
            args.extend(command.args);
            command.program = relay;
            command.args = args;
        }

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("terminal-frame");
        root.append(&terminal);

        let state = Rc::new(Cell::new(ConnState::Connecting));

        let view = Rc::new(Self {
            root,
            id: uuid::Uuid::new_v4(),
            session: session.clone(),
            terminal,
            state: state.clone(),
            search_tag: Cell::new(-1),
            pid: Rc::new(Cell::new(None)),
            log_active: Cell::new(session.log_enabled),
        });

        setup_links(&view.terminal);

        let state_signal = state.clone();
        let on_state_exit = on_state.clone();
        view.terminal.connect_child_exited(move |_, _status| {
            state_signal.set(ConnState::Exited);
            on_state_exit(ConnState::Exited);
        });

        view.spawn(&command, state, on_state)?;
        Ok(view)
    }

    fn spawn(
        &self,
        command: &lt_ssh::CommandSpec,
        state: Rc<Cell<ConnState>>,
        on_state: Rc<dyn Fn(ConnState)>,
    ) -> Result<(), lt_ssh::BuildError> {
        let argv_strings = command.argv();
        let argv: Vec<&str> = argv_strings.iter().map(String::as_str).collect();
        let env_strings = merged_env(&command.env);
        let envv: Vec<&str> = env_strings.iter().map(String::as_str).collect();
        let home = dirs::home_dir().map(|path| path.display().to_string());

        let pid_slot = self.pid.clone();
        let terminal = self.terminal.clone();
        let terminal_cb = terminal.clone();
        let on_state_ok = on_state.clone();
        let on_state_err = on_state;

        terminal.spawn_async(
            vte4::PtyFlags::DEFAULT,
            home.as_deref(),
            &argv,
            &envv,
            glib::SpawnFlags::DEFAULT,
            || {},
            -1,
            None::<&gio::Cancellable>,
            move |result| match result {
                Ok(pid) => {
                    pid_slot.set(Some(pid));
                    terminal_cb.watch_child(pid);
                    state.set(ConnState::Connected);
                    on_state_ok(ConnState::Connected);
                }
                Err(error) => {
                    let message = format!("\r\n\x1b[31mFalha ao iniciar o ssh: {error}\x1b[0m\r\n");
                    terminal_cb.feed(message.as_bytes());
                    state.set(ConnState::Exited);
                    on_state_err(ConnState::Exited);
                }
            },
        );

        Ok(())
    }

    pub fn state(&self) -> ConnState {
        self.state.get()
    }

    pub fn is_running(&self) -> bool {
        self.state.get() != ConnState::Exited
    }

    pub fn focus(&self) {
        self.terminal.grab_focus();
    }

    pub fn send(&self, text: &str) {
        self.terminal.feed_child(text.as_bytes());
    }

    pub fn send_line(&self, text: &str) {
        self.terminal.feed_child(text.as_bytes());
        self.terminal.feed_child(b"\r");
    }

    pub fn copy(&self) {
        self.terminal.copy_clipboard_format(vte4::Format::Text);
    }

    pub fn paste(&self) {
        self.terminal.paste_clipboard();
    }

    pub fn zoom(&self, delta: f64) {
        let scale = (self.terminal.font_scale() + delta).clamp(0.5, 4.0);
        self.terminal.set_font_scale(scale);
    }

    pub fn reset_zoom(&self) {
        self.terminal.set_font_scale(1.0);
    }

    pub fn search(&self, needle: &str) -> bool {
        self.clear_highlight();
        if needle.is_empty() {
            self.terminal.search_set_regex(None, 0);
            return false;
        }
        let escaped = glib::Regex::escape_string(needle);
        let Ok(regex) = vte4::Regex::for_search(&escaped, PCRE2_MULTILINE) else {
            return false;
        };
        self.terminal.search_set_regex(Some(&regex), 0);
        if let Ok(highlight) = vte4::Regex::for_match(&escaped, PCRE2_MULTILINE) {
            let tag = self.terminal.match_add_regex(&highlight, 0);
            self.search_tag.set(tag);
        }
        self.terminal.search_find_next()
    }

    pub fn logging(&self) -> bool {
        self.log_active.get()
    }

    pub fn log_directory(&self) -> PathBuf {
        self.session.log_directory()
    }

    pub fn toggle_log(&self) -> bool {
        if let Some(pid) = self.pid.get() {
            unsafe {
                libc::kill(pid.0, libc::SIGUSR1);
            }
            let next = !self.log_active.get();
            self.log_active.set(next);
        }
        self.log_active.get()
    }

    pub fn clear_highlight(&self) {
        let tag = self.search_tag.replace(-1);
        if tag >= 0 {
            self.terminal.match_remove(tag);
        }
    }

    pub fn find_next(&self) -> bool {
        self.terminal.search_find_next()
    }

    pub fn find_previous(&self) -> bool {
        self.terminal.search_find_previous()
    }
}

fn askpass_for(session: &Session, exe: PathBuf) -> Option<AskpassSpec> {
    if !session.auth.save_password {
        return None;
    }

    let kind = match session.auth.method {
        AuthMethod::PublicKey => AskpassKind::Passphrase,
        AuthMethod::Agent => return None,
        _ => AskpassKind::Password,
    };

    let account = match kind {
        AskpassKind::Passphrase => lt_core::account_passphrase(session.id),
        AskpassKind::Password => lt_core::account_password(session.id),
    };

    let store = lt_core::KeyringStore::new();
    let has_secret = lt_core::SecretStore::get(&store, &account)
        .ok()
        .flatten()
        .is_some();

    has_secret.then_some(AskpassSpec {
        program: exe,
        session_id: session.id,
        kind,
    })
}

fn merged_env(overrides: &[(String, String)]) -> Vec<String> {
    let mut values: Vec<(String, String)> = std::env::vars()
        .filter(|(key, _)| !key.starts_with("LT_ASKPASS_") && !key.starts_with("SSH_ASKPASS"))
        .collect();

    for (key, value) in overrides {
        values.retain(|(existing, _)| existing != key);
        values.push((key.clone(), value.clone()));
    }

    values
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect()
}

pub fn apply_profile(terminal: &vte4::Terminal, profile: &Profile) {
    let font = gtk::pango::FontDescription::from_string(&profile.font);
    terminal.set_font(Some(&font));
    terminal.set_scrollback_lines(i64::from(profile.scrollback));
    terminal.set_cursor_blink_mode(vte4::CursorBlinkMode::On);
    terminal.set_mouse_autohide(true);
    terminal.set_font_scale(1.0);

    if let Some(palette) = Palette::find(&profile.palette) {
        let foreground = gtk::gdk::RGBA::parse(palette.foreground).ok();
        let background = gtk::gdk::RGBA::parse(palette.background).ok();
        let parsed: Vec<gtk::gdk::RGBA> = palette
            .colors
            .iter()
            .filter_map(|color| gtk::gdk::RGBA::parse(*color).ok())
            .collect();
        let refs: Vec<&gtk::gdk::RGBA> = parsed.iter().collect();
        if refs.len() == palette.colors.len() {
            terminal.set_colors(foreground.as_ref(), background.as_ref(), &refs);
        }
        if let Some(accent) = palette
            .colors
            .get(4)
            .and_then(|c| gtk::gdk::RGBA::parse(*c).ok())
        {
            let mut selection = accent;
            selection.set_alpha(0.45);
            terminal.set_color_highlight(Some(&selection));
            terminal.set_color_highlight_foreground(foreground.as_ref());
        }
    }
}

pub const PALETTES: &[(&str, &str)] = &[
    ("system", "Sistema"),
    ("tango", "Tango"),
    ("solarized-dark", "Solarized Dark"),
    ("solarized-light", "Solarized Light"),
    ("dracula", "Dracula"),
    ("nord", "Nord"),
    ("gruvbox-dark", "Gruvbox Dark"),
    ("one-dark", "One Dark"),
    ("catppuccin-mocha", "Catppuccin Mocha"),
    ("tokyo-night", "Tokyo Night"),
];

struct Palette {
    foreground: &'static str,
    background: &'static str,
    colors: [&'static str; 16],
}

impl Palette {
    fn find(name: &str) -> Option<Self> {
        let palette = match name {
            "tango" => Self {
                foreground: "#eeeeec",
                background: "#2e3436",
                colors: [
                    "#2e3436", "#cc0000", "#4e9a06", "#c4a000", "#3465a4", "#75507b", "#06989a",
                    "#d3d7cf", "#555753", "#ef2929", "#8ae234", "#fce94f", "#729fcf", "#ad7fa8",
                    "#34e2e2", "#eeeeec",
                ],
            },
            "solarized-dark" => Self {
                foreground: "#839496",
                background: "#002b36",
                colors: [
                    "#073642", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#2aa198",
                    "#eee8d5", "#002b36", "#cb4b16", "#586e75", "#657b83", "#839496", "#6c71c4",
                    "#93a1a1", "#fdf6e3",
                ],
            },
            "dracula" => Self {
                foreground: "#f8f8f2",
                background: "#282a36",
                colors: [
                    "#21222c", "#ff5555", "#50fa7b", "#f1fa8c", "#bd93f9", "#ff79c6", "#8be9fd",
                    "#f8f8f2", "#6272a4", "#ff6e6e", "#69ff94", "#ffffa5", "#d6acff", "#ff92df",
                    "#a4ffff", "#ffffff",
                ],
            },
            "solarized-light" => Self {
                foreground: "#657b83",
                background: "#fdf6e3",
                colors: [
                    "#073642", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#2aa198",
                    "#eee8d5", "#002b36", "#cb4b16", "#586e75", "#657b83", "#839496", "#6c71c4",
                    "#93a1a1", "#fdf6e3",
                ],
            },
            "nord" => Self {
                foreground: "#d8dee9",
                background: "#2e3440",
                colors: [
                    "#3b4252", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#88c0d0",
                    "#e5e9f0", "#4c566a", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead",
                    "#8fbcbb", "#eceff4",
                ],
            },
            "gruvbox-dark" => Self {
                foreground: "#ebdbb2",
                background: "#282828",
                colors: [
                    "#282828", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a",
                    "#a89984", "#928374", "#fb4934", "#b8bb26", "#fabd2f", "#83a598", "#d3869b",
                    "#8ec07c", "#ebdbb2",
                ],
            },
            "one-dark" => Self {
                foreground: "#abb2bf",
                background: "#282c34",
                colors: [
                    "#282c34", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd", "#56b6c2",
                    "#dcdfe4", "#5c6370", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd",
                    "#56b6c2", "#ffffff",
                ],
            },
            "catppuccin-mocha" => Self {
                foreground: "#cdd6f4",
                background: "#1e1e2e",
                colors: [
                    "#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5",
                    "#bac2de", "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7",
                    "#94e2d5", "#a6adc8",
                ],
            },
            "tokyo-night" => Self {
                foreground: "#c0caf5",
                background: "#1a1b26",
                colors: [
                    "#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff",
                    "#a9b1d6", "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7",
                    "#7dcfff", "#c0caf5",
                ],
            },
            _ => return None,
        };
        Some(palette)
    }
}

fn relay_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let sibling = exe.parent()?.join("like-terminal-relay");
    if sibling.is_file() {
        return Some(sibling.display().to_string());
    }
    find_in_path("like-terminal-relay")
}

fn find_in_path(name: &str) -> Option<String> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
        .map(|path| path.display().to_string())
}

fn setup_links(terminal: &vte4::Terminal) {
    terminal.set_allow_hyperlink(true);
    let patterns = [
        r#"(?i)\b(?:https?|ftp)://[^\s<>"']+"#,
        r"[[:alnum:]._%+-]+@[[:alnum:]-]+\.[[:alnum:].-]+",
        r"(?:~|/)[[:alnum:]_.+/-]{2,}",
    ];
    for pattern in patterns {
        if let Ok(regex) = vte4::Regex::for_match(pattern, PCRE2_MULTILINE) {
            let tag = terminal.match_add_regex(&regex, 0);
            terminal.match_set_cursor_name(tag, "pointer");
        }
    }

    let gesture = gtk::GestureClick::new();
    gesture.set_button(gtk::gdk::BUTTON_PRIMARY);
    let term = terminal.clone();
    gesture.connect_released(move |gesture, _n_press, x, y| {
        if !gesture
            .current_event_state()
            .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return;
        }
        if let Some(uri) = link_at(&term, x, y) {
            let _ = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>);
        }
    });
    terminal.add_controller(gesture);
}

fn link_at(terminal: &vte4::Terminal, x: f64, y: f64) -> Option<String> {
    if let Some(uri) = terminal.check_hyperlink_at(x, y) {
        return Some(uri.to_string());
    }
    let (text, tag) = terminal.check_match_at(x, y);
    if tag < 0 {
        return None;
    }
    let text = text?.to_string();
    if text.contains("://") {
        return Some(text);
    }
    if text.contains('@') && !text.starts_with('/') && !text.starts_with('~') {
        return Some(format!("mailto:{text}"));
    }
    let path = match text.strip_prefix("~/") {
        Some(rest) => dirs::home_dir().map(|home| home.join(rest))?,
        None => PathBuf::from(text),
    };
    Some(format!("file://{}", path.display()))
}

pub fn expand_path(path: &std::path::Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix("~/") {
        Some(rest) => dirs::home_dir()
            .map(|home| home.join(rest))
            .unwrap_or_else(|| path.to_path_buf()),
        None => path.to_path_buf(),
    }
}
