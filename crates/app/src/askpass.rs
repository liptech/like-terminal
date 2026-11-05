use std::env;
use std::io::Write;

use glib::ExitCode;
use lt_core::secrets::{self, KeyringStore, SecretStore};
use uuid::Uuid;

pub fn run_if_requested() -> Option<ExitCode> {
    let session = env::var("LT_ASKPASS_SESSION").ok()?;
    let kind = env::var("LT_ASKPASS_KIND").unwrap_or_else(|_| "password".to_string());
    Some(execute(&session, &kind))
}

fn execute(session: &str, kind: &str) -> ExitCode {
    let Ok(session_id) = session.parse::<Uuid>() else {
        eprintln!("like-terminal: sessão inválida no askpass");
        return ExitCode::FAILURE;
    };

    let account = match kind {
        "passphrase" => secrets::account_passphrase(session_id),
        _ => secrets::account_password(session_id),
    };

    let store = KeyringStore::new();
    match store.get(&account) {
        Ok(Some(secret)) => {
            let mut stdout = std::io::stdout();
            if writeln!(stdout, "{secret}").is_ok() && stdout.flush().is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Ok(None) => {
            eprintln!("like-terminal: nenhum segredo salvo para esta sessão");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("like-terminal: {error}");
            ExitCode::FAILURE
        }
    }
}
