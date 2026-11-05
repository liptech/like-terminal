use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use lt_core::{Folder, Session, Store};
use uuid::Uuid;

#[derive(Clone)]
pub struct SidebarCallbacks {
    pub connect: Rc<dyn Fn(Uuid)>,
    pub edit: Rc<dyn Fn(Uuid)>,
    pub duplicate: Rc<dyn Fn(Uuid)>,
    pub delete_session: Rc<dyn Fn(Uuid)>,
    pub new_session: Rc<dyn Fn(Option<Uuid>)>,
    pub new_folder: Rc<dyn Fn(Option<Uuid>)>,
    pub rename_folder: Rc<dyn Fn(Uuid)>,
    pub delete_folder: Rc<dyn Fn(Uuid)>,
}

pub struct Sidebar {
    pub root: gtk::Box,
    list: gtk::ListBox,
    search: gtk::SearchEntry,
    callbacks: SidebarCallbacks,
    store: RefCell<Store>,
    me: RefCell<Weak<Sidebar>>,
}

impl Sidebar {
    pub fn new(callbacks: SidebarCallbacks) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_width_request(240);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header.add_css_class("sidebar-header");
        let title = gtk::Label::new(Some("Sessões"));
        title.set_hexpand(true);
        title.set_xalign(0.0);
        title.add_css_class("heading");
        header.append(&title);

        let new_folder = gtk::Button::from_icon_name("folder-new-symbolic");
        new_folder.add_css_class("flat");
        new_folder.set_tooltip_text(Some("Nova pasta"));
        header.append(&new_folder);

        let new_session = gtk::Button::from_icon_name("list-add-symbolic");
        new_session.add_css_class("flat");
        new_session.set_tooltip_text(Some("Nova sessão"));
        header.append(&new_session);

        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Filtrar sessões"));
        search.set_margin_start(8);
        search.set_margin_end(8);
        search.set_margin_bottom(6);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_activate_on_single_click(false);
        list.add_css_class("navigation-sidebar");

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&list)
            .build();

        root.append(&header);
        root.append(&search);
        root.append(&scrolled);

        let sidebar = Rc::new(Self {
            root,
            list,
            search,
            callbacks,
            store: RefCell::new(Store::default()),
            me: RefCell::new(Weak::new()),
        });
        *sidebar.me.borrow_mut() = Rc::downgrade(&sidebar);

        let weak = Rc::downgrade(&sidebar);
        new_session.connect_clicked(move |_| {
            if let Some(sidebar) = weak.upgrade() {
                (sidebar.callbacks.new_session)(None);
            }
        });

        let weak = Rc::downgrade(&sidebar);
        new_folder.connect_clicked(move |_| {
            if let Some(sidebar) = weak.upgrade() {
                (sidebar.callbacks.new_folder)(None);
            }
        });

        let weak = Rc::downgrade(&sidebar);
        sidebar.search.connect_search_changed(move |_| {
            if let Some(sidebar) = weak.upgrade() {
                sidebar.rebuild();
            }
        });

        sidebar
    }

    pub fn update(&self, store: &Store) {
        *self.store.borrow_mut() = store.clone();
        self.rebuild();
    }

    fn rebuild(&self) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }

        let store = self.store.borrow();
        let query = self.search.text();
        let query = query.trim();

        if !query.is_empty() {
            let matches = store.matching_sessions(query);
            if matches.is_empty() {
                self.list.append(&empty_row("Nenhuma sessão encontrada"));
            }
            for session in matches {
                let row = self.session_row(session, 0);
                self.list.append(&row);
            }
            return;
        }

        if store.sessions.is_empty() && store.folders.is_empty() {
            self.list
                .append(&empty_row("Nenhuma sessão ainda. Use + para criar."));
            return;
        }

        self.fill(&self.list, &store, None, 0);
    }

    fn fill(&self, list: &gtk::ListBox, store: &Store, parent: Option<Uuid>, depth: i32) {
        for folder in store.child_folders(parent) {
            list.append(&self.folder_row(store, folder, depth));
        }
        for session in store.child_sessions(parent) {
            list.append(&self.session_row(session, depth));
        }
    }

    fn folder_row(&self, store: &Store, folder: &Folder, depth: i32) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);

        let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
        outer.set_margin_start(depth * 12);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let icon = gtk::Image::from_icon_name("folder-symbolic");
        let label = gtk::Label::new(Some(&folder.name));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.add_css_class("heading");

        let menu_button = gtk::MenuButton::new();
        menu_button.set_icon_name("view-more-symbolic");
        menu_button.add_css_class("flat");
        menu_button.set_has_frame(false);

        let group = gio::SimpleActionGroup::new();
        let folder_id = folder.id;
        let callbacks = self.callbacks.clone();
        let action = gio::SimpleAction::new("new-session", None);
        action.connect_activate(move |_, _| (callbacks.new_session)(Some(folder_id)));
        group.add_action(&action);

        let callbacks = self.callbacks.clone();
        let action = gio::SimpleAction::new("new-folder", None);
        action.connect_activate(move |_, _| (callbacks.new_folder)(Some(folder_id)));
        group.add_action(&action);

        let callbacks = self.callbacks.clone();
        let action = gio::SimpleAction::new("rename", None);
        action.connect_activate(move |_, _| (callbacks.rename_folder)(folder_id));
        group.add_action(&action);

        let callbacks = self.callbacks.clone();
        let action = gio::SimpleAction::new("delete", None);
        action.connect_activate(move |_, _| (callbacks.delete_folder)(folder_id));
        group.add_action(&action);

        row.insert_action_group("folder", Some(&group));

        let menu = gio::Menu::new();
        menu.append(Some("Nova sessão aqui"), Some("folder.new-session"));
        menu.append(Some("Nova subpasta"), Some("folder.new-folder"));
        menu.append(Some("Renomear"), Some("folder.rename"));
        menu.append(Some("Excluir pasta"), Some("folder.delete"));
        menu_button.set_popover(Some(&gtk::PopoverMenu::from_model(Some(&menu))));

        header.append(&icon);
        header.append(&label);
        header.append(&menu_button);

        let inner = gtk::ListBox::new();
        inner.set_selection_mode(gtk::SelectionMode::None);
        inner.set_activate_on_single_click(false);
        self.fill(&inner, store, Some(folder.id), depth + 1);

        let expander = gtk::Expander::new(None);
        expander.set_label_widget(Some(&header));
        expander.set_expanded(true);
        expander.set_child(Some(&inner));

        outer.append(&expander);
        row.set_child(Some(&outer));
        row
    }

    fn session_row(&self, session: &Session, depth: i32) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(true);

        let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        content.add_css_class("session-row");
        content.set_margin_start(depth * 12);

        let icon = gtk::Image::from_icon_name("utilities-terminal-symbolic");

        let texts = gtk::Box::new(gtk::Orientation::Vertical, 0);
        texts.set_hexpand(true);
        let name = gtk::Label::new(Some(&session.name));
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let address = gtk::Label::new(Some(&session.subtitle()));
        address.set_xalign(0.0);
        address.set_ellipsize(gtk::pango::EllipsizeMode::End);
        address.add_css_class("address");
        texts.append(&name);
        texts.append(&address);

        let menu_button = gtk::MenuButton::new();
        menu_button.set_icon_name("view-more-symbolic");
        menu_button.add_css_class("flat");
        menu_button.set_has_frame(false);

        let id = session.id;
        let group = gio::SimpleActionGroup::new();

        let callbacks = self.callbacks.clone();
        let action = gio::SimpleAction::new("connect", None);
        action.connect_activate(move |_, _| (callbacks.connect)(id));
        group.add_action(&action);

        let callbacks = self.callbacks.clone();
        let action = gio::SimpleAction::new("edit", None);
        action.connect_activate(move |_, _| (callbacks.edit)(id));
        group.add_action(&action);

        let callbacks = self.callbacks.clone();
        let action = gio::SimpleAction::new("duplicate", None);
        action.connect_activate(move |_, _| (callbacks.duplicate)(id));
        group.add_action(&action);

        let callbacks = self.callbacks.clone();
        let action = gio::SimpleAction::new("delete", None);
        action.connect_activate(move |_, _| (callbacks.delete_session)(id));
        group.add_action(&action);

        row.insert_action_group("row", Some(&group));

        let menu = gio::Menu::new();
        menu.append(Some("Conectar"), Some("row.connect"));
        menu.append(Some("Editar"), Some("row.edit"));
        menu.append(Some("Duplicar"), Some("row.duplicate"));
        let delete_section = gio::Menu::new();
        delete_section.append(Some("Excluir"), Some("row.delete"));
        menu.append_section(None, &delete_section);
        menu_button.set_popover(Some(&gtk::PopoverMenu::from_model(Some(&menu))));

        content.append(&icon);
        content.append(&texts);
        content.append(&menu_button);
        row.set_child(Some(&content));

        let callbacks = self.callbacks.clone();
        row.connect_activate(move |_| (callbacks.connect)(id));

        let callbacks = self.callbacks.clone();
        let double_click = gtk::GestureClick::new();
        double_click.set_button(1);
        double_click.connect_pressed(move |gesture, n_press, _, _| {
            if n_press == 2 {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                (callbacks.connect)(id);
            }
        });
        row.add_controller(double_click);

        row
    }
}

fn empty_row(text: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label.set_margin_top(24);
    label.set_margin_bottom(24);
    label.set_margin_start(12);
    label.set_margin_end(12);
    label.add_css_class("dim-label");
    row.set_child(Some(&label));
    row
}
