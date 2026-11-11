use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lt_core::{
    validate_session, AuthMethod, ForwardRule, SerialFlow, SerialOptions, SerialParity, Session,
    Store, SERIAL_BAUDS,
};

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
    let mut ssh_rows: Vec<gtk::Widget> = Vec::new();
    let mut serial_rows: Vec<gtk::Widget> = Vec::new();

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

    let kind = gtk::DropDown::from_strings(&["SSH / Terminal local", "Serial"]);
    attach(&grid, &mut row, "Tipo de conexão", &kind);

    let host = gtk::Entry::new();
    host.set_hexpand(true);
    host.set_placeholder_text(Some("ex.: 10.0.0.21 — vazio abre um shell local"));
    attach_group(&grid, &mut row, "Host", &host, &mut ssh_rows);

    let port = gtk::SpinButton::with_range(1.0, 65535.0, 1.0);
    port.set_value(22.0);
    attach_group(&grid, &mut row, "Porta", &port, &mut ssh_rows);

    let user = gtk::Entry::new();
    user.set_hexpand(true);
    attach_group(&grid, &mut row, "Usuário", &user, &mut ssh_rows);

    let methods = AuthMethod::all();
    let method_labels: Vec<&str> = methods.iter().map(|method| method.label()).collect();
    let method = gtk::DropDown::from_strings(&method_labels);
    attach_group(&grid, &mut row, "Autenticação", &method, &mut ssh_rows);

    let key_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let key_entry = gtk::Entry::new();
    key_entry.set_editable(false);
    key_entry.set_hexpand(true);
    let key_button = gtk::Button::with_label("Escolher…");
    key_row.append(&key_entry);
    key_row.append(&key_button);
    attach_group(&grid, &mut row, "Chave privada", &key_row, &mut ssh_rows);

    let save_password = gtk::Switch::new();
    save_password.set_halign(gtk::Align::Start);
    attach_group(
        &grid,
        &mut row,
        "Salvar senha",
        &save_password,
        &mut ssh_rows,
    );

    let jump = gtk::Entry::new();
    jump.set_hexpand(true);
    jump.set_placeholder_text(Some("ex.: usuario@bastion:22"));
    attach_group(&grid, &mut row, "Jump host", &jump, &mut ssh_rows);

    let keepalive = gtk::SpinButton::with_range(0.0, 3600.0, 5.0);
    keepalive.set_value(30.0);
    attach_group(&grid, &mut row, "Keepalive (s)", &keepalive, &mut ssh_rows);

    let compression = gtk::Switch::new();
    compression.set_halign(gtk::Align::Start);
    attach_group(&grid, &mut row, "Compressão", &compression, &mut ssh_rows);

    let agent_forward = gtk::Switch::new();
    agent_forward.set_halign(gtk::Align::Start);
    attach_group(
        &grid,
        &mut row,
        "Encaminhar agente",
        &agent_forward,
        &mut ssh_rows,
    );

    let x11 = gtk::Switch::new();
    x11.set_halign(gtk::Align::Start);
    attach_group(&grid, &mut row, "X11 forwarding", &x11, &mut ssh_rows);

    let forwards: Rc<RefCell<Vec<ForwardRule>>> = Rc::new(RefCell::new(
        existing
            .as_ref()
            .map(|session| session.ssh.forwards.clone())
            .unwrap_or_default(),
    ));
    let forwards_button = gtk::Button::with_label(&forwards_label(&forwards.borrow()));
    forwards_button.set_halign(gtk::Align::Start);
    attach_group(
        &grid,
        &mut row,
        "Encaminhamento de portas",
        &forwards_button,
        &mut ssh_rows,
    );

    let extra = gtk::Entry::new();
    extra.set_hexpand(true);
    extra.set_placeholder_text(Some("separados por espaço"));
    attach_group(&grid, &mut row, "Argumentos extras", &extra, &mut ssh_rows);

    let serial_port = gtk::Entry::new();
    serial_port.set_hexpand(true);
    serial_port.set_placeholder_text(Some("/dev/ttyUSB0"));
    let serial_scan = gtk::Button::with_label("Procurar…");
    let serial_port_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    serial_port_row.append(&serial_port);
    serial_port_row.append(&serial_scan);
    attach_group(
        &grid,
        &mut row,
        "Porta serial",
        &serial_port_row,
        &mut serial_rows,
    );

    let baud_labels: Vec<String> = SERIAL_BAUDS.iter().map(u32::to_string).collect();
    let baud_refs: Vec<&str> = baud_labels.iter().map(String::as_str).collect();
    let serial_baud = gtk::DropDown::from_strings(&baud_refs);
    serial_baud.set_selected(5);
    attach_group(
        &grid,
        &mut row,
        "Velocidade (baud)",
        &serial_baud,
        &mut serial_rows,
    );

    let serial_bits = gtk::DropDown::from_strings(&["5", "6", "7", "8"]);
    serial_bits.set_selected(3);
    attach_group(
        &grid,
        &mut row,
        "Bits de dados",
        &serial_bits,
        &mut serial_rows,
    );

    let serial_parity = gtk::DropDown::from_strings(&["Nenhuma", "Par", "Ímpar"]);
    attach_group(
        &grid,
        &mut row,
        "Paridade",
        &serial_parity,
        &mut serial_rows,
    );

    let serial_stop = gtk::DropDown::from_strings(&["1", "2"]);
    attach_group(&grid, &mut row, "Stop bits", &serial_stop, &mut serial_rows);

    let serial_flow =
        gtk::DropDown::from_strings(&["Nenhum", "Software (XON/XOFF)", "Hardware (RTS/CTS)"]);
    attach_group(
        &grid,
        &mut row,
        "Controle de fluxo",
        &serial_flow,
        &mut serial_rows,
    );

    {
        let popover = gtk::Popover::new();
        popover.set_parent(&serial_scan);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        let ports = serial_ports();
        if ports.is_empty() {
            let empty = adw::ActionRow::new();
            empty.set_title("Nenhuma porta encontrada");
            empty.set_subtitle("Plugue o adaptador USB-serial e tente de novo");
            let item = gtk::ListBoxRow::new();
            item.set_child(Some(&empty));
            item.set_activatable(false);
            list.append(&item);
        }
        for (path, label) in ports {
            let item_row = adw::ActionRow::new();
            item_row.set_title(&label);
            item_row.set_subtitle(&path);
            let item = gtk::ListBoxRow::new();
            item.set_child(Some(&item_row));
            list.append(&item);
        }
        popover.set_child(Some(&list));
        {
            let popover = popover.clone();
            let entry = serial_port.clone();
            list.connect_row_activated(move |_, item| {
                if let Some(child) = item
                    .child()
                    .and_then(|c| c.downcast::<adw::ActionRow>().ok())
                {
                    if let Some(path) = child.subtitle() {
                        entry.set_text(&path);
                    }
                }
                popover.popdown();
            });
        }
        {
            let popover = popover.clone();
            serial_scan.connect_clicked(move |_| popover.popup());
        }
    }

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
        if let Some(serial) = session.serial.as_ref() {
            kind.set_selected(1);
            serial_port.set_text(&serial.port);
            if let Some(index) = SERIAL_BAUDS.iter().position(|baud| *baud == serial.baud) {
                serial_baud.set_selected(index as u32);
            }
            serial_bits.set_selected(u32::from(serial.data_bits.saturating_sub(5).min(3)));
            serial_parity.set_selected(match serial.parity {
                SerialParity::None => 0,
                SerialParity::Even => 1,
                SerialParity::Odd => 2,
            });
            serial_stop.set_selected(u32::from(serial.stop_bits.saturating_sub(1).min(1)));
            serial_flow.set_selected(match serial.flow {
                SerialFlow::None => 0,
                SerialFlow::Software => 1,
                SerialFlow::Hardware => 2,
            });
        }
    }

    let apply_kind: Rc<dyn Fn(bool)> = {
        let ssh_rows = ssh_rows.clone();
        let serial_rows = serial_rows.clone();
        Rc::new(move |serial| {
            for widget in &ssh_rows {
                widget.set_visible(!serial);
            }
            for widget in &serial_rows {
                widget.set_visible(serial);
            }
        })
    };
    apply_kind(
        existing
            .as_ref()
            .is_some_and(|session| session.serial.is_some()),
    );
    {
        let apply_kind = apply_kind.clone();
        kind.connect_selected_notify(move |dropdown| apply_kind(dropdown.selected() == 1));
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
    {
        let dialog = dialog.clone();
        let forwards = forwards.clone();
        let label = forwards_button.clone();
        forwards_button.connect_clicked(move |_| {
            let current = forwards.borrow().clone();
            let forwards = forwards.clone();
            let label = label.clone();
            crate::forwards_dialog::show(
                &dialog,
                current,
                Rc::new(move |rules: Vec<ForwardRule>| {
                    *forwards.borrow_mut() = rules;
                    label.set_label(&forwards_label(&forwards.borrow()));
                }),
            );
        });
    }

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
        session.ssh.forwards = forwards.borrow().clone();
        session.ssh.extra_args = extra
            .text()
            .split_whitespace()
            .map(str::to_string)
            .collect();

        if kind.selected() == 1 {
            session.serial = Some(SerialOptions {
                port: serial_port.text().trim().to_string(),
                baud: SERIAL_BAUDS
                    .get(serial_baud.selected() as usize)
                    .copied()
                    .unwrap_or(9600),
                data_bits: 5 + serial_bits.selected() as u8,
                parity: match serial_parity.selected() {
                    1 => SerialParity::Even,
                    2 => SerialParity::Odd,
                    _ => SerialParity::None,
                },
                stop_bits: 1 + serial_stop.selected() as u8,
                flow: match serial_flow.selected() {
                    1 => SerialFlow::Software,
                    2 => SerialFlow::Hardware,
                    _ => SerialFlow::None,
                },
            });
            session.host.clear();
            session.user.clear();
        } else {
            session.serial = None;
        }

        if session.name.is_empty() {
            session.name = match session.serial.as_ref() {
                Some(serial) => serial.port.clone(),
                None if session.host.is_empty() => "Nova sessão".to_string(),
                None => session.host.clone(),
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

fn attach_group(
    grid: &gtk::Grid,
    row: &mut i32,
    label: &str,
    widget: &impl IsA<gtk::Widget>,
    group: &mut Vec<gtk::Widget>,
) {
    let text = gtk::Label::new(Some(label));
    text.set_xalign(1.0);
    text.add_css_class("dim-label");
    grid.attach(&text, 0, *row, 1, 1);
    grid.attach(widget, 1, *row, 1, 1);
    group.push(text.clone().upcast());
    group.push(widget.clone().upcast());
    *row += 1;
}

fn serial_ports() -> Vec<(String, String)> {
    let mut ports: Vec<(String, String)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/serial/by-id") {
        for entry in entries.flatten() {
            let path = entry.path();
            let target = std::fs::canonicalize(&path)
                .map(|real| real.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());
            let label = entry.file_name().to_string_lossy().to_string();
            ports.push((target, label));
        }
    }
    if let Ok(entries) = std::fs::read_dir("/dev") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let known = name.starts_with("ttyUSB")
                || name.starts_with("ttyACM")
                || name.starts_with("ttyS");
            if !known {
                continue;
            }
            let path = entry.path().display().to_string();
            if ports.iter().any(|(existing, _)| *existing == path) {
                continue;
            }
            ports.push((path.clone(), name));
        }
    }
    ports.sort_by(|a, b| a.0.cmp(&b.0));
    ports
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

fn forwards_label(rules: &[ForwardRule]) -> String {
    match rules.len() {
        0 => "Nenhuma".to_string(),
        1 => "1 regra".to_string(),
        count => format!("{count} regras"),
    }
}
