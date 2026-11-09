use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use lt_core::{HighlightConfig, PaneLayout, Profile, Session, SplitAxis};
use uuid::Uuid;

use crate::terminal::{ConnState, TerminalView};

enum Layout {
    Leaf(Rc<TerminalView>),
    Split {
        orientation: gtk::Orientation,
        ratio: f64,
        start: Box<Layout>,
        end: Box<Layout>,
    },
}

pub struct PaneView {
    pub root: gtk::Box,
    session: Session,
    profile: Profile,
    highlight: HighlightConfig,
    exe: PathBuf,
    container: gtk::Box,
    search_bar: gtk::SearchBar,
    search_entry: gtk::SearchEntry,
    layout: RefCell<Option<Layout>>,
    active: RefCell<Option<Rc<TerminalView>>>,
    attached: RefCell<HashSet<Uuid>>,
    me: RefCell<std::rc::Weak<PaneView>>,
    on_close: Rc<dyn Fn()>,
    on_state: Rc<dyn Fn()>,
}

impl PaneView {
    pub fn new(
        session: Session,
        profile: Profile,
        highlight: HighlightConfig,
        exe: PathBuf,
        on_close: Rc<dyn Fn()>,
        on_state: Rc<dyn Fn()>,
    ) -> Result<Rc<Self>, lt_ssh::BuildError> {
        let first = TerminalView::new(&session, &profile, &highlight, exe.clone(), {
            let on_state = on_state.clone();
            Rc::new(move |_state: ConnState| on_state())
        })?;
        Ok(Self::assemble(
            session,
            profile,
            highlight,
            exe,
            Layout::Leaf(first),
            on_close,
            on_state,
        ))
    }

    pub fn with_tree(
        tree: PaneLayout,
        resolve: &dyn Fn(Uuid) -> Option<Session>,
        profile: Profile,
        highlight: HighlightConfig,
        exe: PathBuf,
        on_close: Rc<dyn Fn()>,
        on_state: Rc<dyn Fn()>,
    ) -> Option<Rc<Self>> {
        let base = resolve(tree.first_session()?)?;
        let layout = build_layout(&tree, resolve, &profile, &highlight, &exe, &on_state)?;
        Some(Self::assemble(
            base, profile, highlight, exe, layout, on_close, on_state,
        ))
    }

    fn assemble(
        session: Session,
        profile: Profile,
        highlight: HighlightConfig,
        exe: PathBuf,
        layout: Layout,
        on_close: Rc<dyn Fn()>,
        on_state: Rc<dyn Fn()>,
    ) -> Rc<Self> {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_hexpand(true);
        root.set_vexpand(true);

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.set_hexpand(true);
        container.set_vexpand(true);

        let search_bar = gtk::SearchBar::new();
        let search_entry = gtk::SearchEntry::new();
        search_entry.set_placeholder_text(Some("Buscar no terminal"));
        search_entry.set_hexpand(true);
        let search_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let previous = gtk::Button::from_icon_name("go-up-symbolic");
        previous.add_css_class("flat");
        let next = gtk::Button::from_icon_name("go-down-symbolic");
        next.add_css_class("flat");
        search_box.append(&search_entry);
        search_box.append(&previous);
        search_box.append(&next);
        search_bar.connect_entry(&search_entry);
        search_bar.set_child(Some(&search_box));

        root.append(&container);
        root.append(&search_bar);

        let active = leaves(&layout).first().map(|view| (*view).clone());

        let pane = Rc::new(Self {
            root,
            session,
            profile,
            highlight,
            exe,
            container,
            search_bar,
            search_entry,
            layout: RefCell::new(Some(layout)),
            active: RefCell::new(active),
            attached: RefCell::new(HashSet::new()),
            me: RefCell::new(std::rc::Weak::new()),
            on_close,
            on_state,
        });
        *pane.me.borrow_mut() = Rc::downgrade(&pane);

        pane.rebuild();
        pane.wire_search(&previous, &next);
        if let Some(view) = pane.active.borrow().clone() {
            view.root.add_css_class("active-pane");
        }
        pane
    }

    pub fn tree(&self) -> Option<PaneLayout> {
        self.layout.borrow().as_ref().map(layout_tree)
    }

    fn wire_search(&self, previous: &gtk::Button, next: &gtk::Button) {
        let weak = self.me.borrow().clone();
        let entry = self.search_entry.clone();
        let pane_for_change = weak.clone();
        entry.connect_search_changed(move |entry| {
            if let Some(pane) = pane_for_change.upgrade() {
                if let Some(terminal) = pane.active.borrow().clone() {
                    terminal.search(entry.text().as_str());
                }
            }
        });

        let pane_for_next = weak.clone();
        next.connect_clicked(move |_| {
            if let Some(pane) = pane_for_next.upgrade() {
                pane.find_next();
            }
        });

        previous.connect_clicked(move |_| {
            if let Some(pane) = weak.upgrade() {
                pane.find_previous();
            }
        });
    }

    fn rebuild_later(&self) {
        let weak = self.me.borrow().clone();
        glib::idle_add_local_once(move || {
            if let Some(pane) = weak.upgrade() {
                pane.rebuild();
            }
        });
    }

    fn rebuild(&self) {
        {
            let layout = self.layout.borrow();
            if let Some(layout) = layout.as_ref() {
                for view in leaves(layout) {
                    view.root.unparent();
                }
            }
        }
        while let Some(child) = self.container.first_child() {
            detach_paneds(&child);
            self.container.remove(&child);
        }

        let layout = self.layout.borrow();
        let widget = layout.as_ref().map(|layout| self.build_widget(layout));
        drop(layout);

        if let Some(widget) = widget {
            self.container.append(&widget);
        }
    }

    fn build_widget(&self, layout: &Layout) -> gtk::Widget {
        match layout {
            Layout::Leaf(view) => {
                self.attach_menu(view);
                view.root.clone().upcast()
            }
            Layout::Split {
                orientation,
                ratio,
                start,
                end,
            } => {
                let paned = gtk::Paned::new(*orientation);
                paned.set_wide_handle(true);
                paned.set_resize_start_child(true);
                paned.set_resize_end_child(true);
                paned.set_shrink_start_child(false);
                paned.set_shrink_end_child(false);
                paned.set_hexpand(true);
                paned.set_vexpand(true);
                paned.set_start_child(Some(&self.build_widget(start)));
                paned.set_end_child(Some(&self.build_widget(end)));

                let ratio = *ratio;
                let applied = Cell::new(false);
                paned.connect_map(move |paned| {
                    if applied.get() {
                        return;
                    }
                    let total = if paned.orientation() == gtk::Orientation::Horizontal {
                        paned.width()
                    } else {
                        paned.height()
                    };
                    if total > 0 {
                        applied.set(true);
                        paned.set_position((f64::from(total) * ratio) as i32);
                    }
                });

                paned.upcast()
            }
        }
    }

    fn attach_menu(&self, view: &Rc<TerminalView>) {
        if !self.attached.borrow_mut().insert(view.id) {
            return;
        }

        let weak = self.me.borrow().clone();
        let group = gio::SimpleActionGroup::new();

        let target = view.clone();
        let weak_split = weak.clone();
        let split_right = gio::SimpleAction::new("split-right", None);
        split_right.connect_activate(move |_, _| {
            if let Some(pane) = weak_split.upgrade() {
                pane.set_active(target.clone());
                pane.split(gtk::Orientation::Horizontal);
            }
        });
        group.add_action(&split_right);

        let target = view.clone();
        let weak_split = weak.clone();
        let split_down = gio::SimpleAction::new("split-down", None);
        split_down.connect_activate(move |_, _| {
            if let Some(pane) = weak_split.upgrade() {
                pane.set_active(target.clone());
                pane.split(gtk::Orientation::Vertical);
            }
        });
        group.add_action(&split_down);

        let weak_close = weak.clone();
        let close = gio::SimpleAction::new("close", None);
        close.connect_activate(move |_, _| {
            if let Some(pane) = weak_close.upgrade() {
                pane.close_active();
            }
        });
        group.add_action(&close);

        let weak_copy = weak.clone();
        let copy = gio::SimpleAction::new("copy", None);
        copy.connect_activate(move |_, _| {
            if let Some(pane) = weak_copy.upgrade() {
                pane.copy();
            }
        });
        group.add_action(&copy);

        let weak_paste = weak.clone();
        let paste = gio::SimpleAction::new("paste", None);
        paste.connect_activate(move |_, _| {
            if let Some(pane) = weak_paste.upgrade() {
                pane.paste();
            }
        });
        group.add_action(&paste);

        let weak_log = weak.clone();
        let toggle_log = gio::SimpleAction::new("toggle-log", None);
        toggle_log.connect_activate(move |_, _| {
            if let Some(pane) = weak_log.upgrade() {
                if let Some(view) = pane.active() {
                    view.toggle_log();
                }
                (pane.on_state)();
            }
        });
        group.add_action(&toggle_log);

        let weak_logs = weak.clone();
        let open_logs = gio::SimpleAction::new("open-logs", None);
        open_logs.connect_activate(move |_, _| {
            if let Some(pane) = weak_logs.upgrade() {
                if let Some(view) = pane.active() {
                    let directory = view.log_directory();
                    let _ = std::fs::create_dir_all(&directory);
                    let uri = gtk::gio::File::for_path(&directory).uri();
                    let _ = gtk::gio::AppInfo::launch_default_for_uri(
                        &uri,
                        None::<&gtk::gio::AppLaunchContext>,
                    );
                }
            }
        });
        group.add_action(&open_logs);

        let weak_find = weak;
        let find = gio::SimpleAction::new("find", None);
        find.connect_activate(move |_, _| {
            if let Some(pane) = weak_find.upgrade() {
                pane.toggle_search();
            }
        });
        group.add_action(&find);

        view.root.insert_action_group("pane", Some(&group));

        let menu = gio::Menu::new();
        menu.append(Some("Dividir à direita"), Some("pane.split-right"));
        menu.append(Some("Dividir abaixo"), Some("pane.split-down"));
        let edit_section = gio::Menu::new();
        edit_section.append(Some("Copiar"), Some("pane.copy"));
        edit_section.append(Some("Colar"), Some("pane.paste"));
        edit_section.append(Some("Buscar…"), Some("pane.find"));
        menu.append_section(None, &edit_section);

        let log_section = gio::Menu::new();
        log_section.append(Some("Iniciar/parar log"), Some("pane.toggle-log"));
        log_section.append(Some("Abrir pasta de logs"), Some("pane.open-logs"));
        menu.append_section(None, &log_section);

        let close_section = gio::Menu::new();
        close_section.append(Some("Fechar painel"), Some("pane.close"));
        menu.append_section(None, &close_section);

        let popover = gtk::PopoverMenu::from_model(Some(&menu));
        popover.set_has_arrow(false);
        popover.set_parent(&view.root);

        let gesture = gtk::GestureClick::new();
        gesture.set_button(3);
        let popover_for_click = popover.clone();
        gesture.connect_pressed(move |gesture, _n, x, y| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            popover_for_click
                .set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover_for_click.popup();
        });
        view.root.add_controller(gesture);

        let popover_for_destroy = popover;
        view.root
            .connect_destroy(move |_| popover_for_destroy.unparent());

        let target = view.clone();
        let weak_focus = self.me.borrow().clone();
        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter(move |_| {
            if let Some(pane) = weak_focus.upgrade() {
                pane.set_active(target.clone());
            }
        });
        view.root.add_controller(focus);
    }

    fn set_active(&self, view: Rc<TerminalView>) {
        if let Some(previous) = self.active.borrow().as_ref() {
            previous.root.remove_css_class("active-pane");
        }
        view.root.add_css_class("active-pane");
        *self.active.borrow_mut() = Some(view);
        (self.on_state)();
    }

    pub fn active(&self) -> Option<Rc<TerminalView>> {
        self.active.borrow().clone()
    }

    pub fn focus(&self) {
        if let Some(active) = self.active() {
            active.focus();
        }
    }

    pub fn split(&self, orientation: gtk::Orientation) {
        let Some(active) = self.active() else {
            return;
        };

        let new_view = match TerminalView::new(
            &self.session,
            &self.profile,
            &self.highlight,
            self.exe.clone(),
            {
                let on_state = self.on_state.clone();
                Rc::new(move |_state: ConnState| on_state())
            },
        ) {
            Ok(view) => view,
            Err(error) => {
                tracing::warn!(%error, "falha ao dividir o painel");
                return;
            }
        };

        let layout = self.layout.borrow_mut().take();
        let Some(layout) = layout else {
            return;
        };
        let (layout, done) = split_in(layout, active.id, orientation, new_view.clone());
        *self.layout.borrow_mut() = Some(layout);
        if !done {
            return;
        }

        self.rebuild_later();
        *self.active.borrow_mut() = Some(new_view.clone());
        new_view.focus();
        (self.on_state)();
    }

    pub fn close_active(&self) {
        let Some(active) = self.active.borrow().clone() else {
            return;
        };

        let layout = self.layout.borrow_mut().take();
        let Some(layout) = layout else {
            return;
        };
        let (layout, removed) = remove_in(layout, active.id);
        if !removed {
            *self.layout.borrow_mut() = Some(layout.expect("layout kept"));
            return;
        }

        match layout {
            Some(layout) => {
                let next_active = first_leaf(&layout).cloned();
                *self.layout.borrow_mut() = Some(layout);
                self.rebuild();
                *self.active.borrow_mut() = next_active.clone();
                if let Some(view) = next_active {
                    view.focus();
                }
                (self.on_state)();
            }
            None => {
                *self.active.borrow_mut() = None;
                (self.on_close)();
            }
        }
    }

    pub fn broadcast_raw(&self, text: &str) {
        let layout = self.layout.borrow();
        if let Some(layout) = layout.as_ref() {
            for view in leaves(layout) {
                if view.is_running() {
                    view.send(text);
                }
            }
        }
    }

    pub fn broadcast(&self, text: &str) {
        let layout = self.layout.borrow();
        if let Some(layout) = layout.as_ref() {
            for view in leaves(layout) {
                if view.is_running() {
                    view.send_line(text);
                }
            }
        }
    }

    pub fn copy(&self) {
        if let Some(active) = self.active() {
            active.copy();
        }
    }

    pub fn paste(&self) {
        if let Some(active) = self.active() {
            active.paste();
        }
    }

    pub fn zoom(&self, delta: f64) {
        if let Some(active) = self.active() {
            active.zoom(delta);
        }
    }

    pub fn reset_zoom(&self) {
        if let Some(active) = self.active() {
            active.reset_zoom();
        }
    }

    pub fn find_next(&self) {
        if let Some(active) = self.active() {
            active.find_next();
        }
    }

    pub fn find_previous(&self) {
        if let Some(active) = self.active() {
            active.find_previous();
        }
    }

    pub fn toggle_search(&self) {
        let visible = !self.search_bar.is_search_mode();
        self.search_bar.set_search_mode(visible);
        if visible {
            self.search_entry.grab_focus();
        } else {
            self.focus();
        }
    }

    pub fn has_running(&self) -> bool {
        let layout = self.layout.borrow();
        layout
            .as_ref()
            .map(|layout| leaves(layout).iter().any(|view| view.is_running()))
            .unwrap_or(false)
    }

    pub fn apply_profile(&self, profile: &Profile) {
        let layout = self.layout.borrow();
        if let Some(layout) = layout.as_ref() {
            for view in leaves(layout) {
                crate::terminal::apply_profile(&view.terminal, profile);
            }
        }
    }

    pub fn title(&self) -> String {
        self.session.name.clone()
    }

    pub fn session(&self) -> &Session {
        &self.session
    }
}

fn split_in(
    layout: Layout,
    target: Uuid,
    orientation: gtk::Orientation,
    new_view: Rc<TerminalView>,
) -> (Layout, bool) {
    match layout {
        Layout::Leaf(view) => {
            if view.id == target {
                let split = Layout::Split {
                    orientation,
                    ratio: 0.5,
                    start: Box::new(Layout::Leaf(view)),
                    end: Box::new(Layout::Leaf(new_view)),
                };
                (split, true)
            } else {
                (Layout::Leaf(view), false)
            }
        }
        Layout::Split {
            orientation: existing,
            ratio,
            start,
            end,
        } => {
            let (start, done) = split_in(*start, target, orientation, new_view.clone());
            if done {
                return (
                    Layout::Split {
                        orientation: existing,
                        ratio,
                        start: Box::new(start),
                        end,
                    },
                    true,
                );
            }
            let (end, done) = split_in(*end, target, orientation, new_view);
            (
                Layout::Split {
                    orientation: existing,
                    ratio,
                    start: Box::new(start),
                    end: Box::new(end),
                },
                done,
            )
        }
    }
}

fn remove_in(layout: Layout, target: Uuid) -> (Option<Layout>, bool) {
    match layout {
        Layout::Leaf(view) => {
            if view.id == target {
                (None, true)
            } else {
                (Some(Layout::Leaf(view)), false)
            }
        }
        Layout::Split {
            orientation,
            ratio,
            start,
            end,
        } => {
            let (left, removed) = remove_in(*start, target);
            if removed {
                return match left {
                    Some(left) => (
                        Some(Layout::Split {
                            orientation,
                            ratio,
                            start: Box::new(left),
                            end,
                        }),
                        true,
                    ),
                    None => (Some(*end), true),
                };
            }
            let (right, removed) = remove_in(*end, target);
            if removed {
                return match right {
                    Some(right) => (
                        Some(Layout::Split {
                            orientation,
                            ratio,
                            start: Box::new(left.unwrap()),
                            end: Box::new(right),
                        }),
                        true,
                    ),
                    None => (Some(left.unwrap()), true),
                };
            }
            (
                Some(Layout::Split {
                    orientation,
                    ratio,
                    start: Box::new(left.unwrap()),
                    end: Box::new(right.unwrap()),
                }),
                false,
            )
        }
    }
}

fn layout_tree(layout: &Layout) -> PaneLayout {
    match layout {
        Layout::Leaf(view) => PaneLayout::leaf(view.session.id),
        Layout::Split {
            orientation,
            ratio,
            start,
            end,
        } => PaneLayout::split(
            if *orientation == gtk::Orientation::Horizontal {
                SplitAxis::Horizontal
            } else {
                SplitAxis::Vertical
            },
            *ratio,
            layout_tree(start),
            layout_tree(end),
        ),
    }
}

fn build_layout(
    tree: &PaneLayout,
    resolve: &dyn Fn(Uuid) -> Option<Session>,
    profile: &Profile,
    highlight: &HighlightConfig,
    exe: &PathBuf,
    on_state: &Rc<dyn Fn()>,
) -> Option<Layout> {
    match tree {
        PaneLayout::Leaf { session } => {
            let session = resolve(*session)?;
            let on_state = on_state.clone();
            let view = TerminalView::new(&session, profile, highlight, exe.clone(), {
                let on_state = on_state.clone();
                Rc::new(move |_state: ConnState| on_state())
            })
            .ok()?;
            Some(Layout::Leaf(view))
        }
        PaneLayout::Split {
            axis,
            ratio,
            start,
            end,
        } => {
            let left = build_layout(start, resolve, profile, highlight, exe, on_state);
            let right = build_layout(end, resolve, profile, highlight, exe, on_state);
            let (start, end) = match (left, right) {
                (Some(left), Some(right)) => (left, right),
                (Some(left), None) => return Some(left),
                (None, Some(right)) => return Some(right),
                (None, None) => return None,
            };
            Some(Layout::Split {
                orientation: match axis {
                    SplitAxis::Horizontal => gtk::Orientation::Horizontal,
                    SplitAxis::Vertical => gtk::Orientation::Vertical,
                },
                ratio: *ratio,
                start: Box::new(start),
                end: Box::new(end),
            })
        }
    }
}

fn detach_paneds(widget: &gtk::Widget) {
    let Some(paned) = widget.downcast_ref::<gtk::Paned>() else {
        return;
    };
    if let Some(child) = paned.start_child() {
        detach_paneds(&child);
    }
    if let Some(child) = paned.end_child() {
        detach_paneds(&child);
    }
    paned.set_start_child(None::<&gtk::Widget>);
    paned.set_end_child(None::<&gtk::Widget>);
}

fn leaves(layout: &Layout) -> Vec<&Rc<TerminalView>> {
    let mut found = Vec::new();
    collect_leaves(layout, &mut found);
    found
}

fn collect_leaves<'a>(layout: &'a Layout, found: &mut Vec<&'a Rc<TerminalView>>) {
    match layout {
        Layout::Leaf(view) => found.push(view),
        Layout::Split { start, end, .. } => {
            collect_leaves(start, found);
            collect_leaves(end, found);
        }
    }
}

fn first_leaf(layout: &Layout) -> Option<&Rc<TerminalView>> {
    match layout {
        Layout::Leaf(view) => Some(view),
        Layout::Split { start, .. } => first_leaf(start),
    }
}
