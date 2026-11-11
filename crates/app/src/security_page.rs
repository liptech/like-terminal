use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lt_core::auth::{hash_password, otpauth_uri, random_secret};
use lt_core::secrets::{account_totp, KeyringStore, SecretStore};
use lt_core::Store;

pub fn build(store: &Store, on_change: Rc<dyn Fn(Option<String>, bool)>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_title("Segurança");
    page.set_icon_name(Some("changes-prevent-symbolic"));

    let lock = Rc::new(RefCell::new(store.settings.lock_hash.clone()));
    let totp = Rc::new(Cell::new(store.settings.totp_enabled));

    let password_group = adw::PreferencesGroup::new();
    password_group.set_title("Bloqueio do aplicativo");

    let password_row = adw::ActionRow::new();
    password_row.set_title("Senha de desbloqueio");
    let password_button = gtk::Button::with_label("Definir…");
    password_button.set_valign(gtk::Align::Center);
    password_row.add_suffix(&password_button);
    password_group.add(&password_row);
    page.add(&password_group);

    let totp_group = adw::PreferencesGroup::new();
    totp_group.set_title("Verificação em duas etapas");
    let totp_row = adw::SwitchRow::new();
    totp_row.set_title("Exigir código TOTP");
    totp_row.set_subtitle("Compatível com Google Authenticator, Aegis e 1Password");
    totp_row.set_active(totp.get());
    totp_group.add(&totp_row);
    page.add(&totp_group);

    let note_group = adw::PreferencesGroup::new();
    let note = adw::ActionRow::new();
    note.set_title("O bloqueio é solicitado toda vez que o app é aberto.");
    note_group.add(&note);
    page.add(&note_group);

    let refresh = {
        let password_row = password_row.clone();
        let password_button = password_button.clone();
        let lock = lock.clone();
        Rc::new(move || {
            if lock.borrow().is_some() {
                password_row.set_subtitle("Ativa");
                password_button.set_label("Alterar…");
            } else {
                password_row.set_subtitle("Desativada");
                password_button.set_label("Definir…");
            }
        })
    };
    refresh();

    {
        let on_change = on_change.clone();
        let lock = lock.clone();
        let totp = totp.clone();
        let refresh = refresh.clone();
        password_button.connect_clicked(move |button| {
            show_password_dialog(
                button.upcast_ref::<gtk::Widget>(),
                on_change.clone(),
                lock.clone(),
                totp.clone(),
                refresh.clone(),
            );
        });
    }

    {
        let on_change = on_change.clone();
        let lock = lock.clone();
        let totp = totp.clone();
        let refresh = refresh.clone();
        totp_row.connect_active_notify(move |row| {
            if row.is_active() {
                enable_totp(row, &on_change, &lock, &totp);
            } else {
                let keyring = KeyringStore::new();
                let _ = keyring.delete(&account_totp());
                totp.set(false);
                (on_change)(lock.borrow().clone(), false);
            }
            refresh();
        });
    }

    page
}

fn show_password_dialog(
    parent: &gtk::Widget,
    on_change: Rc<dyn Fn(Option<String>, bool)>,
    lock: Rc<RefCell<Option<String>>>,
    totp: Rc<Cell<bool>>,
    refresh: Rc<dyn Fn()>,
) {
    let dialog = adw::Window::new();
    dialog.set_modal(true);
    dialog.set_default_width(420);
    dialog.set_title(Some("Senha de desbloqueio"));
    if let Some(root) = parent.root().and_downcast::<gtk::Window>() {
        dialog.set_transient_for(Some(&root));
    }

    let header = adw::HeaderBar::new();
    let cancel = gtk::Button::with_label("Cancelar");
    header.pack_start(&cancel);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let first = gtk::PasswordEntry::new();
    first.set_show_peek_icon(true);
    first.set_placeholder_text(Some("Nova senha"));
    content.append(&first);

    let second = gtk::PasswordEntry::new();
    second.set_show_peek_icon(true);
    second.set_placeholder_text(Some("Repita a senha"));
    content.append(&second);

    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_visible(false);
    content.append(&error);

    let save = gtk::Button::with_label("Salvar");
    save.add_css_class("suggested-action");
    content.append(&save);

    let remove = gtk::Button::with_label("Remover senha");
    remove.add_css_class("destructive-action");
    remove.set_visible(lock.borrow().is_some());
    content.append(&remove);

    toolbar.set_content(Some(&content));
    dialog.set_content(Some(&toolbar));
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| dialog.close()
    });

    {
        let dialog = dialog.clone();
        let first = first.clone();
        let second = second.clone();
        let error = error.clone();
        let on_change = on_change.clone();
        let lock = lock.clone();
        let totp = totp.clone();
        let refresh = refresh.clone();
        save.connect_clicked(move |_| {
            let password = first.text().to_string();
            if password.len() < 4 {
                error.set_text("Use pelo menos 4 caracteres.");
                error.set_visible(true);
                return;
            }
            if password != second.text().as_str() {
                error.set_text("As senhas não conferem.");
                error.set_visible(true);
                return;
            }
            match hash_password(&password) {
                Ok(hash) => {
                    *lock.borrow_mut() = Some(hash.clone());
                    (on_change)(Some(hash), totp.get());
                    refresh();
                    dialog.close();
                }
                Err(message) => {
                    error.set_text(&message);
                    error.set_visible(true);
                }
            }
        });
    }

    {
        let dialog = dialog.clone();
        let on_change = on_change.clone();
        let lock = lock.clone();
        let totp = totp.clone();
        let refresh = refresh.clone();
        remove.connect_clicked(move |_| {
            *lock.borrow_mut() = None;
            (on_change)(None, totp.get());
            refresh();
            dialog.close();
        });
    }

    dialog.present();
}

fn enable_totp(
    parent: &adw::SwitchRow,
    on_change: &Rc<dyn Fn(Option<String>, bool)>,
    lock: &Rc<RefCell<Option<String>>>,
    totp: &Rc<Cell<bool>>,
) {
    let secret = match random_secret() {
        Ok(secret) => secret,
        Err(message) => {
            tracing::error!(%message, "falha ao gerar segredo TOTP");
            parent.set_active(false);
            return;
        }
    };

    let keyring = KeyringStore::new();
    if let Err(message) = keyring.set(&account_totp(), &secret) {
        tracing::error!(%message, "falha ao guardar segredo TOTP");
        parent.set_active(false);
        return;
    }

    totp.set(true);
    (on_change)(lock.borrow().clone(), true);
    show_secret_dialog(parent, &secret);
}

fn show_secret_dialog(parent: &adw::SwitchRow, secret: &str) {
    let dialog = adw::Window::new();
    dialog.set_modal(true);
    dialog.set_default_width(460);
    dialog.set_title(Some("Segredo TOTP"));
    if let Some(root) = parent.root().and_downcast::<gtk::Window>() {
        dialog.set_transient_for(Some(&root));
    }

    let header = adw::HeaderBar::new();
    let close = gtk::Button::with_label("Concluir");
    header.pack_end(&close);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let title = gtk::Label::new(Some("Cadastre no autenticador"));
    title.add_css_class("title-4");
    title.set_xalign(0.0);
    content.append(&title);

    let hint = gtk::Label::new(Some(
        "Digite o segredo abaixo no app autenticador ou importe o endereço otpauth.",
    ));
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    hint.add_css_class("dim-label");
    content.append(&hint);

    let secret_label = gtk::Label::new(Some(secret));
    secret_label.set_selectable(true);
    secret_label.set_wrap(true);
    secret_label.set_xalign(0.0);
    secret_label.add_css_class("monospace");
    content.append(&secret_label);

    let uri = otpauth_uri("Like Terminal", "like-terminal", secret);
    let uri_label = gtk::Label::new(Some(&uri));
    uri_label.set_selectable(true);
    uri_label.set_wrap(true);
    uri_label.set_xalign(0.0);
    uri_label.add_css_class("dim-label");
    content.append(&uri_label);

    toolbar.set_content(Some(&content));
    dialog.set_content(Some(&toolbar));

    {
        let dialog = dialog.clone();
        close.connect_clicked(move |_| dialog.close());
    }

    dialog.present();
}
