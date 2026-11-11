use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lt_core::auth;
use lt_core::secrets::{account_totp, KeyringStore, SecretStore};
use lt_core::Store;

pub fn show(parent: &adw::ApplicationWindow, store: &Store, on_unlock: Rc<dyn Fn()>) {
    let settings = store.settings.clone();
    let Some(lock_hash) = settings.lock_hash.clone() else {
        on_unlock();
        return;
    };

    let window = adw::Window::new();
    window.set_transient_for(Some(parent));
    window.set_modal(true);
    window.set_resizable(false);
    window.set_deletable(false);
    window.set_default_width(380);

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);

    let icon = gtk::Image::from_icon_name("dialog-password-symbolic");
    icon.set_pixel_size(48);
    content.append(&icon);

    let title = gtk::Label::new(Some("Like Terminal bloqueado"));
    title.add_css_class("title-2");
    content.append(&title);

    let subtitle = gtk::Label::new(Some("Informe a senha para acessar suas sessões."));
    subtitle.add_css_class("dim-label");
    subtitle.set_wrap(true);
    content.append(&subtitle);

    let password = gtk::Entry::new();
    password.set_visibility(false);
    password.set_placeholder_text(Some("Senha"));
    password.set_input_purpose(gtk::InputPurpose::Password);
    content.append(&password);

    let code = gtk::Entry::new();
    code.set_placeholder_text(Some("Código de 6 dígitos"));
    code.set_max_length(6);
    code.set_input_purpose(gtk::InputPurpose::Digits);
    code.set_visible(settings.totp_enabled);
    content.append(&code);

    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_visible(false);
    content.append(&error);

    let unlock = gtk::Button::with_label("Desbloquear");
    unlock.add_css_class("suggested-action");
    unlock.add_css_class("pill");
    unlock.set_halign(gtk::Align::Center);
    content.append(&unlock);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content));
    window.set_content(Some(&toolbar));

    let attempt: Rc<dyn Fn()> = {
        let window = window.clone();
        let on_unlock = on_unlock.clone();
        let password = password.clone();
        let code = code.clone();
        let error = error.clone();
        let totp_enabled = settings.totp_enabled;
        Rc::new(move || {
            let entered = password.text().to_string();
            if !auth::verify_password(&entered, &lock_hash) {
                error.set_text("Senha incorreta.");
                error.set_visible(true);
                password.grab_focus();
                return;
            }
            if totp_enabled {
                let secret = KeyringStore::new()
                    .get(&account_totp())
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                if secret.is_empty() || !auth::verify_totp(&secret, code.text().as_str(), now) {
                    error.set_text("Código de verificação inválido.");
                    error.set_visible(true);
                    code.grab_focus();
                    return;
                }
            }
            window.close();
            on_unlock();
        })
    };

    let clicked = attempt.clone();
    unlock.connect_clicked(move |_| clicked());
    let activated = attempt.clone();
    password.connect_activate(move |_| activated());
    let activated = attempt.clone();
    code.connect_activate(move |_| activated());

    window.present();
    password.grab_focus();
}
