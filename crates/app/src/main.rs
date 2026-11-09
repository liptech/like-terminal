mod app;
mod askpass;
mod document;
mod forwards_dialog;
mod highlight_page;
mod panes;
mod preferences;
mod session_dialog;
mod sftp_window;
mod sidebar;
mod snippets;
mod ssh_import;
mod state;
mod style;
mod terminal;
mod window;

use glib::ExitCode;

fn main() -> ExitCode {
    if let Some(code) = askpass::run_if_requested() {
        return code;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "like_terminal=info,lt_core=info,lt_ssh=info".into()),
        )
        .init();

    app::run()
}
