use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use lt_core::{AuthMethod, Profile, Session};
use lt_ssh::{AskpassKind, AskpassSpec};
use vte4::prelude::*;

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
}

impl TerminalView {
    pub fn new(
        session: &Session,
        profile: &Profile,
        exe: PathBuf,
        on_state: Rc<dyn Fn(ConnState)>,
    ) -> Result<Rc<Self>, lt_ssh::BuildError> {
        let terminal = vte4::Terminal::new();
        terminal.set_hexpand(true);
        terminal.set_vexpand(true);
        apply_profile(&terminal, profile);

        let askpass = askpass_for(session, exe);
        let command = lt_ssh::build(session, askpass.as_ref())?;

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
        });

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
        if needle.is_empty() {
            self.terminal.search_set_regex(None, 0);
            return false;
        }
        let escaped = glib::Regex::escape_string(needle);
        match vte4::Regex::for_search(&escaped, 0) {
            Ok(regex) => {
                self.terminal.search_set_regex(Some(&regex), 0);
                self.terminal.search_find_next()
            }
            Err(_) => false,
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
    }
}

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
            _ => return None,
        };
        Some(palette)
    }
}
