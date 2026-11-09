use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lt_core::{HighlightRule, Store};

pub fn build(
    store: &Store,
    on_change: Rc<dyn Fn(Vec<HighlightRule>, bool)>,
) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_title("Realce");
    page.set_icon_name(Some("emblem-important-symbolic"));

    let rules = Rc::new(RefCell::new(store.highlight_rules.clone()));
    let ips = Rc::new(Cell::new(store.highlight_ips));

    let keywords = adw::PreferencesGroup::new();
    keywords.set_title("Palavras-chave");

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    keywords.add(&list);

    let add_button = gtk::Button::new();
    add_button.add_css_class("flat");
    add_button.set_halign(gtk::Align::Start);
    let add_content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    add_content.append(&gtk::Image::from_icon_name("list-add-symbolic"));
    add_content.append(&gtk::Label::new(Some("Adicionar palavra")));
    add_button.set_child(Some(&add_content));
    keywords.add(&add_button);
    page.add(&keywords);

    let ip_group = adw::PreferencesGroup::new();
    ip_group.set_title("Endereços IP");
    let ip_row = adw::SwitchRow::new();
    ip_row.set_title("Colorir endereços IPv4");
    ip_row.set_active(ips.get());
    ip_group.add(&ip_row);
    page.add(&ip_group);

    let note_group = adw::PreferencesGroup::new();
    let note = adw::ActionRow::new();
    note.set_title("As mudanças valem para sessões abertas depois da alteração.");
    note_group.add(&note);
    page.add(&note_group);

    rebuild(&page, &list, &rules, &ips, &on_change);

    {
        let page = page.clone();
        let list = list.clone();
        let rules = rules.clone();
        let ips = ips.clone();
        let on_change = on_change.clone();
        add_button.connect_clicked(move |_| {
            let parent_page = page.clone();
            let callback_page = page.clone();
            let list = list.clone();
            let rules = rules.clone();
            let ips = ips.clone();
            let on_change = on_change.clone();
            show_rule_dialog(
                &parent_page,
                HighlightRule::default(),
                false,
                Rc::new(move |rule| {
                    rules.borrow_mut().push(rule);
                    rebuild(&callback_page, &list, &rules, &ips, &on_change);
                    let snapshot = rules.borrow().clone();
                    on_change(snapshot, ips.get());
                }),
            );
        });
    }

    {
        let page = page.clone();
        let list = list.clone();
        let rules = rules.clone();
        let ips = ips.clone();
        ip_row.connect_active_notify(move |row| {
            ips.set(row.is_active());
            rebuild(&page, &list, &rules, &ips, &on_change);
            let snapshot = rules.borrow().clone();
            on_change(snapshot, ips.get());
        });
    }

    page
}

fn rebuild(
    page: &adw::PreferencesPage,
    list: &gtk::ListBox,
    rules: &Rc<RefCell<Vec<HighlightRule>>>,
    ips: &Rc<Cell<bool>>,
    on_change: &Rc<dyn Fn(Vec<HighlightRule>, bool)>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let snapshot = rules.borrow().clone();
    for (index, rule) in snapshot.into_iter().enumerate() {
        let row = adw::ActionRow::new();
        row.set_title(&rule.keyword);
        if rule.bold {
            row.set_subtitle(&format!("{} · negrito", rule.color));
        } else {
            row.set_subtitle(&rule.color);
        }

        let swatch = gtk::Label::new(None);
        swatch.add_css_class("highlight-swatch");
        swatch.set_markup(&format!(
            "<span foreground=\"{}\" size=\"large\">■</span>",
            rule.color
        ));
        row.add_prefix(&swatch);

        let edit = gtk::Button::from_icon_name("document-edit-symbolic");
        edit.add_css_class("flat");
        edit.set_valign(gtk::Align::Center);
        edit.set_tooltip_text(Some("Editar palavra"));
        row.add_suffix(&edit);

        let delete = gtk::Button::from_icon_name("user-trash-symbolic");
        delete.add_css_class("flat");
        delete.set_valign(gtk::Align::Center);
        delete.set_tooltip_text(Some("Excluir palavra"));
        row.add_suffix(&delete);

        {
            let page = page.clone();
            let list = list.clone();
            let rules = rules.clone();
            let ips = ips.clone();
            let on_change = on_change.clone();
            let initial = rule.clone();
            edit.connect_clicked(move |_| {
                let parent_page = page.clone();
                let callback_page = page.clone();
                let list = list.clone();
                let rules = rules.clone();
                let ips = ips.clone();
                let on_change = on_change.clone();
                show_rule_dialog(
                    &parent_page,
                    initial.clone(),
                    true,
                    Rc::new(move |rule| {
                        {
                            let mut current = rules.borrow_mut();
                            let Some(current_rule) = current.get_mut(index) else {
                                return;
                            };
                            *current_rule = rule;
                        }
                        rebuild(&callback_page, &list, &rules, &ips, &on_change);
                        let snapshot = rules.borrow().clone();
                        on_change(snapshot, ips.get());
                    }),
                );
            });
        }

        {
            let page = page.clone();
            let list = list.clone();
            let rules = rules.clone();
            let ips = ips.clone();
            let on_change = on_change.clone();
            delete.connect_clicked(move |_| {
                {
                    let mut current = rules.borrow_mut();
                    if index >= current.len() {
                        return;
                    }
                    current.remove(index);
                }
                rebuild(&page, &list, &rules, &ips, &on_change);
                let snapshot = rules.borrow().clone();
                on_change(snapshot, ips.get());
            });
        }

        list.append(&row);
    }
}

fn show_rule_dialog(
    page: &adw::PreferencesPage,
    initial: HighlightRule,
    editing: bool,
    on_save: Rc<dyn Fn(HighlightRule)>,
) {
    let dialog = adw::Window::builder()
        .modal(true)
        .title(if editing {
            "Editar palavra"
        } else {
            "Adicionar palavra"
        })
        .default_width(440)
        .default_height(400)
        .build();
    if let Some(parent) = page.root().and_downcast::<gtk::Window>() {
        dialog.set_transient_for(Some(&parent));
    }

    let header = adw::HeaderBar::new();
    let cancel = gtk::Button::with_label("Cancelar");
    let save = gtk::Button::with_label("Salvar");
    save.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&save);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 18);
    content.set_margin_top(18);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);

    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_wrap(true);
    error.set_xalign(0.0);
    error.set_visible(false);
    content.append(&error);

    let fields = adw::PreferencesGroup::new();

    let keyword = adw::EntryRow::new();
    keyword.set_title("Palavra-chave");
    keyword.set_text(&initial.keyword);
    fields.add(&keyword);

    let color_row = adw::ActionRow::new();
    color_row.set_title("Cor");
    let color_dialog = gtk::ColorDialog::new();
    color_dialog.set_with_alpha(false);
    let color_button = gtk::ColorDialogButton::new(Some(color_dialog));
    color_button.set_valign(gtk::Align::Center);
    let initial_color = gtk::gdk::RGBA::parse(&initial.color)
        .unwrap_or(gtk::gdk::RGBA::new(1.0, 0.373, 0.373, 1.0));
    color_button.set_rgba(&initial_color);
    color_row.add_suffix(&color_button);
    fields.add(&color_row);

    let bold = adw::SwitchRow::new();
    bold.set_title("Negrito");
    bold.set_active(initial.bold);
    fields.add(&bold);

    let case_insensitive = adw::SwitchRow::new();
    case_insensitive.set_title("Ignorar maiúsculas e minúsculas");
    case_insensitive.set_active(initial.case_insensitive);
    fields.add(&case_insensitive);

    content.append(&fields);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scrolled));
    dialog.set_content(Some(&toolbar));

    {
        let dialog = dialog.clone();
        cancel.connect_clicked(move |_| dialog.close());
    }

    {
        let dialog = dialog.clone();
        let keyword_entry = keyword.clone();
        save.connect_clicked(move |_| {
            let rgba = color_button.rgba();
            let rule = HighlightRule {
                keyword: keyword_entry.text().trim().to_string(),
                color: format!(
                    "#{:02x}{:02x}{:02x}",
                    (rgba.red() * 255.0).round() as u8,
                    (rgba.green() * 255.0).round() as u8,
                    (rgba.blue() * 255.0).round() as u8
                ),
                bold: bold.is_active(),
                case_insensitive: case_insensitive.is_active(),
            };

            if let Some(problem) = rule.problem() {
                error.set_text(problem);
                error.set_visible(true);
                return;
            }

            dialog.close();
            on_save(rule);
        });
    }

    dialog.present();
    keyword.grab_focus();
}
