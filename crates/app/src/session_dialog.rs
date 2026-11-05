use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lt_core::{validate_session, AuthMethod, Session, Store};

pub fn show<F>(
    parent: &adw::ApplicationWindow,
    store: &Store,
    initial: Session,
    editing: bool,
    on_save: F,
) where
    F: Fn(Session) + 'static,
{
    let existing = Some(initial);
    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(if editing {
            "Editar sessão"
        } else {
            "Nova sessão"
        })
        .default_width(560)
        .default_height(720)
        .build();

    let header = adw::HeaderBar::new();
    let cancel = gtk::Button::with_label("Cancelar");
    let save = gtk::Button::with_label("Salvar");
    save.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&save);

    let grid = gtk::Grid::builder()
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .row_spacing(10)
        .column_spacing(12)
        .build();

    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_wrap(true);
    error.set_xalign(0.0);
    error.set_visible(false);
    grid.attach(&error, 0, 0, 2, 1);

    let mut row = 1;

    let name = gtk::Entry::new();
    name.set_hexpand(true);
    attach(&grid, &mut row, "Nome", &name);

    let folder_ids: Vec<Option<uuid::Uuid>> = std::iter::once(None)
        .chain(store.folders.iter().map(|f| Some(f.id)))
        .collect();
    let folder_names: Vec<&str> = std::iter::once("Sem pasta")
        .chain(store.folders.iter().map(|f| f.name.as_str()))
        .collect();
    let folder = gtk::DropDown::from_strings(&folder_names);
    attach(&grid, &mut row, "Pasta", &folder);

    let host = gtk::Entry::new();
    host.set_hexpand(true);
    host.set_placeholder_text(Some("ex.: 10.0.0.21 — vazio abre um shell local"));
    attach(&grid, &mut row, "Host", &host);

    let port = gtk::SpinButton::with_range(1.0, 65535.0, 1.0);
    port.set_value(22.0);
    attach(&grid, &mut row, "Porta", &port);

    let user = gtk::Entry::new();
    user.set_hexpand(true);
    attach(&grid, &mut row, "Usuário", &user);

    let methods = AuthMethod::all();
    let method_labels: Vec<&str> = methods.iter().map(|method| method.label()).collect();
    let method = gtk::DropDown::from_strings(&method_labels);
    attach(&grid, &mut row, "Autenticação", &method);

    let key_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let key_entry = gtk::Entry::new();
    key_entry.set_editable(false);
    key_entry.set_hexpand(true);
    let key_button = gtk::Button::with_label("Escolher…");
    key_row.append(&key_entry);
    key_row.append(&key_button);
    attach(&grid, &mut row, "Chave privada", &key_row);

    let save_password = gtk::Switch::new();
    save_password.set_halign(gtk::Align::Start);
    attach(&grid, &mut row, "Salvar senha", &save_password);

    let jump = gtk::Entry::new();
    jump.set_hexpand(true);
    jump.set_placeholder_text(Some("ex.: usuario@bastion:22"));
    attach(&grid, &mut row, "Jump host", &jump);

    let keepalive = gtk::SpinButton::with_range(0.0, 3600.0, 5.0);
    keepalive.set_value(30.0);
    attach(&grid, &mut row, "Keepalive (s)", &keepalive);

    let compression = gtk::Switch::new();
    compression.set_halign(gtk::Align::Start);
    attach(&grid, &mut row, "Compressão", &compression);

    let agent_forward = gtk::Switch::new();
    agent_forward.set_halign(gtk::Align::Start);
    attach(&grid, &mut row, "Encaminhar agente", &agent_forward);

    let x11 = gtk::Switch::new();
    x11.set_halign(gtk::Align::Start);
    attach(&grid, &mut row, "X11 forwarding", &x11);

    let extra = gtk::Entry::new();
    extra.set_hexpand(true);
    extra.set_placeholder_text(Some("separados por espaço"));
    attach(&grid, &mut row, "Argumentos extras", &extra);

    let key_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

    if let Some(session) = existing.as_ref() {
        name.set_text(&session.name);
        host.set_text(&session.host);
        port.set_value(f64::from(session.port));
        user.set_text(&session.user);
        save_password.set_active(session.auth.save_password);
        jump.set_text(session.ssh.jump.as_deref().unwrap_or(""));
        keepalive.set_value(f64::from(session.ssh.keepalive));
        compression.set_active(session.ssh.compression);
        agent_forward.set_active(session.ssh.agent_forward);
        x11.set_active(session.ssh.x11);
        extra.set_text(&session.ssh.extra_args.join(" "));

        if let Some(index) = methods.iter().position(|m| *m == session.auth.method) {
            method.set_selected(index as u32);
        }
        if let Some(index) = folder_ids.iter().position(|id| *id == session.folder) {
            folder.set_selected(index as u32);
        }
        if let Some(path) = session.auth.key_path.as_ref() {
            key_entry.set_text(&path.display().to_string());
            *key_path.borrow_mut() = Some(path.clone());
        }
    }

    {
        let dialog = dialog.clone();
        let key_path = key_path.clone();
        let key_entry = key_entry.clone();
        key_button.connect_clicked(move |_| {
            let key_entry = key_entry.clone();
            let key_path = key_path.clone();
            let file_dialog = gtk::FileDialog::new();
            file_dialog.set_title("Selecionar chave privada");
            file_dialog.open(Some(&dialog), None::<&gio::Cancellable>, move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        key_entry.set_text(&path.display().to_string());
                        *key_path.borrow_mut() = Some(path);
                    }
                }
            });
        });
    }

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&grid)
        .build();

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scrolled));
    dialog.set_content(Some(&toolbar));

    {
        let dialog = dialog.clone();
        cancel.connect_clicked(move |_| dialog.close());
    }

    let on_save = Rc::new(on_save);
    let save_dialog = dialog.clone();
    save.connect_clicked(move |_| {
        let mut session = existing.clone().unwrap_or_else(|| Session::new("", ""));
        session.name = name.text().trim().to_string();
        session.host = host.text().trim().to_string();
        session.user = user.text().trim().to_string();
        session.port = port.value().clamp(1.0, 65535.0) as u16;
        session.folder = folder_ids
            .get(folder.selected() as usize)
            .copied()
            .flatten();
        session.auth.method = methods
            .get(method.selected() as usize)
            .cloned()
            .unwrap_or_default();
        session.auth.key_path = key_path.borrow().clone();
        session.auth.save_password = save_password.is_active();
        session.ssh.jump = non_empty(jump.text().as_str());
        session.ssh.keepalive = keepalive.value().max(0.0) as u32;
        session.ssh.compression = compression.is_active();
        session.ssh.agent_forward = agent_forward.is_active();
        session.ssh.x11 = x11.is_active();
        session.ssh.extra_args = extra
            .text()
            .split_whitespace()
            .map(str::to_string)
            .collect();

        if session.name.is_empty() {
            session.name = if session.host.is_empty() {
                "Nova sessão".to_string()
            } else {
                session.host.clone()
            };
        }

        match validate_session(&session).first() {
            Some(issue) => {
                error.set_text(&issue.message);
                error.set_visible(true);
            }
            None => {
                on_save(session);
                save_dialog.close();
            }
        }
    });

    dialog.present();
}

fn attach(grid: &gtk::Grid, row: &mut i32, label: &str, widget: &impl IsA<gtk::Widget>) {
    let text = gtk::Label::new(Some(label));
    text.set_xalign(1.0);
    text.add_css_class("dim-label");
    grid.attach(&text, 0, *row, 1, 1);
    grid.attach(widget, 1, *row, 1, 1);
    *row += 1;
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub fn ask_text<F>(parent: &impl IsA<gtk::Window>, title: &str, initial: &str, on_ok: F)
where
    F: Fn(String) + 'static,
{
    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(title)
        .default_width(380)
        .build();

    let header = adw::HeaderBar::new();
    let cancel = gtk::Button::with_label("Cancelar");
    let ok = gtk::Button::with_label("OK");
    ok.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&ok);

    let entry = gtk::Entry::new();
    entry.set_text(initial);
    entry.set_margin_top(18);
    entry.set_margin_bottom(18);
    entry.set_margin_start(18);
    entry.set_margin_end(18);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&entry));
    dialog.set_content(Some(&toolbar));

    {
        let dialog = dialog.clone();
        cancel.connect_clicked(move |_| dialog.close());
    }

    let on_ok = Rc::new(on_ok);
    {
        let dialog = dialog.clone();
        let entry = entry.clone();
        let on_ok = on_ok.clone();
        ok.connect_clicked(move |_| {
            let value = entry.text().trim().to_string();
            if value.is_empty() {
                return;
            }
            on_ok(value);
            dialog.close();
        });
    }
    {
        let dialog = dialog.clone();
        let entry_cb = entry.clone();
        let on_ok = on_ok.clone();
        entry.connect_activate(move |_| {
            let value = entry_cb.text().trim().to_string();
            if value.is_empty() {
                return;
            }
            on_ok(value);
            dialog.close();
        });
    }

    dialog.present();
    entry.grab_focus();
}
