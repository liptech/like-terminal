use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lt_core::{Folder, HighlightRule, Paths, Profile, Session, Store};
use uuid::Uuid;

use crate::app::{APP_ID, APP_NAME, VERSION};
use crate::panes::PaneView;
use crate::preferences::{self, PrefsHooks};
use crate::session_dialog;
use crate::sidebar::{Sidebar, SidebarCallbacks};
use crate::state::{self, UiState};

const CSD_SHADOW: i32 = 5;
const MIN_WINDOW_WIDTH: i32 = 480;
const MIN_WINDOW_HEIGHT: i32 = 320;

thread_local! {
    static WINDOW: RefCell<Option<Rc<Window>>> = const { RefCell::new(None) };
}

pub fn present(app: &adw::Application) {
    if let Some(existing) = WINDOW.with(|slot| slot.borrow().clone()) {
        existing.window.present();
        return;
    }

    match Window::new(app) {
        Ok(window) => {
            WINDOW.with(|slot| *slot.borrow_mut() = Some(window.clone()));
            window.window.present();
        }
        Err(error) => tracing::error!(%error, "falha ao abrir a janela"),
    }
}

pub struct Window {
    app: adw::Application,
    pub window: adw::ApplicationWindow,
    store: RefCell<Store>,
    paths: Paths,
    exe: PathBuf,
    tabs: adw::TabView,
    split: adw::OverlaySplitView,
    sidebar: Rc<Sidebar>,
    quick: gtk::Entry,
    send_entry: gtk::Entry,
    broadcast: gtk::CheckButton,
    status_label: gtk::Label,
    state_label: gtk::Label,
    status_dot: gtk::Box,
    toast: adw::ToastOverlay,
    panes: RefCell<Vec<(adw::TabPage, Rc<PaneView>)>>,
    me: RefCell<Weak<Window>>,
}

impl Window {
    fn new(app: &adw::Application) -> anyhow::Result<Rc<Self>> {
        let paths = Paths::discover()
            .unwrap_or_else(|_| Paths::with_base(&std::env::temp_dir().join("like-terminal")));
        let store = lt_core::load(&paths.config).unwrap_or_else(|error| {
            tracing::warn!(%error, "usando configuração padrão");
            Store::default()
        });
        let ui = state::load(&paths.state);
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("like-terminal"));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(APP_NAME)
            .default_width(ui.width.max(640))
            .default_height(ui.height.max(420))
            .build();

        let window_ref: Rc<RefCell<Weak<Window>>> = Rc::new(RefCell::new(Weak::new()));

        let sidebar = Sidebar::new(SidebarCallbacks {
            connect: {
                let window_ref = window_ref.clone();
                Rc::new(move |id| {
                    if let Some(window) = window_ref.borrow().upgrade() {
                        window.connect_session(id);
                    }
                })
            },
            edit: {
                let window_ref = window_ref.clone();
                Rc::new(move |id| {
                    if let Some(window) = window_ref.borrow().upgrade() {
                        window.edit_session(id);
                    }
                })
            },
            duplicate: {
                let window_ref = window_ref.clone();
                Rc::new(move |id| {
                    if let Some(window) = window_ref.borrow().upgrade() {
                        window.duplicate_session(id);
                    }
                })
            },
            delete_session: {
                let window_ref = window_ref.clone();
                Rc::new(move |id| {
                    if let Some(window) = window_ref.borrow().upgrade() {
                        window.delete_session(id);
                    }
                })
            },
            new_session: {
                let window_ref = window_ref.clone();
                Rc::new(move |folder| {
                    if let Some(window) = window_ref.borrow().upgrade() {
                        window.new_session(folder);
                    }
                })
            },
            new_folder: {
                let window_ref = window_ref.clone();
                Rc::new(move |parent| {
                    if let Some(window) = window_ref.borrow().upgrade() {
                        window.new_folder(parent);
                    }
                })
            },
            rename_folder: {
                let window_ref = window_ref.clone();
                Rc::new(move |id| {
                    if let Some(window) = window_ref.borrow().upgrade() {
                        window.rename_folder(id);
                    }
                })
            },
            delete_folder: {
                let window_ref = window_ref.clone();
                Rc::new(move |id| {
                    if let Some(window) = window_ref.borrow().upgrade() {
                        window.delete_folder(id);
                    }
                })
            },
        });

        let split = adw::OverlaySplitView::new();
        split.set_min_sidebar_width(220.0);
        split.set_max_sidebar_width(420.0);
        split.set_show_sidebar(ui.sidebar);
        split.set_sidebar(Some(&sidebar.root));

        let header = adw::HeaderBar::new();

        let sidebar_button = gtk::ToggleButton::new();
        sidebar_button.set_icon_name("sidebar-show-symbolic");
        sidebar_button.set_tooltip_text(Some("Mostrar/ocultar sessões (F9)"));
        split
            .bind_property("show-sidebar", &sidebar_button, "active")
            .sync_create()
            .bidirectional()
            .build();
        header.pack_start(&sidebar_button);

        let menu = gio::Menu::new();
        menu.append(Some("Nova sessão"), Some("app.new-session"));
        menu.append(Some("Nova pasta"), Some("app.new-folder"));
        let preferences_section = gio::Menu::new();
        preferences_section.append(Some("Preferências"), Some("app.preferences"));
        preferences_section.append(Some("Atalhos"), Some("app.shortcuts"));
        preferences_section.append(Some("Política de privacidade"), Some("app.privacy"));
        preferences_section.append(Some("Termos de serviço"), Some("app.terms"));
        preferences_section.append(Some("Sobre o Like Terminal"), Some("app.about"));
        menu.append_section(None, &preferences_section);
        let quit_section = gio::Menu::new();
        quit_section.append(Some("Sair"), Some("app.quit"));
        menu.append_section(None, &quit_section);

        let menu_button = gtk::MenuButton::new();
        menu_button.set_icon_name("open-menu-symbolic");
        menu_button.set_menu_model(Some(&menu));
        menu_button.set_tooltip_text(Some("Menu principal"));
        header.pack_end(&menu_button);

        let quick = gtk::Entry::new();
        quick.set_placeholder_text(Some("Conectar: usuario@host:porta ou nome da sessão"));
        quick.set_hexpand(true);
        quick.set_tooltip_text(Some("Ctrl+L"));
        let quick_button = gtk::Button::from_icon_name("go-next-symbolic");
        quick_button.set_tooltip_text(Some("Conectar"));
        let quick_bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        quick_bar.add_css_class("quick-bar");
        quick_bar.append(&quick);
        quick_bar.append(&quick_button);

        let tabs = adw::TabView::new();
        tabs.set_hexpand(true);
        tabs.set_vexpand(true);
        tabs.set_default_icon(&gio::ThemedIcon::new("utilities-terminal-symbolic"));

        let tab_bar = adw::TabBar::new();
        tab_bar.set_view(Some(&tabs));
        tab_bar.set_autohide(false);

        let send_entry = gtk::Entry::new();
        send_entry.set_placeholder_text(Some("Enviar texto para o terminal ativo"));
        send_entry.set_hexpand(true);

        let broadcast = gtk::CheckButton::with_label("Todas as abas");
        broadcast.set_tooltip_text(Some("Ctrl+Shift+B"));

        let send_button = gtk::Button::from_icon_name("document-send-symbolic");
        send_button.set_tooltip_text(Some("Enviar"));

        let bottom_bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        bottom_bar.add_css_class("bottom-bar");
        bottom_bar.append(&broadcast);
        bottom_bar.append(&send_entry);
        bottom_bar.append(&send_button);

        let status_dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        status_dot.add_css_class("status-dot");
        status_dot.set_valign(gtk::Align::Center);
        let status_label = gtk::Label::new(Some("Nenhuma sessão"));
        status_label.set_xalign(0.0);
        let state_label = gtk::Label::new(None);
        state_label.set_hexpand(true);
        state_label.set_xalign(1.0);

        let status_bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        status_bar.add_css_class("status-bar");
        status_bar.append(&status_dot);
        status_bar.append(&status_label);
        status_bar.append(&state_label);

        let empty = adw::StatusPage::new();
        empty.set_icon_name(Some("utilities-terminal-symbolic"));
        empty.set_title("Nenhuma sessão aberta");
        empty.set_description(Some(
            "Abra uma sessão pela barra lateral ou use a conexão rápida para começar.",
        ));
        let empty_button = gtk::Button::with_label("Nova sessão");
        empty_button.add_css_class("pill");
        empty_button.add_css_class("suggested-action");
        empty_button.set_action_name(Some("app.new-session"));
        empty_button.set_halign(gtk::Align::Center);
        empty_button.set_width_request(200);
        empty.set_child(Some(&empty_button));

        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.add_named(&tabs, Some("tabs"));
        stack.add_named(&empty, Some("empty"));

        let stack_cb = stack.clone();
        tabs.connect_n_pages_notify(move |tabs| {
            let name = if tabs.n_pages() == 0 { "empty" } else { "tabs" };
            stack_cb.set_visible_child_name(name);
        });
        stack.set_visible_child_name("tabs");

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_vexpand(true);
        content.append(&stack);
        content.append(&bottom_bar);
        content.append(&status_bar);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&quick_bar);
        toolbar.add_top_bar(&tab_bar);
        toolbar.set_content(Some(&content));

        let toast = adw::ToastOverlay::new();
        toast.set_child(Some(&toolbar));

        split.set_content(Some(&toast));
        window.set_content(Some(&split));

        if ui.maximized {
            window.maximize();
        }

        let window = Rc::new(Self {
            app: app.clone(),
            window,
            store: RefCell::new(store),
            paths,
            exe,
            tabs,
            split,
            sidebar,
            quick,
            send_entry,
            broadcast,
            status_label,
            state_label,
            status_dot,
            toast,
            panes: RefCell::new(Vec::new()),
            me: RefCell::new(Weak::new()),
        });

        *window_ref.borrow_mut() = Rc::downgrade(&window);
        *window.me.borrow_mut() = Rc::downgrade(&window);

        window.sidebar.update(&window.store.borrow());
        window.register_app_actions();
        window.wire_events(&quick_button, &send_button);
        window.restore_sessions(&ui.open_sessions);
        window.refresh_status();

        Ok(window)
    }

    fn wire_events(&self, quick_button: &gtk::Button, send_button: &gtk::Button) {
        let weak = Rc::downgrade(&self.as_rc());
        self.quick.connect_activate(move |_| {
            if let Some(window) = weak.upgrade() {
                window.quick_connect();
            }
        });

        let weak = Rc::downgrade(&self.as_rc());
        quick_button.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.quick_connect();
            }
        });

        let weak = Rc::downgrade(&self.as_rc());
        self.send_entry.connect_activate(move |_| {
            if let Some(window) = weak.upgrade() {
                window.send_text();
            }
        });

        let weak = Rc::downgrade(&self.as_rc());
        send_button.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.send_text();
            }
        });

        let weak = Rc::downgrade(&self.as_rc());
        self.tabs.connect_close_page(move |tabs, page| {
            if let Some(window) = weak.upgrade() {
                return window.request_close_page(tabs, page);
            }
            glib::Propagation::Proceed
        });

        let weak = Rc::downgrade(&self.as_rc());
        self.tabs.connect_selected_page_notify(move |_| {
            if let Some(window) = weak.upgrade() {
                window.refresh_status();
            }
        });

        let weak = Rc::downgrade(&self.as_rc());
        let state_path = self.paths.state.clone();
        self.window.connect_close_request(move |window| {
            if let Some(window_ref) = weak.upgrade() {
                window_ref.save_store();
                let width = window.width() + CSD_SHADOW * 2;
                let height = window.height() + CSD_SHADOW * 2;
                let ui = UiState {
                    width: if width >= MIN_WINDOW_WIDTH { width } else { 0 },
                    height: if height >= MIN_WINDOW_HEIGHT {
                        height
                    } else {
                        0
                    },
                    maximized: window.is_maximized(),
                    sidebar: window_ref.split.shows_sidebar(),
                    open_sessions: window_ref.open_session_ids(),
                };
                state::save(&state_path, &ui);
            }
            glib::Propagation::Proceed
        });

        let weak = Rc::downgrade(&self.as_rc());
        self.tabs.connect_n_pages_notify(move |_| {
            if let Some(window) = weak.upgrade() {
                window.save_ui_state();
            }
        });
    }

    fn save_ui_state(&self) {
        let width = self.window.width() + CSD_SHADOW * 2;
        let height = self.window.height() + CSD_SHADOW * 2;
        let ui = UiState {
            width: if width >= MIN_WINDOW_WIDTH { width } else { 0 },
            height: if height >= MIN_WINDOW_HEIGHT {
                height
            } else {
                0
            },
            maximized: self.window.is_maximized(),
            sidebar: self.split.shows_sidebar(),
            open_sessions: self.open_session_ids(),
        };
        state::save(&self.paths.state, &ui);
    }

    fn as_rc(&self) -> Rc<Self> {
        self.me.borrow().upgrade().expect("janela registrada")
    }

    fn register_app_actions(self: &Rc<Self>) {
        let app = &self.app;

        add_action(app, self, "new-session", |window| window.new_session(None));
        add_action(app, self, "new-folder", |window| window.new_folder(None));
        add_action(app, self, "quick-connect", |window| {
            window.quick.grab_focus();
        });
        add_action(app, self, "new-tab", |window| {
            window.duplicate_current_tab()
        });
        add_action(app, self, "close-tab", |window| window.close_current_tab());
        add_action(app, self, "split-right", |window| {
            if let Some(pane) = window.active_pane() {
                pane.split(gtk::Orientation::Horizontal);
            }
        });
        add_action(app, self, "split-down", |window| {
            if let Some(pane) = window.active_pane() {
                pane.split(gtk::Orientation::Vertical);
            }
        });
        add_action(app, self, "copy", |window| {
            if let Some(pane) = window.active_pane() {
                pane.copy();
            }
        });
        add_action(app, self, "paste", |window| {
            if let Some(pane) = window.active_pane() {
                pane.paste();
            }
        });
        add_action(app, self, "find", |window| {
            if let Some(pane) = window.active_pane() {
                pane.toggle_search();
            }
        });
        add_action(app, self, "zoom-in", |window| {
            if let Some(pane) = window.active_pane() {
                pane.zoom(0.1);
            }
        });
        add_action(app, self, "zoom-out", |window| {
            if let Some(pane) = window.active_pane() {
                pane.zoom(-0.1);
            }
        });
        add_action(app, self, "zoom-reset", |window| {
            if let Some(pane) = window.active_pane() {
                pane.reset_zoom();
            }
        });
        add_action(app, self, "toggle-sidebar", |window| {
            let show = !window.split.shows_sidebar();
            window.split.set_show_sidebar(show);
        });
        add_action(app, self, "broadcast", |window| {
            window.broadcast.set_active(!window.broadcast.is_active());
            window.send_entry.grab_focus();
        });
        add_action(app, self, "preferences", |window| window.open_preferences());
        add_action(app, self, "shortcuts", |window| window.show_shortcuts());
        add_action(app, self, "privacy", |window| {
            window.show_document("Política de privacidade", "PRIVACY.md")
        });
        add_action(app, self, "terms", |window| {
            window.show_document("Termos de serviço", "TERMS.md")
        });
        add_action(app, self, "about", |window| window.show_about());
        add_action(app, self, "quit", |window| window.app.quit());
    }

    fn connect_session(&self, id: Uuid) {
        let session = self.store.borrow().session(id).cloned();
        match session {
            Some(session) => self.open_session(session),
            None => self.toast("Sessão não encontrada."),
        }
    }

    fn quick_connect(&self) {
        let text = self.quick.text().trim().to_string();
        if text.is_empty() {
            return;
        }

        if let Some(session) = self
            .store
            .borrow()
            .sessions
            .iter()
            .find(|session| session.name.eq_ignore_ascii_case(&text))
            .cloned()
        {
            self.quick.set_text("");
            self.open_session(session);
            return;
        }

        match lt_ssh::parse_quick_connect(&text) {
            Some(parsed) => {
                self.quick.set_text("");
                self.open_session(parsed.to_session());
            }
            None => self.toast("Endereço inválido. Use usuario@host:porta."),
        }
    }

    fn open_session(&self, session: Session) {
        let profile = {
            let store = self.store.borrow();
            let profile_id = session.terminal.profile.clone();
            store.profile(&profile_id).clone()
        };

        let page_holder: Rc<RefCell<Option<adw::TabPage>>> = Rc::new(RefCell::new(None));
        let weak = Rc::downgrade(&self.as_rc());

        let on_close = {
            let weak = weak.clone();
            let page_holder = page_holder.clone();
            Rc::new(move || {
                if let Some(window) = weak.upgrade() {
                    if let Some(page) = page_holder.borrow().clone() {
                        window.tabs.close_page(&page);
                    }
                }
            })
        };
        let on_state = Rc::new(move || {
            if let Some(window) = weak.upgrade() {
                window.refresh_status();
            }
        });

        let highlight = self.store.borrow().highlight_config();
        let pane = match PaneView::new(
            session.clone(),
            profile,
            highlight,
            self.exe.clone(),
            on_close,
            on_state,
        ) {
            Ok(pane) => pane,
            Err(error) => {
                self.toast(&format!("Não foi possível iniciar: {error}"));
                return;
            }
        };

        let page = self.tabs.add_page(&pane.root, None);
        page.set_title(&session.name);
        page.set_tooltip(&session.subtitle());
        page.set_icon(Some(&gio::ThemedIcon::new("utilities-terminal-symbolic")));
        *page_holder.borrow_mut() = Some(page.clone());

        self.panes.borrow_mut().push((page.clone(), pane.clone()));
        self.tabs.set_selected_page(&page);
        pane.focus();
        self.refresh_status();
        self.save_store();
        self.save_ui_state();
    }

    fn pane_for(&self, page: &adw::TabPage) -> Option<Rc<PaneView>> {
        self.panes
            .borrow()
            .iter()
            .find(|(candidate, _)| candidate == page)
            .map(|(_, pane)| pane.clone())
    }

    fn active_pane(&self) -> Option<Rc<PaneView>> {
        let page = self.tabs.selected_page()?;
        self.pane_for(&page)
    }

    fn active_terminal(&self) -> Option<Rc<crate::terminal::TerminalView>> {
        self.active_pane().and_then(|pane| pane.active())
    }

    fn request_close_page(&self, _tabs: &adw::TabView, page: &adw::TabPage) -> glib::Propagation {
        let running = self
            .pane_for(page)
            .map(|pane| pane.has_running())
            .unwrap_or(false);
        let confirm = self.store.borrow().settings.confirm_close_running;

        if running && confirm {
            let weak = Rc::downgrade(&self.as_rc());
            let accept = {
                let page = page.clone();
                let weak = weak.clone();
                move || {
                    if let Some(window) = weak.upgrade() {
                        window.tabs.close_page_finish(&page, true);
                    }
                }
            };
            let cancel = {
                let page = page.clone();
                let weak = weak.clone();
                move || {
                    if let Some(window) = weak.upgrade() {
                        window.tabs.close_page_finish(&page, false);
                    }
                }
            };
            if let Some(window) = weak.upgrade() {
                window.confirm(
                    "Fechar sessão ativa?",
                    "Há comandos em execução neste terminal. Fechar mesmo assim?",
                    "Fechar",
                    accept,
                    cancel,
                );
            }
            return glib::Propagation::Stop;
        }

        self.detach_page(page);
        glib::Propagation::Proceed
    }

    fn detach_page(&self, page: &adw::TabPage) {
        self.panes
            .borrow_mut()
            .retain(|(candidate, _)| candidate != page);
        self.refresh_status();
        self.save_ui_state();
    }

    fn close_current_tab(&self) {
        if let Some(page) = self.tabs.selected_page() {
            self.tabs.close_page(&page);
        }
    }

    fn duplicate_current_tab(&self) {
        let session = self
            .active_terminal()
            .map(|terminal| terminal.session.clone())
            .or_else(|| {
                let store = self.store.borrow();
                store.sessions.first().cloned()
            });

        match session {
            Some(session) => self.open_session(session),
            None => {
                self.quick.grab_focus();
            }
        }
    }

    fn send_text(&self) {
        let text = self.send_entry.text().to_string();
        if text.is_empty() {
            return;
        }

        if self.broadcast.is_active() {
            for (_, pane) in self.panes.borrow().iter() {
                pane.broadcast(&text);
            }
        } else if let Some(pane) = self.active_pane() {
            if let Some(terminal) = pane.active() {
                terminal.send_line(&text);
            }
        }

        self.send_entry.set_text("");
    }

    fn refresh_status(&self) {
        self.status_dot.remove_css_class("exited");
        self.status_dot.remove_css_class("idle");

        let Some(pane) = self.active_pane() else {
            self.status_dot.add_css_class("idle");
            self.status_label.set_text("Nenhuma sessão");
            self.state_label.set_text("");
            return;
        };

        let Some(terminal) = pane.active() else {
            self.status_label.set_text(&pane.title());
            self.state_label.set_text("");
            return;
        };

        let logging = terminal.logging();
        let address = terminal.session.address();
        self.status_label.set_text(&if logging {
            format!("{address} · log")
        } else {
            address
        });
        let (text, exited) = match terminal.state() {
            crate::terminal::ConnState::Connecting => ("Conectando…", false),
            crate::terminal::ConnState::Connected => ("Conectado", false),
            crate::terminal::ConnState::Exited => ("Desconectado", true),
        };
        self.state_label.set_text(text);
        if exited {
            self.status_dot.add_css_class("exited");
        }
    }

    fn restore_sessions(&self, ids: &[Uuid]) {
        let sessions: Vec<Session> = {
            let store = self.store.borrow();
            ids.iter()
                .filter_map(|id| store.session(*id).cloned())
                .collect()
        };
        for session in sessions {
            self.open_session(session);
        }
    }

    fn open_session_ids(&self) -> Vec<Uuid> {
        self.panes
            .borrow()
            .iter()
            .map(|(_, pane)| pane.session().id)
            .collect()
    }

    fn new_session(&self, folder: Option<Uuid>) {
        let mut draft = Session::new("", "");
        draft.folder = folder;
        let store = self.store.borrow().clone();
        let weak = Rc::downgrade(&self.as_rc());
        session_dialog::show(&self.window, &store, draft, false, move |mut session| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            if session.name.is_empty() {
                session.name = session.host.clone();
            }
            window.store.borrow_mut().sessions.push(session.clone());
            window.save_store();
            window.sidebar.update(&window.store.borrow());
            window.open_session(session);
        });
    }

    fn edit_session(&self, id: Uuid) {
        let Some(existing) = self.store.borrow().session(id).cloned() else {
            return;
        };
        let store = self.store.borrow().clone();
        let weak = Rc::downgrade(&self.as_rc());
        session_dialog::show(&self.window, &store, existing, true, move |session| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            {
                let mut store = window.store.borrow_mut();
                if let Some(slot) = store.session_mut(id) {
                    *slot = session;
                }
            }
            window.save_store();
            window.sidebar.update(&window.store.borrow());
        });
    }

    fn duplicate_session(&self, id: Uuid) {
        let Some(mut copy) = self.store.borrow().session(id).cloned() else {
            return;
        };
        copy.id = Uuid::new_v4();
        copy.name = format!("{} (cópia)", copy.name);
        copy.auth.save_password = false;
        self.store.borrow_mut().sessions.push(copy);
        self.save_store();
        self.sidebar.update(&self.store.borrow());
    }

    fn delete_session(&self, id: Uuid) {
        let Some(session) = self.store.borrow().session(id).cloned() else {
            return;
        };
        let weak = Rc::downgrade(&self.as_rc());
        self.confirm(
            "Excluir sessão?",
            &format!("A sessão \"{}\" será removida.", session.name),
            "Excluir",
            move || {
                let Some(window) = weak.upgrade() else {
                    return;
                };
                window
                    .store
                    .borrow_mut()
                    .sessions
                    .retain(|candidate| candidate.id != id);
                let secrets = lt_core::KeyringStore::new();
                use lt_core::SecretStore;
                let _ = secrets.delete(&lt_core::account_password(id));
                let _ = secrets.delete(&lt_core::account_passphrase(id));
                window.save_store();
                window.sidebar.update(&window.store.borrow());
            },
            || {},
        );
    }

    fn new_folder(&self, parent: Option<Uuid>) {
        let weak = Rc::downgrade(&self.as_rc());
        session_dialog::ask_text(&self.window, "Nova pasta", "", move |name| {
            if let Some(window) = weak.upgrade() {
                window
                    .store
                    .borrow_mut()
                    .folders
                    .push(Folder::new(name, parent));
                window.save_store();
                window.sidebar.update(&window.store.borrow());
            }
        });
    }

    fn rename_folder(&self, id: Uuid) {
        let Some(folder) = self.store.borrow().folder(id).cloned() else {
            return;
        };
        let weak = Rc::downgrade(&self.as_rc());
        session_dialog::ask_text(&self.window, "Renomear pasta", &folder.name, move |name| {
            if let Some(window) = weak.upgrade() {
                if let Some(slot) = window
                    .store
                    .borrow_mut()
                    .folders
                    .iter_mut()
                    .find(|candidate| candidate.id == id)
                {
                    slot.name = name;
                }
                window.save_store();
                window.sidebar.update(&window.store.borrow());
            }
        });
    }

    fn delete_folder(&self, id: Uuid) {
        let Some(folder) = self.store.borrow().folder(id).cloned() else {
            return;
        };
        let weak = Rc::downgrade(&self.as_rc());
        self.confirm(
            "Excluir pasta?",
            &format!(
                "A pasta \"{}\" e as sessões dentro dela serão removidas.",
                folder.name
            ),
            "Excluir",
            move || {
                let Some(window) = weak.upgrade() else {
                    return;
                };
                window.store.borrow_mut().remove_folder_tree(id);
                window.save_store();
                window.sidebar.update(&window.store.borrow());
            },
            || {},
        );
    }

    fn open_preferences(&self) {
        let store = self.store.borrow().clone();
        let weak = Rc::downgrade(&self.as_rc());
        preferences::show(
            &self.window,
            &store,
            PrefsHooks {
                set_profile: Rc::new(move |profile: Profile| {
                    if let Some(window) = weak.upgrade() {
                        {
                            let mut store = window.store.borrow_mut();
                            if let Some(slot) = store
                                .profiles
                                .iter_mut()
                                .find(|candidate| candidate.id == profile.id)
                            {
                                *slot = profile.clone();
                            }
                        }
                        window.save_store();
                        for (_, pane) in window.panes.borrow().iter() {
                            pane.apply_profile(&profile);
                        }
                    }
                }),
                set_confirm_close: {
                    let weak = Rc::downgrade(&self.as_rc());
                    Rc::new(move |confirm: bool| {
                        if let Some(window) = weak.upgrade() {
                            window.store.borrow_mut().settings.confirm_close_running = confirm;
                            window.save_store();
                        }
                    })
                },
                set_highlight: {
                    let weak = Rc::downgrade(&self.as_rc());
                    Rc::new(move |rules: Vec<HighlightRule>, ips: bool| {
                        if let Some(window) = weak.upgrade() {
                            {
                                let mut store = window.store.borrow_mut();
                                store.highlight_rules = rules;
                                store.highlight_ips = ips;
                            }
                            window.save_store();
                        }
                    })
                },
            },
        );
    }

    fn show_shortcuts(&self) {
        let text = [
            "Ctrl+Shift+N — nova sessão",
            "Ctrl+L — conexão rápida",
            "Ctrl+Shift+T — nova aba",
            "Ctrl+Shift+W — fechar aba",
            "Ctrl+Shift+D — dividir à direita",
            "Ctrl+Shift+E — dividir abaixo",
            "Ctrl+Shift+C / V — copiar / colar",
            "Ctrl+Shift+F — buscar",
            "Ctrl+= / Ctrl+- / Ctrl+0 — zoom",
            "F9 — mostrar/ocultar sessões",
            "Ctrl+Shift+B — enviar para todas as abas",
            "Ctrl+, — preferências",
        ]
        .join("\n");

        let dialog = adw::MessageDialog::new(Some(&self.window), Some("Atalhos"), Some(&text));
        dialog.add_response("close", "Fechar");
        dialog.set_default_response(Some("close"));
        dialog.connect_response(None, |dialog: &adw::MessageDialog, _| dialog.close());
        dialog.present();
    }

    fn show_about(&self) {
        let about = adw::AboutWindow::builder()
            .application_name(APP_NAME)
            .application_icon(APP_ID)
            .version(VERSION)
            .developer_name("Felipe Mendes")
            .comments("Terminal com abas, painéis divididos e gerenciamento de sessões SSH.")
            .copyright("© 2026 Felipe Mendes")
            .license_type(gtk::License::Gpl30)
            .modal(true)
            .build();
        about.set_transient_for(Some(&self.window));
        about.present();
    }

    fn show_document(&self, title: &str, file: &str) {
        match doc_path(file) {
            Some(path) if crate::document::show(&self.window, title, &path) => {}
            Some(_) => self.toast("Não foi possível ler o documento."),
            None => self.toast("Documento não encontrado."),
        }
    }

    fn confirm<F, G>(&self, heading: &str, body: &str, accept: &str, on_accept: F, on_cancel: G)
    where
        F: Fn() + 'static,
        G: Fn() + 'static,
    {
        let dialog = adw::MessageDialog::new(Some(&self.window), Some(heading), Some(body));
        dialog.add_response("cancel", "Cancelar");
        dialog.add_response("accept", accept);
        dialog.set_response_appearance("accept", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let on_accept = Rc::new(on_accept);
        let on_cancel = Rc::new(on_cancel);
        dialog.connect_response(None, move |dialog: &adw::MessageDialog, response| {
            if response == "accept" {
                on_accept();
            } else {
                on_cancel();
            }
            dialog.close();
        });
        dialog.present();
    }

    fn save_store(&self) {
        let store = self.store.borrow();
        if let Err(error) = lt_core::save(&self.paths.config, &store) {
            tracing::warn!(%error, "falha ao salvar as sessões");
        }
    }

    fn toast(&self, message: &str) {
        self.toast.add_toast(adw::Toast::new(message));
    }
}

fn add_action<F>(app: &adw::Application, window: &Rc<Window>, name: &str, handler: F)
where
    F: Fn(&Window) + 'static,
{
    let action = gio::SimpleAction::new(name, None);
    let weak = Rc::downgrade(window);
    action.connect_activate(move |_, _| {
        if let Some(window) = weak.upgrade() {
            handler(&window);
        }
    });
    app.add_action(&action);
}

fn doc_path(name: &str) -> Option<std::path::PathBuf> {
    let installed = std::path::Path::new("/app/share/doc/like-terminal").join(name);
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(name);
    [installed, source].into_iter().find(|path| path.exists())
}
