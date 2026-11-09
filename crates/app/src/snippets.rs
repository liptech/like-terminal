use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lt_core::{Snippet, Store};

pub fn show_manager(
    parent: &adw::ApplicationWindow,
    store: &Store,
    on_change: Rc<dyn Fn(Vec<Snippet>)>,
) {
    let snippets = Rc::new(RefCell::new(store.snippets.clone()));

    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Snippets")
        .default_width(500)
        .default_height(560)
        .build();

    let header = adw::HeaderBar::new();
    let add_button = gtk::Button::from_icon_name("list-add-symbolic");
    add_button.set_tooltip_text(Some("Adicionar snippet"));
    add_button.add_css_class("flat");
    header.pack_start(&add_button);

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();
    group.set_title("Comandos salvos");

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    group.add(&list);

    let add_snippet = gtk::Button::with_label("Adicionar snippet");
    add_snippet.add_css_class("suggested-action");
    add_snippet.set_halign(gtk::Align::Start);
    add_snippet.set_margin_top(6);
    group.add(&add_snippet);

    page.add(&group);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    dialog.set_content(Some(&toolbar));

    rebuild_list(&list, &snippets, parent, &on_change);

    {
        let parent = parent.clone();
        let snippets = snippets.clone();
        let list = list.clone();
        let on_change = on_change.clone();
        add_button.connect_clicked(move |_| {
            edit_snippet(&parent, &snippets, &list, &on_change, None);
            rebuild_list(&list, &snippets, &parent, &on_change);
        });
    }
    {
        let parent = parent.clone();
        let snippets = snippets.clone();
        let list = list.clone();
        let on_change = on_change.clone();
        add_snippet.connect_clicked(move |_| {
            edit_snippet(&parent, &snippets, &list, &on_change, None);
            rebuild_list(&list, &snippets, &parent, &on_change);
        });
    }

    dialog.present();
}

fn rebuild_list(
    list: &gtk::ListBox,
    snippets: &Rc<RefCell<Vec<Snippet>>>,
    parent: &adw::ApplicationWindow,
    on_change: &Rc<dyn Fn(Vec<Snippet>)>,
) {
    list.remove_all();
    if snippets.borrow().is_empty() {
        let empty = gtk::Label::new(Some("Nenhum snippet ainda."));
        empty.add_css_class("dim-label");
        empty.set_margin_top(12);
        empty.set_margin_bottom(12);
        empty.set_xalign(0.0);
        list.append(&empty);
        return;
    }

    for snippet in snippets.borrow().iter() {
        let id = snippet.id;
        let row = adw::ActionRow::new();
        row.set_title(&snippet.name);
        row.set_subtitle(&preview(&snippet.text));

        let edit = gtk::Button::from_icon_name("document-edit-symbolic");
        edit.add_css_class("flat");
        let delete = gtk::Button::from_icon_name("user-trash-symbolic");
        delete.add_css_class("flat");
        delete.add_css_class("destructive-action");

        row.add_suffix(&edit);
        row.add_suffix(&delete);
        list.append(&row);

        {
            let parent = parent.clone();
            let snippets = snippets.clone();
            let list = list.clone();
            let on_change = on_change.clone();
            edit.connect_clicked(move |_| {
                if let Some(index) = snippets.borrow().iter().position(|item| item.id == id) {
                    edit_snippet(&parent, &snippets, &list, &on_change, Some(index));
                    rebuild_list(&list, &snippets, &parent, &on_change);
                }
            });
        }
        {
            let parent = parent.clone();
            let snippets = snippets.clone();
            let list = list.clone();
            let on_change = on_change.clone();
            delete.connect_clicked(move |_| {
                let dialog = adw::MessageDialog::new(
                    Some(&parent),
                    Some("Excluir snippet?"),
                    Some("Esta ação não pode ser desfeita."),
                );
                dialog.add_response("cancel", "Cancelar");
                dialog.add_response("delete", "Excluir");
                dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");

                let snippets = snippets.clone();
                let list = list.clone();
                let on_change = on_change.clone();
                let parent = parent.clone();
                dialog.connect_response(None, move |_dialog, response| {
                    if response == "delete" {
                        snippets.borrow_mut().retain(|item| item.id != id);
                        on_change(snippets.borrow().clone());
                        rebuild_list(&list, &snippets, &parent, &on_change);
                    }
                    _dialog.close();
                });
                dialog.present();
            });
        }
    }
}

fn edit_snippet(
    parent: &adw::ApplicationWindow,
    snippets: &Rc<RefCell<Vec<Snippet>>>,
    list: &gtk::ListBox,
    on_change: &Rc<dyn Fn(Vec<Snippet>)>,
    editing: Option<usize>,
) {
    let initial = editing
        .and_then(|index| snippets.borrow().get(index).cloned())
        .unwrap_or_else(|| Snippet::new("", ""));

    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(if editing.is_some() {
            "Editar snippet"
        } else {
            "Novo snippet"
        })
        .default_width(480)
        .default_height(420)
        .build();

    let header = adw::HeaderBar::new();
    let cancel = gtk::Button::with_label("Cancelar");
    let save = gtk::Button::with_label("Salvar");
    save.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&save);

    let content_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content_box.set_margin_top(18);
    content_box.set_margin_bottom(18);
    content_box.set_margin_start(18);
    content_box.set_margin_end(18);

    let name = gtk::Entry::new();
    name.set_placeholder_text(Some("Nome do snippet"));
    name.set_hexpand(true);
    content_box.append(&name);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(140)
        .build();
    let text_view = gtk::TextView::new();
    text_view.set_monospace(true);
    text_view.add_css_class("monospace");
    text_view.set_wrap_mode(gtk::WrapMode::WordChar);
    scrolled.set_child(Some(&text_view));
    content_box.append(&scrolled);

    let send_enter = adw::SwitchRow::new();
    send_enter.set_title("Enviar Enter ao aplicar");
    content_box.append(&send_enter);

    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_wrap(true);
    error.set_xalign(0.0);
    error.set_visible(false);
    content_box.append(&error);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content_box));
    dialog.set_content(Some(&toolbar));

    name.set_text(&initial.name);
    let buffer = text_view.buffer();
    buffer.set_text(&initial.text);
    send_enter.set_active(initial.send_enter);

    {
        let dialog = dialog.clone();
        cancel.connect_clicked(move |_| dialog.close());
    }

    let snippets = snippets.clone();
    let list = list.clone();
    let on_change = on_change.clone();
    let parent_for_rebuild = parent.clone();
    let dialog_for_save = dialog.clone();
    let name_for_save = name.clone();
    let text_view_for_save = text_view.clone();
    let send_enter_for_save = send_enter.clone();
    let error_for_save = error.clone();
    save.connect_clicked(move |_| {
        let mut snippet = editing
            .and_then(|index| snippets.borrow().get(index).cloned())
            .unwrap_or_else(|| Snippet::new("", ""));
        snippet.name = name_for_save.text().trim().to_string();
        snippet.text = text_view_for_save
            .buffer()
            .text(
                &text_view_for_save.buffer().start_iter(),
                &text_view_for_save.buffer().end_iter(),
                false,
            )
            .to_string();
        snippet.send_enter = send_enter_for_save.is_active();

        if let Some(problem) = snippet.problem() {
            error_for_save.set_text(problem);
            error_for_save.set_visible(true);
            return;
        }

        if let Some(index) = editing {
            if let Some(slot) = snippets.borrow_mut().get_mut(index) {
                *slot = snippet;
            }
        } else {
            snippets.borrow_mut().push(snippet);
        }

        on_change(snippets.borrow().clone());

        rebuild_list(&list, &snippets, &parent_for_rebuild, &on_change);

        dialog_for_save.close();
    });

    dialog.present();
    name.grab_focus();
}

pub fn show_picker(
    parent: &adw::ApplicationWindow,
    store: &Store,
    on_send: Rc<dyn Fn(Snippet)>,
    on_manage: Rc<dyn Fn()>,
) {
    let snippets = store.snippets.clone();

    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Aplicar snippet")
        .default_width(420)
        .default_height(520)
        .build();

    let header = adw::HeaderBar::new();

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Buscar snippet"));

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_activate_on_single_click(true);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list)
        .build();

    let manage = gtk::Button::with_label("Gerenciar snippets…");
    manage.add_css_class("flat");
    manage.set_halign(gtk::Align::Start);
    manage.set_margin_top(6);
    manage.set_margin_bottom(12);
    manage.set_margin_start(12);
    manage.set_margin_end(12);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&search);
    content.append(&scrolled);
    content.append(&manage);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content));
    dialog.set_content(Some(&toolbar));

    if snippets.is_empty() {
        let empty = gtk::Label::new(Some("Nenhum snippet. Crie um em Gerenciar snippets."));
        empty.add_css_class("dim-label");
        empty.set_margin_top(24);
        empty.set_xalign(0.5);
        list.append(&empty);
    } else {
        for snippet in &snippets {
            let id = snippet.id;
            let row = adw::ActionRow::new();
            row.set_title(&snippet.name);
            row.set_subtitle(&preview(&snippet.text));
            row.set_widget_name(&id.to_string());

            let picked = snippet.clone();
            let send = on_send.clone();
            let close = dialog.clone();
            let gesture = gtk::GestureClick::new();
            gesture.connect_released(move |gesture, _n_press, _x, _y| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                send(picked.clone());
                close.close();
            });
            row.add_controller(gesture);

            list.append(&row);
        }
    }

    let snippets_for_filter = snippets.clone();
    let search_for_filter = search.clone();
    list.set_filter_func(move |row| {
        let query = search_for_filter.text().to_lowercase();
        if query.is_empty() {
            return true;
        }
        let id_text = row.widget_name();
        if let Ok(id) = uuid::Uuid::parse_str(&id_text) {
            if let Some(snippet) = snippets_for_filter.iter().find(|item| item.id == id) {
                return snippet.name.to_lowercase().contains(&query)
                    || snippet.text.to_lowercase().contains(&query);
            }
        }
        true
    });

    {
        let list = list.clone();
        search.connect_search_changed(move |_| list.invalidate_filter());
    }

    {
        let dialog = dialog.clone();
        let on_send = on_send.clone();
        let snippets = snippets.clone();
        list.connect_row_activated(move |_list, row| {
            let id_text = row.widget_name();
            if let Ok(id) = uuid::Uuid::parse_str(&id_text) {
                if let Some(snippet) = snippets.iter().find(|item| item.id == id) {
                    on_send(snippet.clone());
                    dialog.close();
                }
            }
        });
    }

    {
        let dialog = dialog.clone();
        let on_manage = on_manage.clone();
        manage.connect_clicked(move |_| {
            dialog.close();
            on_manage();
        });
    }

    dialog.present();
    search.grab_focus();
}

fn preview(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    let count = line.chars().count();
    if count > 60 {
        let mut value: String = line.chars().take(60).collect();
        value.push('…');
        value
    } else {
        line.to_string()
    }
}
