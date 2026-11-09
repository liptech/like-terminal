use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lt_core::{AuthMethod, Session};
use lt_ssh::config::ConfigEntry;

pub fn sessions_from(entries: &[ConfigEntry]) -> Vec<Session> {
    entries
        .iter()
        .map(|entry| {
            let host = entry
                .host_name
                .clone()
                .unwrap_or_else(|| entry.alias.clone());
            let mut session = Session::new(entry.alias.clone(), host);
            session.user = entry.user.clone().unwrap_or_default();
            session.port = entry.port.unwrap_or(22);
            if let Some(key) = entry.identity_file.clone() {
                session.auth.method = AuthMethod::PublicKey;
                session.auth.key_path = Some(PathBuf::from(key));
            }
            session
        })
        .collect()
}

pub fn show(
    parent: &adw::ApplicationWindow,
    entries: Vec<ConfigEntry>,
    on_import: Rc<dyn Fn(Vec<Session>)>,
) {
    let window = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Importar de ~/.ssh/config")
        .default_width(520)
        .default_height(560)
        .build();

    let header = adw::HeaderBar::new();
    let cancel = gtk::Button::with_label("Cancelar");
    let import = gtk::Button::with_label("Importar");
    import.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&import);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.set_margin_top(12);
    list.set_margin_bottom(12);
    list.set_margin_start(12);
    list.set_margin_end(12);

    let checks: Rc<Vec<(ConfigEntry, gtk::CheckButton)>> = Rc::new(
        entries
            .into_iter()
            .map(|entry| {
                let row = adw::ActionRow::new();
                row.set_title(&entry.alias);
                let mut subtitle = entry
                    .host_name
                    .clone()
                    .unwrap_or_else(|| entry.alias.clone());
                if let Some(user) = &entry.user {
                    subtitle = format!("{user}@{subtitle}");
                }
                if let Some(port) = entry.port {
                    subtitle = format!("{subtitle}:{port}");
                }
                row.set_subtitle(&subtitle);
                let check = gtk::CheckButton::new();
                check.set_active(true);
                check.set_valign(gtk::Align::Center);
                row.add_suffix(&check);
                row.set_activatable_widget(Some(&check));
                list.append(&row);
                (entry, check)
            })
            .collect(),
    );

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_child(Some(&list));
    scroller.set_vexpand(true);
    content.append(&scroller);
    toolbar.set_content(Some(&content));
    window.set_content(Some(&toolbar));

    {
        let window = window.clone();
        cancel.connect_clicked(move |_| window.close());
    }

    {
        let window = window.clone();
        let checks = checks.clone();
        import.connect_clicked(move |_| {
            let selected: Vec<ConfigEntry> = checks
                .iter()
                .filter(|(_, check)| check.is_active())
                .map(|(entry, _)| entry.clone())
                .collect();
            if selected.is_empty() {
                return;
            }
            on_import(sessions_from(&selected));
            window.close();
        });
    }

    window.present();
}
