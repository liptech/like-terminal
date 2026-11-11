use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lt_core::{CursorShape, HighlightRule, Profile, Store};

use crate::terminal::PALETTES;

#[derive(Clone)]
pub struct PrefsHooks {
    pub set_profile: Rc<dyn Fn(Profile)>,
    pub set_confirm_close: Rc<dyn Fn(bool)>,
    pub set_restore_tabs: Rc<dyn Fn(bool)>,
    pub set_highlight: Rc<dyn Fn(Vec<HighlightRule>, bool)>,
    pub set_security: Rc<dyn Fn(Option<String>, bool)>,
}

pub fn show(parent: &adw::ApplicationWindow, store: &Store, hooks: PrefsHooks) {
    let profile = Rc::new(RefCell::new(store.default_profile().clone()));

    let window = adw::PreferencesWindow::new();
    window.set_transient_for(Some(parent));
    window.set_modal(true);
    window.set_title(Some("Preferências"));
    window.set_default_width(620);
    window.set_default_height(640);
    window.set_search_enabled(true);

    let page = adw::PreferencesPage::new();
    page.set_title("Terminal");
    page.set_icon_name(Some("utilities-terminal-symbolic"));

    let appearance = adw::PreferencesGroup::new();
    appearance.set_title("Aparência");

    let font_row = adw::EntryRow::new();
    font_row.set_title("Fonte");
    font_row.set_text(&profile.borrow().font);
    appearance.add(&font_row);

    let palette_row = adw::ActionRow::new();
    palette_row.set_title("Paleta de cores");
    let palette_labels: Vec<&str> = PALETTES.iter().map(|(_, label)| *label).collect();
    let palettes = gtk::DropDown::from_strings(&palette_labels);
    palettes.set_valign(gtk::Align::Center);
    let current_palette = profile.borrow().palette.clone();
    let palette_index = PALETTES
        .iter()
        .position(|(id, _)| *id == current_palette)
        .unwrap_or(0);
    palettes.set_selected(palette_index as u32);
    palette_row.add_suffix(&palettes);
    appearance.add(&palette_row);

    let scroll_row = adw::ActionRow::new();
    scroll_row.set_title("Linhas de histórico");
    let scrollback = gtk::SpinButton::with_range(100.0, 1_000_000.0, 1000.0);
    scrollback.set_valign(gtk::Align::Center);
    scrollback.set_value(f64::from(profile.borrow().scrollback));
    scroll_row.add_suffix(&scrollback);
    appearance.add(&scroll_row);

    let cursor_row = adw::ActionRow::new();
    cursor_row.set_title("Cursor");
    let cursors = gtk::DropDown::from_strings(&["Bloco", "Barra", "Sublinhado"]);
    cursors.set_valign(gtk::Align::Center);
    cursors.set_selected(match profile.borrow().cursor {
        CursorShape::Block => 0,
        CursorShape::Ibeam => 1,
        CursorShape::Underline => 2,
    });
    cursor_row.add_suffix(&cursors);
    appearance.add(&cursor_row);

    let syntax_row = adw::SwitchRow::new();
    syntax_row.set_title("Realce de sintaxe no shell");
    syntax_row
        .set_subtitle("Usa o fish, quando instalado, para colorir comandos enquanto você digita");
    syntax_row.set_active(profile.borrow().syntax_highlight);
    appearance.add(&syntax_row);

    let output_row = adw::SwitchRow::new();
    output_row.set_title("Realce de saída");
    output_row.set_subtitle(
        "Colore palavras-chave (Error, Warning, Info, IPs) no texto que chega do host",
    );
    output_row.set_active(profile.borrow().highlight_output);
    appearance.add(&output_row);

    page.add(&appearance);

    let behavior = adw::PreferencesGroup::new();
    behavior.set_title("Comportamento");

    let copy_row = adw::SwitchRow::new();
    copy_row.set_title("Copiar ao selecionar");
    copy_row.set_active(profile.borrow().copy_on_select);
    behavior.add(&copy_row);

    let restore_row = adw::SwitchRow::new();
    restore_row.set_title("Restaurar abas ao abrir");
    restore_row.set_subtitle("Reabre as abas e divisões da última sessão ao iniciar o app");
    restore_row.set_active(store.settings.restore_tabs);
    behavior.add(&restore_row);

    let confirm_row = adw::SwitchRow::new();
    confirm_row.set_title("Confirmar ao fechar sessões ativas");
    confirm_row.set_active(store.settings.confirm_close_running);
    behavior.add(&confirm_row);

    page.add(&behavior);
    window.add(&page);

    {
        let profile = profile.clone();
        let hooks = hooks.set_profile.clone();
        font_row.connect_changed(move |row| {
            profile.borrow_mut().font = row.text().to_string();
            hooks(profile.borrow().clone());
        });
    }
    {
        let profile = profile.clone();
        let hooks = hooks.set_profile.clone();
        palettes.connect_selected_notify(move |dropdown| {
            let index = dropdown.selected() as usize;
            let name = PALETTES.get(index).map(|(id, _)| *id).unwrap_or("system");
            profile.borrow_mut().palette = name.to_string();
            hooks(profile.borrow().clone());
        });
    }
    {
        let profile = profile.clone();
        let hooks = hooks.set_profile.clone();
        scrollback.connect_value_changed(move |spin| {
            profile.borrow_mut().scrollback = spin.value().max(100.0) as u32;
            hooks(profile.borrow().clone());
        });
    }
    {
        let profile = profile.clone();
        let hooks = hooks.set_profile.clone();
        cursors.connect_selected_notify(move |dropdown| {
            profile.borrow_mut().cursor = match dropdown.selected() {
                1 => CursorShape::Ibeam,
                2 => CursorShape::Underline,
                _ => CursorShape::Block,
            };
            hooks(profile.borrow().clone());
        });
    }
    {
        let profile = profile.clone();
        let hooks = hooks.set_profile.clone();
        output_row.connect_active_notify(move |row| {
            profile.borrow_mut().highlight_output = row.is_active();
            hooks(profile.borrow().clone());
        });
    }
    {
        let profile = profile.clone();
        let hooks = hooks.set_profile.clone();
        syntax_row.connect_active_notify(move |row| {
            profile.borrow_mut().syntax_highlight = row.is_active();
            hooks(profile.borrow().clone());
        });
    }
    {
        let profile = profile.clone();
        let hooks = hooks.set_profile.clone();
        copy_row.connect_active_notify(move |row| {
            profile.borrow_mut().copy_on_select = row.is_active();
            hooks(profile.borrow().clone());
        });
    }
    {
        let hooks = hooks.set_restore_tabs.clone();
        restore_row.connect_active_notify(move |row| hooks(row.is_active()));
    }
    {
        let hooks = hooks.set_confirm_close.clone();
        confirm_row.connect_active_notify(move |row| hooks(row.is_active()));
    }

    window.add(&crate::highlight_page::build(
        store,
        hooks.set_highlight.clone(),
    ));
    window.add(&crate::security_page::build(
        store,
        hooks.set_security.clone(),
    ));

    window.present();
}
