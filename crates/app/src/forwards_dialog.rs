use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use lt_core::{ForwardKind, ForwardRule};

type RebuildSlot = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

pub fn show(
    parent: &impl IsA<gtk::Window>,
    rules: Vec<ForwardRule>,
    on_save: Rc<dyn Fn(Vec<ForwardRule>)>,
) {
    let rules = Rc::new(RefCell::new(rules));

    let window = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Encaminhamento de portas")
        .default_width(520)
        .default_height(560)
        .build();

    let header = adw::HeaderBar::new();
    let cancel = gtk::Button::with_label("Cancelar");
    let save = gtk::Button::with_label("Salvar");
    save.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&save);

    let page = adw::PreferencesPage::new();

    let group = adw::PreferencesGroup::new();
    group.set_title("Regras");
    group.set_description(Some("Local (–L), remoto (–R) e SOCKS dinâmico (–D)"));

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    group.add(&list);

    let add_button = gtk::Button::new();
    add_button.add_css_class("flat");
    let add_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    add_box.append(&gtk::Image::from_icon_name("list-add-symbolic"));
    add_box.append(&gtk::Label::new(Some("Adicionar regra")));
    add_button.set_child(Some(&add_box));
    add_button.set_halign(gtk::Align::Start);
    group.add(&add_button);

    page.add(&group);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&page));
    window.set_content(Some(&toolbar));

    let rebuild_slot: RebuildSlot = Rc::new(RefCell::new(None));

    let rebuild_list: Rc<dyn Fn()> = Rc::new({
        let list = list.clone();
        let window = window.clone();
        let rules = rules.clone();
        let slot = rebuild_slot.clone();
        move || {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            let snapshot = rules.borrow().clone();
            if snapshot.is_empty() {
                let row = adw::ActionRow::new();
                row.set_title("Nenhuma regra");
                row.set_subtitle("As conexões usam apenas o terminal SSH");
                list.append(&row);
                return;
            }
            for (idx, rule) in snapshot.iter().enumerate() {
                let row = adw::ActionRow::new();
                row.set_title(&rule.summary());
                let subtitle = match rule.kind {
                    ForwardKind::Local => "Local",
                    ForwardKind::Remote => "Remoto",
                    ForwardKind::Dynamic => "SOCKS dinâmico",
                };
                row.set_subtitle(subtitle);

                let edit = gtk::Button::new();
                edit.add_css_class("flat");
                edit.set_child(Some(&gtk::Image::from_icon_name("document-edit-symbolic")));

                let del = gtk::Button::new();
                del.add_css_class("flat");
                del.add_css_class("destructive-action");
                del.set_child(Some(&gtk::Image::from_icon_name("user-trash-symbolic")));

                row.add_suffix(&edit);
                row.add_suffix(&del);
                list.append(&row);

                let rebuild = slot.borrow().as_ref().unwrap().clone();
                {
                    let rules = rules.clone();
                    let window = window.clone();
                    let rebuild = rebuild.clone();
                    edit.connect_clicked(move |_| {
                        let initial = rules.borrow()[idx].clone();
                        let rules = rules.clone();
                        let window = window.clone();
                        let rebuild = rebuild.clone();
                        show_rule_dialog(
                            &window,
                            initial,
                            Rc::new(move |updated| {
                                rules.borrow_mut()[idx] = updated;
                                rebuild();
                            }),
                        );
                    });
                }

                let rebuild = slot.borrow().as_ref().unwrap().clone();
                {
                    let rules = rules.clone();
                    let rebuild = rebuild.clone();
                    del.connect_clicked(move |_| {
                        rules.borrow_mut().remove(idx);
                        rebuild();
                    });
                }
            }
        }
    }) as Rc<dyn Fn()>;

    *rebuild_slot.borrow_mut() = Some(rebuild_list.clone());

    {
        let window = window.clone();
        let rules = rules.clone();
        let rebuild = rebuild_list.clone();
        add_button.connect_clicked(move |_| {
            let window = window.clone();
            let rules = rules.clone();
            let rebuild = rebuild.clone();
            show_rule_dialog(
                &window,
                ForwardRule::default(),
                Rc::new(move |rule| {
                    rules.borrow_mut().push(rule);
                    rebuild();
                }),
            );
        });
    }

    {
        let window = window.clone();
        cancel.connect_clicked(move |_| window.close());
    }

    {
        let window = window.clone();
        let rules = rules.clone();
        let on_save = on_save.clone();
        save.connect_clicked(move |_| {
            on_save(rules.borrow().clone());
            window.close();
        });
    }

    rebuild_list();
    window.present();
}

fn show_rule_dialog(
    parent: &impl IsA<gtk::Window>,
    initial: ForwardRule,
    on_rule: Rc<dyn Fn(ForwardRule)>,
) {
    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Regra")
        .default_width(460)
        .default_height(420)
        .build();

    let header = adw::HeaderBar::new();
    let cancel = gtk::Button::with_label("Cancelar");
    let save = gtk::Button::with_label("Salvar");
    save.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&save);

    let grid = gtk::Grid::builder()
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .row_spacing(12)
        .column_spacing(12)
        .build();

    let error = gtk::Label::new(None);
    error.add_css_class("error");
    error.set_wrap(true);
    error.set_xalign(0.0);
    error.set_visible(false);
    grid.attach(&error, 0, 0, 2, 1);

    let mut row = 1;

    let type_label = gtk::Label::new(Some("Tipo"));
    type_label.set_halign(gtk::Align::Start);
    grid.attach(&type_label, 0, row, 1, 1);
    let type_drop =
        gtk::DropDown::from_strings(&["Local (–L)", "Remoto (–R)", "SOCKS dinâmico (–D)"]);
    type_drop.set_hexpand(true);
    let selected = match initial.kind {
        ForwardKind::Local => 0,
        ForwardKind::Remote => 1,
        ForwardKind::Dynamic => 2,
    };
    type_drop.set_selected(selected);
    grid.attach(&type_drop, 1, row, 1, 1);
    row += 1;

    let bind_label = gtk::Label::new(Some("Endereço de escuta"));
    bind_label.set_halign(gtk::Align::Start);
    grid.attach(&bind_label, 0, row, 1, 1);
    let bind_entry = gtk::Entry::new();
    bind_entry.set_text(&initial.bind_address);
    bind_entry.set_hexpand(true);
    grid.attach(&bind_entry, 1, row, 1, 1);
    row += 1;

    let bind_port_label = gtk::Label::new(Some("Porta"));
    bind_port_label.set_halign(gtk::Align::Start);
    grid.attach(&bind_port_label, 0, row, 1, 1);
    let bind_port = gtk::SpinButton::with_range(0.0, 65535.0, 1.0);
    bind_port.set_value(f64::from(initial.bind_port));
    grid.attach(&bind_port, 1, row, 1, 1);
    row += 1;

    let target_host_label = gtk::Label::new(Some("Host de destino"));
    target_host_label.set_halign(gtk::Align::Start);
    grid.attach(&target_host_label, 0, row, 1, 1);
    let target_host = gtk::Entry::new();
    target_host.set_text(&initial.target_host);
    target_host.set_hexpand(true);
    grid.attach(&target_host, 1, row, 1, 1);
    row += 1;

    let target_port_label = gtk::Label::new(Some("Porta de destino"));
    target_port_label.set_halign(gtk::Align::Start);
    grid.attach(&target_port_label, 0, row, 1, 1);
    let target_port = gtk::SpinButton::with_range(0.0, 65535.0, 1.0);
    target_port.set_value(f64::from(initial.target_port));
    grid.attach(&target_port, 1, row, 1, 1);

    let update_sensitive = {
        let target_host = target_host.clone();
        let target_port = target_port.clone();
        let type_drop = type_drop.clone();
        move || {
            let dynamic = type_drop.selected() == 2;
            target_host.set_sensitive(!dynamic);
            target_port.set_sensitive(!dynamic);
        }
    };
    update_sensitive();
    {
        let update = update_sensitive.clone();
        type_drop.connect_selected_notify(move |_| update());
    }

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&grid));
    dialog.set_content(Some(&toolbar));

    {
        let dialog = dialog.clone();
        cancel.connect_clicked(move |_| dialog.close());
    }

    {
        let dialog = dialog.clone();
        let type_drop = type_drop.clone();
        let bind_entry = bind_entry.clone();
        let bind_port = bind_port.clone();
        let target_host = target_host.clone();
        let target_port = target_port.clone();
        let error = error.clone();
        let on_rule = on_rule.clone();
        save.connect_clicked(move |_| {
            let kind = match type_drop.selected() {
                1 => ForwardKind::Remote,
                2 => ForwardKind::Dynamic,
                _ => ForwardKind::Local,
            };
            let rule = ForwardRule {
                kind,
                bind_address: bind_entry.text().to_string(),
                bind_port: bind_port.value() as u16,
                target_host: target_host.text().to_string(),
                target_port: target_port.value() as u16,
            };
            if let Some(msg) = rule.problem() {
                error.set_text(msg);
                error.set_visible(true);
                return;
            }
            dialog.close();
            on_rule(rule);
        });
    }

    dialog.present();
    bind_entry.grab_focus();
}
