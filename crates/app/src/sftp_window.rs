use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

type Shared = Arc<Mutex<Option<lt_sftp::SftpSession>>>;
type Reload = Rc<dyn Fn()>;

struct Panel {
    root: gtk::Box,
    list: gtk::ListBox,
    stack: gtk::Stack,
    spinner: gtk::Spinner,
    path_entry: gtk::Entry,
    up: gtk::Button,
    refresh: gtk::Button,
    path: RefCell<String>,
    entries: RefCell<Vec<lt_sftp::Entry>>,
}

impl Panel {
    fn set_path(&self, path: &str) {
        *self.path.borrow_mut() = path.to_string();
        self.path_entry.set_text(path);
    }

    fn current(&self) -> String {
        self.path.borrow().clone()
    }

    fn selected(&self) -> Option<lt_sftp::Entry> {
        let row = self.list.selected_row()?;
        let index = row.index();
        self.entries.borrow().get(index as usize).cloned()
    }

    fn show_list(&self, entries: Vec<lt_sftp::Entry>) {
        populate_list(&self.list, &entries);
        *self.entries.borrow_mut() = entries;
        self.stack.set_visible_child_name("content");
        self.spinner.stop();
    }
}

pub fn show(parent: &adw::ApplicationWindow, title: String, config: lt_sftp::SftpConfig) {
    let window = adw::Window::builder()
        .transient_for(parent)
        .title(format!("SFTP — {title}"))
        .default_width(1080)
        .default_height(680)
        .build();

    let header = adw::HeaderBar::new();

    let local = Rc::new(build_panel("Local"));
    let remote = Rc::new(build_panel("Remoto"));

    let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
    paned.set_start_child(Some(&local.root));
    paned.set_end_child(Some(&remote.root));
    paned.set_resize_start_child(true);
    paned.set_resize_end_child(true);
    paned.set_wide_handle(true);
    paned.set_vexpand(true);
    paned.set_position(540);

    let status = gtk::Label::new(Some("Conectando…"));
    status.set_xalign(0.0);
    status.set_hexpand(true);
    status.add_css_class("dim-label");

    let btn_download = button("document-save-symbolic", "Baixar");
    let btn_upload = button("document-send-symbolic", "Enviar");
    let btn_mkdir = button("folder-new-symbolic", "Nova pasta");
    let btn_rename = button("document-edit-symbolic", "Renomear");
    let btn_delete = button("user-trash-symbolic", "Excluir");
    btn_delete.add_css_class("destructive-action");
    let btn_disconnect = gtk::Button::with_label("Desconectar");

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.set_margin_top(6);
    actions.set_margin_bottom(6);
    actions.set_margin_start(12);
    actions.set_margin_end(12);
    actions.append(&status);
    actions.append(&btn_download);
    actions.append(&btn_upload);
    actions.append(&btn_mkdir);
    actions.append(&btn_rename);
    actions.append(&btn_delete);
    actions.append(&btn_disconnect);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&paned);
    body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    body.append(&actions);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&body));

    let toast_overlay = adw::ToastOverlay::new();
    toast_overlay.set_child(Some(&toolbar));
    window.set_content(Some(&toast_overlay));

    let session: Shared = Arc::new(Mutex::new(None));

    let notify: Rc<dyn Fn(String)> = {
        let status = status.clone();
        let toast = toast_overlay.clone();
        Rc::new(move |message: String| {
            status.set_text(&message);
            status.add_css_class("error");
            toast.add_toast(adw::Toast::new(&message));
        })
    };

    let refresh_local: Rc<dyn Fn()> = {
        let panel = local.clone();
        let status = status.clone();
        let notify = notify.clone();
        Rc::new(move || match read_local(&panel.current()) {
            Ok(entries) => {
                panel.show_list(entries.clone());
                status.set_text(&format!("{} itens", entries.len()));
                status.remove_css_class("error");
            }
            Err(error) => notify(error),
        })
    };

    let refresh_remote: Reload = {
        let panel = remote.clone();
        let session = session.clone();
        let status = status.clone();
        let notify = notify.clone();
        Rc::new(move || {
            let path = panel.current();
            panel.stack.set_visible_child_name("spinner");
            panel.spinner.start();
            let panel = panel.clone();
            let status = status.clone();
            let notify = notify.clone();
            run_async(
                {
                    let session = session.clone();
                    move || with_session(&session, |sft| sft.list(&path))
                },
                move |result| match result {
                    Ok(entries) => {
                        panel.show_list(entries.clone());
                        status.set_text(&format!("{} itens", entries.len()));
                        status.remove_css_class("error");
                    }
                    Err(error) => {
                        panel.stack.set_visible_child_name("content");
                        panel.spinner.stop();
                        notify(error);
                    }
                },
            );
        })
    };

    {
        let panel = local.clone();
        let reload = refresh_local.clone();
        let widget = panel.path_entry.clone();
        widget.connect_activate(move |entry| {
            panel.set_path(&entry.text());
            reload();
        });
        let panel = local.clone();
        let reload = refresh_local.clone();
        let widget = panel.up.clone();
        widget.connect_clicked(move |_| {
            let parent = parent_path(&panel.current());
            if parent != panel.current() {
                panel.set_path(&parent);
                reload();
            }
        });
        let reload = refresh_local.clone();
        local.refresh.connect_clicked(move |_| reload());
    }

    {
        let panel = remote.clone();
        let reload = refresh_remote.clone();
        let widget = panel.path_entry.clone();
        widget.connect_activate(move |entry| {
            panel.set_path(&entry.text());
            reload();
        });
        let panel = remote.clone();
        let reload = refresh_remote.clone();
        let widget = panel.up.clone();
        widget.connect_clicked(move |_| {
            let parent = parent_path(&panel.current());
            if parent != panel.current() {
                panel.set_path(&parent);
                reload();
            }
        });
        let reload = refresh_remote.clone();
        remote.refresh.connect_clicked(move |_| reload());
    }

    {
        let local_panel = local.clone();
        let remote_panel = remote.clone();
        let session = session.clone();
        let status = status.clone();
        let notify = notify.clone();
        let reload = refresh_remote.clone();
        local.list.connect_row_activated(move |_, row| {
            let index = row.index();
            let Some(entry) = local_panel.entries.borrow().get(index as usize).cloned() else {
                return;
            };
            if entry.is_dir {
                local_panel.set_path(&entry.path);
                if let Ok(entries) = read_local(&entry.path) {
                    local_panel.show_list(entries);
                }
                return;
            }
            let remote_path = join(&remote_panel.current(), &entry.name);
            let local_path = PathBuf::from(&entry.path);
            let status = status.clone();
            let notify = notify.clone();
            let reload = reload.clone();
            status.set_text("Enviando…");
            run_async(
                {
                    let session = session.clone();
                    move || with_session(&session, |sft| sft.upload(&local_path, &remote_path))
                },
                move |result| match result {
                    Ok(bytes) => {
                        status.set_text(&format!("Enviado — {}", format_size(bytes)));
                        status.remove_css_class("error");
                        reload();
                    }
                    Err(error) => notify(error),
                },
            );
        });
    }

    {
        let remote_panel = remote.clone();
        let local_panel = local.clone();
        let session = session.clone();
        let status = status.clone();
        let notify = notify.clone();
        let reload_local = refresh_local.clone();
        let reload_remote = refresh_remote.clone();
        remote.list.connect_row_activated(move |_, row| {
            let index = row.index();
            let Some(entry) = remote_panel.entries.borrow().get(index as usize).cloned() else {
                return;
            };
            if entry.is_dir {
                remote_panel.set_path(&entry.path);
                reload_remote();
                return;
            }
            let local_path = PathBuf::from(local_panel.current()).join(&entry.name);
            let remote_path = entry.path.clone();
            let status = status.clone();
            let notify = notify.clone();
            let reload = reload_local.clone();
            status.set_text("Baixando…");
            run_async(
                {
                    let session = session.clone();
                    move || with_session(&session, |sft| sft.download(&remote_path, &local_path))
                },
                move |result| match result {
                    Ok(bytes) => {
                        status.set_text(&format!("Baixado — {}", format_size(bytes)));
                        status.remove_css_class("error");
                        reload();
                    }
                    Err(error) => notify(error),
                },
            );
        });
    }

    {
        let local_panel = local.clone();
        let remote_panel = remote.clone();
        let session = session.clone();
        let status = status.clone();
        let notify = notify.clone();
        let reload = refresh_remote.clone();
        btn_upload.connect_clicked(move |_| {
            let Some(entry) = local_panel.selected() else {
                notify("Selecione um arquivo local.".to_string());
                return;
            };
            if entry.is_dir {
                notify("Escolha um arquivo, não uma pasta.".to_string());
                return;
            }
            let remote_path = join(&remote_panel.current(), &entry.name);
            let local_path = PathBuf::from(&entry.path);
            let status = status.clone();
            let notify = notify.clone();
            let reload = reload.clone();
            status.set_text("Enviando…");
            run_async(
                {
                    let session = session.clone();
                    move || with_session(&session, |sft| sft.upload(&local_path, &remote_path))
                },
                move |result| match result {
                    Ok(bytes) => {
                        status.set_text(&format!("Enviado — {}", format_size(bytes)));
                        status.remove_css_class("error");
                        reload();
                    }
                    Err(error) => notify(error),
                },
            );
        });
    }

    {
        let remote_panel = remote.clone();
        let local_panel = local.clone();
        let session = session.clone();
        let status = status.clone();
        let notify = notify.clone();
        let reload = refresh_local.clone();
        btn_download.connect_clicked(move |_| {
            let Some(entry) = remote_panel.selected() else {
                notify("Selecione um arquivo remoto.".to_string());
                return;
            };
            if entry.is_dir {
                notify("Escolha um arquivo, não uma pasta.".to_string());
                return;
            }
            let local_path = PathBuf::from(local_panel.current()).join(&entry.name);
            let remote_path = entry.path.clone();
            let status = status.clone();
            let notify = notify.clone();
            let reload = reload.clone();
            status.set_text("Baixando…");
            run_async(
                {
                    let session = session.clone();
                    move || with_session(&session, |sft| sft.download(&remote_path, &local_path))
                },
                move |result| match result {
                    Ok(bytes) => {
                        status.set_text(&format!("Baixado — {}", format_size(bytes)));
                        status.remove_css_class("error");
                        reload();
                    }
                    Err(error) => notify(error),
                },
            );
        });
    }

    {
        let remote_panel = remote.clone();
        let parent = parent.clone();
        let session = session.clone();
        let status = status.clone();
        let notify = notify.clone();
        let reload = refresh_remote.clone();
        btn_mkdir.connect_clicked(move |_| {
            let base = remote_panel.current();
            let session = session.clone();
            let status = status.clone();
            let notify = notify.clone();
            let reload = reload.clone();
            crate::session_dialog::ask_text(&parent, "Nova pasta", "", move |name| {
                let path = join(&base, &name);
                let status = status.clone();
                let notify = notify.clone();
                let reload = reload.clone();
                run_async(
                    {
                        let session = session.clone();
                        move || with_session(&session, |sft| sft.create_dir(&path))
                    },
                    move |result| match result {
                        Ok(()) => {
                            status.set_text("Pasta criada");
                            status.remove_css_class("error");
                            reload();
                        }
                        Err(error) => notify(error),
                    },
                );
            });
        });
    }

    {
        let remote_panel = remote.clone();
        let parent = parent.clone();
        let session = session.clone();
        let notify = notify.clone();
        let reload = refresh_remote.clone();
        btn_rename.connect_clicked(move |_| {
            let Some(entry) = remote_panel.selected() else {
                notify("Selecione um arquivo remoto.".to_string());
                return;
            };
            let base = remote_panel.current();
            let session = session.clone();
            let notify = notify.clone();
            let reload = reload.clone();
            crate::session_dialog::ask_text(&parent, "Renomear", &entry.name, move |name| {
                let target = join(&base, &name);
                let from = entry.path.clone();
                let notify = notify.clone();
                let reload = reload.clone();
                run_async(
                    {
                        let session = session.clone();
                        move || with_session(&session, |sft| sft.rename(&from, &target))
                    },
                    move |result| match result {
                        Ok(()) => reload(),
                        Err(error) => notify(error),
                    },
                );
            });
        });
    }

    {
        let remote_panel = remote.clone();
        let session = session.clone();
        let notify = notify.clone();
        let reload = refresh_remote.clone();
        btn_delete.connect_clicked(move |_| {
            let Some(entry) = remote_panel.selected() else {
                notify("Selecione um arquivo remoto.".to_string());
                return;
            };
            let notify = notify.clone();
            let reload = reload.clone();
            run_async(
                {
                    let session = session.clone();
                    move || {
                        with_session(&session, |sft| {
                            if entry.is_dir {
                                sft.remove_dir(&entry.path)
                            } else {
                                sft.remove_file(&entry.path)
                            }
                        })
                    }
                },
                move |result| match result {
                    Ok(()) => reload(),
                    Err(error) => notify(error),
                },
            );
        });
    }

    {
        let close_window = window.clone();
        btn_disconnect.connect_clicked(move |_| close_window.close());
    }

    {
        let session = session.clone();
        window.connect_close_request(move |_| {
            if let Ok(mut guard) = session.lock() {
                if let Some(sft) = guard.take() {
                    sft.close();
                }
            }
            gtk::glib::Propagation::Proceed
        });
    }

    *local.path.borrow_mut() = dirs::home_dir()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string());
    local.path_entry.set_text(&local.path.borrow());
    refresh_local();

    remote.set_path("/");
    remote.stack.set_visible_child_name("spinner");
    remote.spinner.start();

    {
        let remote_panel = remote.clone();
        let session = session.clone();
        let status = status.clone();
        let notify = notify.clone();
        let refresh = refresh_remote.clone();
        let buttons = [
            btn_download.clone(),
            btn_upload.clone(),
            btn_mkdir.clone(),
            btn_rename.clone(),
            btn_delete.clone(),
            btn_disconnect.clone(),
        ];
        run_async(
            move || lt_sftp::SftpSession::connect(config).map_err(|error| error.to_string()),
            move |result| match result {
                Ok(sft) => {
                    let home = sft.home().unwrap_or_else(|_| "/".to_string());
                    if let Ok(mut guard) = session.lock() {
                        *guard = Some(sft);
                    }
                    remote_panel.set_path(&home);
                    for button in &buttons {
                        button.set_sensitive(true);
                    }
                    remote_panel.stack.set_visible_child_name("content");
                    remote_panel.spinner.stop();
                    status.set_text("Conectado");
                    status.remove_css_class("error");
                    refresh();
                }
                Err(error) => {
                    remote_panel.stack.set_visible_child_name("content");
                    remote_panel.spinner.stop();
                    notify(error);
                }
            },
        );
    }

    window.present();
}

fn build_panel(title: &str) -> Panel {
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_activate_on_single_click(false);
    list.add_css_class("boxed-list");

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_child(Some(&list));
    scroll.set_vexpand(true);

    let spinner = gtk::Spinner::new();
    spinner.set_size_request(32, 32);
    spinner.set_halign(gtk::Align::Center);
    spinner.set_valign(gtk::Align::Center);

    let stack = gtk::Stack::new();
    stack.add_named(&scroll, Some("content"));
    stack.add_named(&spinner, Some("spinner"));
    stack.set_vexpand(true);

    let path_entry = gtk::Entry::new();
    path_entry.set_hexpand(true);
    path_entry.set_placeholder_text(Some("Caminho"));

    let up = gtk::Button::from_icon_name("go-up-symbolic");
    up.set_tooltip_text(Some("Pasta acima"));
    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.set_tooltip_text(Some("Atualizar"));

    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.add_css_class("heading");

    let nav = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    nav.append(&path_entry);
    nav.append(&up);
    nav.append(&refresh);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    root.set_margin_top(8);
    root.set_margin_bottom(8);
    root.set_margin_start(8);
    root.set_margin_end(8);
    root.append(&heading);
    root.append(&nav);
    root.append(&stack);

    Panel {
        root,
        list,
        stack,
        spinner,
        path_entry,
        up,
        refresh,
        path: RefCell::new(String::new()),
        entries: RefCell::new(Vec::new()),
    }
}

fn populate_list(list: &gtk::ListBox, entries: &[lt_sftp::Entry]) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let mut sorted: Vec<&lt_sftp::Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for entry in sorted {
        let row = adw::ActionRow::new();
        row.set_title(&entry.name);
        let icon = gtk::Image::from_icon_name(if entry.is_dir {
            "folder-symbolic"
        } else {
            "text-x-generic-symbolic"
        });
        row.add_prefix(&icon);
        let details = if entry.is_dir {
            "Pasta".to_string()
        } else {
            format!(
                "{} · {}",
                format_size(entry.size),
                format_date(entry.modified)
            )
        };
        row.set_subtitle(&details);
        list.append(&row);
    }
}

fn read_local(path: &str) -> Result<Vec<lt_sftp::Entry>, String> {
    let dir = std::fs::read_dir(path).map_err(|error| format!("{path}: {error}"))?;
    let mut entries = Vec::new();
    for item in dir.flatten() {
        let name = item.file_name().to_string_lossy().to_string();
        let full = item.path();
        let metadata = match std::fs::symlink_metadata(&full) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let is_symlink = metadata.file_type().is_symlink();
        let target = if is_symlink {
            std::fs::metadata(&full).ok()
        } else {
            None
        };
        let is_dir = target
            .as_ref()
            .map(|meta| meta.is_dir())
            .unwrap_or_else(|| metadata.is_dir());
        let size = target
            .as_ref()
            .map(|meta| meta.len())
            .unwrap_or_else(|| metadata.len());
        entries.push(lt_sftp::Entry {
            name,
            path: full.to_string_lossy().to_string(),
            is_dir,
            is_symlink,
            size,
            mode: 0,
            modified: metadata.modified().ok().and_then(|time| {
                time.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|duration| duration.as_secs() as i64)
            }),
        });
    }
    Ok(entries)
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn format_date(epoch: Option<i64>) -> String {
    let Some(epoch) = epoch else {
        return "—".to_string();
    };
    gtk::glib::DateTime::from_unix_local(epoch)
        .ok()
        .and_then(|date| date.format("%d/%m/%Y %H:%M").ok())
        .map(|text| text.to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn parent_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return "/".to_string();
    }
    match trimmed.rfind('/') {
        Some(0) => "/".to_string(),
        Some(index) => trimmed[..index].to_string(),
        None => "/".to_string(),
    }
}

fn join(base: &str, name: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), name)
}

fn button(icon: &str, label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.set_icon_name(icon);
    button
}

fn with_session<T>(
    session: &Shared,
    action: impl FnOnce(&lt_sftp::SftpSession) -> Result<T, lt_sftp::SftpError>,
) -> Result<T, String> {
    let guard = session
        .lock()
        .map_err(|_| "sessão SFTP indisponível".to_string())?;
    match guard.as_ref() {
        Some(sft) => action(sft).map_err(|error| error.to_string()),
        None => Err("Não conectado".to_string()),
    }
}

fn run_async<T, W, D>(work: W, done: D)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    D: FnOnce(T) + 'static,
{
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    let mut done = Some(done);
    gtk::glib::timeout_add_local(Duration::from_millis(20), move || {
        match receiver.try_recv() {
            Ok(value) => {
                if let Some(done) = done.take() {
                    done(value);
                }
                gtk::glib::ControlFlow::Break
            }
            Err(TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
            Err(TryRecvError::Disconnected) => gtk::glib::ControlFlow::Break,
        }
    });
}
