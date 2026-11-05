use glib::ExitCode;
use libadwaita as adw;
use libadwaita::prelude::*;

pub const APP_ID: &str = "io.github.liketerminal.LikeTerminal";
pub const APP_NAME: &str = "Like Terminal";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() -> ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    glib::set_application_name(APP_NAME);

    app.connect_startup(|app| {
        crate::style::load();
        register_accels(app);
    });

    app.connect_activate(|app| {
        crate::window::present(app);
    });

    app.run()
}

fn register_accels(app: &adw::Application) {
    let accels: &[(&str, &[&str])] = &[
        ("app.new-session", &["<Control><Shift>n"]),
        ("app.quick-connect", &["<Control>l"]),
        ("app.new-tab", &["<Control><Shift>t"]),
        ("app.close-tab", &["<Control><Shift>w"]),
        ("app.split-right", &["<Control><Shift>d"]),
        ("app.split-down", &["<Control><Shift>e"]),
        ("app.copy", &["<Control><Shift>c"]),
        ("app.paste", &["<Control><Shift>v"]),
        ("app.find", &["<Control><Shift>f"]),
        ("app.zoom-in", &["<Control>plus", "<Control>equal"]),
        ("app.zoom-out", &["<Control>minus"]),
        ("app.zoom-reset", &["<Control>0"]),
        ("app.toggle-sidebar", &["F9"]),
        ("app.preferences", &["<Control>comma"]),
        ("app.shortcuts", &["<Control>question"]),
        ("app.about", &[]),
        ("app.quit", &["<Control>q"]),
        ("app.broadcast", &["<Control><Shift>b"]),
    ];

    for (action, keys) in accels {
        app.set_accels_for_action(action, keys);
    }
}
